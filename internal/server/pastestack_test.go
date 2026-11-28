package server

import (
	"bytes"
	"crypto/ed25519"
	"encoding/binary"
	"image"
	"image/color"
	"image/png"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"tapsend/internal/config"
)

func TestPasteStackPersistsOrderAndDeduplicatesRetries(t *testing.T) {
	cfg := config.Config{ListenAddress: ":8443", DataDirectory: t.TempDir(), DirectJoinLimit: 2}
	start := func() *App {
		t.Helper()
		app, err := New(cfg)
		if err != nil {
			t.Fatal(err)
		}
		return app
	}
	app := start()
	public, private, err := ed25519.GenerateKey(nil)
	if err != nil {
		t.Fatal(err)
	}
	if err := app.pairing.Pair(app.pairingCode, "paste-phone", "Test phone", public); err != nil {
		t.Fatal(err)
	}
	login := func(a *App) string {
		t.Helper()
		challenge, err := a.pairing.Challenge("paste-phone")
		if err != nil {
			t.Fatal(err)
		}
		token, _, err := a.pairing.Verify("paste-phone", ed25519.Sign(private, []byte(challenge)))
		if err != nil {
			t.Fatal(err)
		}
		return token
	}
	token := login(app)
	upload := func(id, kind string, data []byte, status int) {
		t.Helper()
		req := httptest.NewRequest("POST", "/api/paste-stack/"+id, bytes.NewReader(data))
		req.SetPathValue("id", id)
		req.Header.Set("Authorization", "Bearer "+token)
		req.Header.Set("X-TapSend-Kind", kind)
		result := httptest.NewRecorder()
		app.requireAuth(app.handlePasteUpload)(result, req)
		if result.Code != status {
			t.Fatalf("upload: got %d, want %d: %s", result.Code, status, result.Body)
		}
	}
	bitmap := image.NewRGBA(image.Rect(0, 0, 1, 2))
	bitmap.Set(0, 0, color.RGBA{R: 255, A: 255})
	bitmap.Set(0, 1, color.RGBA{B: 255, A: 255})
	var pngData bytes.Buffer
	if err := png.Encode(&pngData, bitmap); err != nil {
		t.Fatal(err)
	}
	firstID, secondID := strings.Repeat("a", 32), strings.Repeat("b", 32)
	text := []byte("  link https://example.invalid\n日本語\n")
	upload(firstID, "text", text, 200)
	upload(secondID, "image", pngData.Bytes(), 200)
	upload(secondID, "image", pngData.Bytes(), 200)
	if len(app.paste.manifest.Entries) != 2 {
		t.Fatal("upload retry added a duplicate")
	}
	textKey, imageKey := app.paste.manifest.Entries[0].Key, app.paste.manifest.Entries[1].Key
	if err := app.changePaste(imageKey, "paste-up"); err != nil {
		t.Fatal(err)
	}

	app = start()
	token = login(app)
	if rows := app.paste.manifest.Entries; len(rows) != 2 || rows[0].Key != imageKey || rows[1].Key != textKey {
		t.Fatalf("reordered stack did not survive restart: %+v", rows)
	}
	dib, err := os.ReadFile(filepath.Join(app.pasteDirectory(), imageKey+".clip"))
	if err != nil || len(dib) != 48 || binary.LittleEndian.Uint32(dib[4:]) != 1 ||
		binary.LittleEndian.Uint32(dib[8:]) != 2 || !bytes.Equal(dib[40:], []byte{255, 0, 0, 0, 0, 0, 255, 0}) {
		t.Fatalf("wrong Windows clipboard header or pixels: %v %v", dib, err)
	}
	if err := app.changePaste(imageKey, "paste-copied"); err != nil {
		t.Fatal(err)
	}
	app = start()
	token = login(app)
	upload(secondID, "image", pngData.Bytes(), 200)
	if len(app.paste.manifest.Entries) != 1 {
		t.Fatal("copied item returned after a retry")
	}
	saved, err := os.ReadFile(filepath.Join(app.pasteDirectory(), textKey+".clip"))
	if err != nil || !bytes.Equal(saved, text) {
		t.Fatalf("remaining text changed: %q %v", saved, err)
	}
	upload(strings.Repeat("c", 32), "image", []byte("invalid image"), http.StatusUnprocessableEntity)
}
