package server

import (
	"bytes"
	"crypto/ed25519"
	"crypto/tls"
	"image"
	"image/jpeg"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
	"time"

	"tapsend/internal/config"
)

func TestScreenRequiresSelectedPhoneAndLivePrivateViewer(t *testing.T) {
	app, phone := pairedScreenFixture(t)
	other := pairFixturePhone(t, app, "other")
	keyPair, err := tls.LoadX509KeyPair(app.certificates.Certificate, app.certificates.Key)
	if err != nil {
		t.Fatal(err)
	}
	closeListener, err := app.startScreenListener(keyPair)
	if err != nil {
		t.Fatal(err)
	}
	defer closeListener()

	const id = "00112233445566778899aabbccddeeff"
	var jpegData bytes.Buffer
	if err := jpeg.Encode(&jpegData, image.NewRGBA(image.Rect(0, 0, 2, 3)), nil); err != nil {
		t.Fatal(err)
	}
	send := func(token string, handler http.HandlerFunc, data []byte, status int) {
		t.Helper()
		req := httptest.NewRequest("POST", "/", bytes.NewReader(data))
		req.SetPathValue("id", id)
		req.Header.Set("Authorization", "Bearer "+token)
		w := httptest.NewRecorder()
		app.requireAuth(handler)(w, req)
		if w.Code != status {
			t.Fatalf("got %d, want %d: %s", w.Code, status, w.Body)
		}
	}
	start := []byte(`{"id":"` + id + `"}`)
	send(phone, app.handleScreenStart, start, 409)
	if err := app.armScreen("phone"); err != nil {
		t.Fatal(err)
	}
	viewKey := app.screen.viewKey
	view := func(key string, after uint64, status int) *httptest.ResponseRecorder {
		t.Helper()
		req := httptest.NewRequest("GET", "/screen?after="+strconv.FormatUint(after, 10), nil)
		req.Header.Set("X-TapSend-View", key)
		w := httptest.NewRecorder()
		app.handleScreenView(w, req)
		if w.Code != status {
			t.Fatalf("viewer got %d, want %d: %s", w.Code, status, w.Body)
		}
		return w
	}
	view("wrong-key", 0, 410)
	view(viewKey, 0, 204)
	viewerAction := func(handler http.HandlerFunc, key, kind, payload string, status int) {
		t.Helper()
		req := httptest.NewRequest("POST", "/screen/action", strings.NewReader(payload))
		req.Header.Set("X-TapSend-View", key)
		req.SetPathValue("kind", kind)
		recorder := httptest.NewRecorder()
		handler(recorder, req)
		if recorder.Code != status {
			t.Fatalf("viewer action returned %d, expected %d: %s", recorder.Code, status, recorder.Body)
		}
	}
	viewerAction(app.handleScreenGrab, "wrong-key", "text", "private note", 410)
	viewerAction(app.handleScreenGrab, viewKey, "image", jpegData.String(), 204)
	viewerAction(app.handleScreenGrab, viewKey, "text", "private note", 204)
	if len(app.paste.manifest.Entries) != 2 {
		t.Fatal("screen grabs did not reach Paste Stack")
	}
	send(other, app.handleScreenStart, start, 409)
	send(phone, app.handleScreenStart, start, 200)
	viewerAction(app.handleScreenArrow, viewKey, "", "[10,20,40,50]", 409)
	startPen := []byte(`{"id":"` + id + `","redPen":true}`)
	send(phone, app.handleScreenStart, startPen, 200)
	viewerAction(app.handleScreenArrow, "wrong-key", "", "[10,20,40,50]", 410)
	viewerAction(app.handleScreenArrow, viewKey, "", "[10,20,40,2000]", 400)
	viewerAction(app.handleScreenArrow, viewKey, "", "[10,20,40,50]", 204)
	heartbeat := httptest.NewRequest("POST", "/", nil)
	heartbeat.SetPathValue("id", id)
	heartbeat.Header.Set("Authorization", "Bearer "+phone)
	response := httptest.NewRecorder()
	app.requireAuth(app.handleScreenFrame)(response, heartbeat)
	if response.Header().Get("X-TapSend-Arrow") != "10,20,40,50" {
		t.Fatal("an opted-in phone did not receive the arrow")
	}
	send(other, app.handleScreenFrame, jpegData.Bytes(), 410)
	send(phone, app.handleScreenFrame, []byte("bad"), 400)
	send(phone, app.handleScreenFrame, jpegData.Bytes(), 204)
	if response := view(viewKey, 0, 200); !bytes.Equal(response.Body.Bytes(), jpegData.Bytes()) {
		t.Fatal("viewer received different frame bytes")
	}
	view(viewKey, 1, 204)
	waiting := httptest.NewRecorder()
	request := httptest.NewRequest("GET", "/screen?after=1&wait=1&phase=sharing", nil)
	request.Header.Set("X-TapSend-View", viewKey)
	done := make(chan struct{})
	go func() {
		defer close(done)
		app.handleScreenView(waiting, request)
	}()
	deadline := time.Now().Add(time.Second)
	for {
		app.screen.Lock()
		registered := app.screen.changed != nil
		app.screen.Unlock()
		if registered {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("viewer did not wait for a new frame")
		}
		time.Sleep(time.Millisecond)
	}
	send(phone, app.handleScreenFrame, jpegData.Bytes(), 204)
	select {
	case <-done:
		if waiting.Code != 200 || waiting.Header().Get("X-TapSend-Frame") != "2" {
			t.Fatal("waiting viewer did not receive the next frame")
		}
	case <-time.After(screenWait / 2):
		t.Fatal("the viewer was not woken by a new frame")
	}
	send(other, app.handleScreenStop, nil, 204)
	view(viewKey, 0, 200)
	send(phone, app.handleScreenStop, nil, 204)
	if response := view(viewKey, 0, 204); response.Header().Get("X-TapSend-Screen") != "stopped" {
		t.Fatal("the stopped share left a frame in the viewer")
	}
	send(phone, app.handleScreenStart, start, 200)
	app.screen.Lock()
	app.screen.viewedAt = time.Now().Add(-2 * screenLease)
	app.screen.Unlock()
	send(phone, app.handleScreenFrame, nil, 410)
	app.closeScreen()
	view(viewKey, 0, 410)
	if strings.TrimSpace(app.screen.viewKey) != "" {
		t.Fatal("viewer secret survived close")
	}
}

func pairedScreenFixture(t *testing.T) (*App, string) {
	t.Helper()
	app, err := New(config.Config{DataDirectory: t.TempDir(), DirectJoinLimit: 2})
	if err != nil {
		t.Fatal(err)
	}
	return app, pairFixturePhone(t, app, "phone")
}

func pairFixturePhone(t *testing.T, app *App, id string) string {
	t.Helper()
	public, private, err := ed25519.GenerateKey(nil)
	if err != nil {
		t.Fatal(err)
	}
	if err := app.pairing.Pair(app.pairingCode, id, id, public); err != nil {
		t.Fatal(err)
	}
	challenge, err := app.pairing.Challenge(id)
	if err != nil {
		t.Fatal(err)
	}
	token, _, err := app.pairing.Verify(id, ed25519.Sign(private, []byte(challenge)))
	if err != nil {
		t.Fatal(err)
	}
	return token
}
