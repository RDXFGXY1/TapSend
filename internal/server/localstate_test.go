package server

import (
	"crypto/ed25519"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"tapsend/internal/config"
)

func TestOutboxAndScratchpadSurviveRestartAndStayPrivate(t *testing.T) {
	root := t.TempDir()
	cfg := config.Config{ListenAddress: ":8443", DataDirectory: filepath.Join(root, "data"), DirectJoinLimit: 2}
	start := func() *App {
		t.Helper()
		app, err := New(cfg)
		if err != nil {
			t.Fatal(err)
		}
		return app
	}
	app := start()
	keys := map[string]ed25519.PrivateKey{}
	for _, id := range []string{"chosen", "other"} {
		public, private, err := ed25519.GenerateKey(nil)
		if err != nil {
			t.Fatal(err)
		}
		keys[id] = private
		if err := app.pairing.Pair(app.pairingCode, id, id, public); err != nil {
			t.Fatal(err)
		}
	}
	login := func(app *App, peer string) string {
		t.Helper()
		challenge, err := app.pairing.Challenge(peer)
		if err != nil {
			t.Fatal(err)
		}
		token, _, err := app.pairing.Verify(peer, ed25519.Sign(keys[peer], []byte(challenge)))
		if err != nil {
			t.Fatal(err)
		}
		return token
	}
	request := func(method, token, body string) *http.Request {
		req := httptest.NewRequest(method, "/", strings.NewReader(body))
		req.Header.Set("Authorization", "Bearer "+token)
		return req
	}
	path := filepath.Join(root, "video.txt")
	if err := os.WriteFile(path, []byte("original"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := app.offerFiles("chosen", []offerFile{{Request: 1, Path: path}}); err != nil {
		t.Fatal(err)
	}
	content := "  example\n  second line\n"
	if err := app.addSnippet("chosen", "computer", content); err != nil {
		t.Fatal(err)
	}
	if err := app.addSnippet("other", "phone", "other device"); err != nil {
		t.Fatal(err)
	}

	app = start()
	chosen, other := login(app, "chosen"), login(app, "other")
	pending := app.pendingOffers("chosen")
	if len(pending) != 1 || len(app.pendingOffers("other")) != 0 {
		t.Fatalf("offer was lost or exposed after restart: %+v", pending)
	}
	id := pending[0].Files[0].ID
	res := httptest.NewRecorder()
	app.handleScratchpad(res, request("GET", chosen, ""))
	var snippets []snippet
	if err := json.Unmarshal(res.Body.Bytes(), &snippets); err != nil || len(snippets) != 1 || snippets[0].Text != content {
		t.Fatalf("scratchpad did not restore exact private text: %s (%v)", res.Body, err)
	}
	res = httptest.NewRecorder()
	app.handleScratchpad(res, request("GET", other, ""))
	if strings.Contains(res.Body.String(), "example") {
		t.Fatal("private text exposed to another device")
	}
	accept := request("POST", chosen, `{"state":"accepted"}`)
	accept.SetPathValue("id", pending[0].ID)
	res = httptest.NewRecorder()
	app.handleOfferDecision(res, accept)
	if res.Code != 200 {
		t.Fatalf("could not accept restored offer: %s", res.Body)
	}

	app = start()
	chosen = login(app, "chosen")
	download := func() *httptest.ResponseRecorder {
		req := request("GET", chosen, "")
		req.SetPathValue("id", id)
		req.Header.Set("Range", "bytes=4-")
		result := httptest.NewRecorder()
		app.handleDownload(result, req)
		return result
	}
	if response := download(); response.Code != 206 || response.Body.String() != "inal" {
		t.Fatalf("restored accepted file could not resume: %d %s", response.Code, response.Body)
	}
	if err := os.Chtimes(path, time.Now().Add(-time.Hour), time.Now().Add(-time.Hour)); err != nil {
		t.Fatal(err)
	}
	if response := download(); response.Code != 409 {
		t.Fatalf("a changed original was offered under the old ID: %d", response.Code)
	}
	if err := app.saveOfferReceipt(id, "chosen", "received"); err != nil {
		t.Fatal(err)
	}
	app = start()
	if app.offers[0].Files[0].State != "received" {
		t.Fatal("save receipt was lost after another restart")
	}
}
