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

	"tapsend/internal/config"
)

func TestOfferOnlyRecipientCanSeeAcceptAndDownload(t *testing.T) {
	root := t.TempDir()
	app, err := New(config.Config{ListenAddress: ":8443", DataDirectory: filepath.Join(root, "data"), DirectJoinLimit: 2})
	if err != nil {
		t.Fatal(err)
	}
	pair := func(id string) string {
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
	chosen, other := pair("chosen-phone"), pair("other-phone")
	path := filepath.Join(root, "private.txt")
	if err := os.WriteFile(path, []byte("only for you"), 0600); err != nil {
		t.Fatal(err)
	}
	mux := http.NewServeMux()
	mux.HandleFunc("GET /api/session", app.requireAuth(app.handleSession))
	mux.HandleFunc("GET /api/files", app.requireAuth(app.handleFiles))
	mux.HandleFunc("GET /api/files/{id}", app.requireAuth(app.handleDownload))
	mux.HandleFunc("POST /api/offers/{id}", app.requireAuth(app.handleOfferDecision))
	mux.HandleFunc("POST /api/receipt", app.requireAuth(app.handleReceipt))
	call := func(token, method, path, body string, status int) string {
		t.Helper()
		req := httptest.NewRequest(method, path, strings.NewReader(body))
		req.Header.Set("Authorization", "Bearer "+token)
		resp := httptest.NewRecorder()
		mux.ServeHTTP(resp, req)
		if resp.Code != status {
			t.Fatalf("%s %s: got %d, want %d (%s)", method, path, resp.Code, status, resp.Body)
		}
		return resp.Body.String()
	}
	offer := func(request uint64) publicOffer {
		t.Helper()
		if err := app.offerFiles("chosen-phone", []offerFile{{Request: request, Path: path}}); err != nil {
			t.Fatal(err)
		}
		var session struct{ Offers []publicOffer }
		if err := json.Unmarshal([]byte(call(chosen, "GET", "/api/session", "", 200)), &session); err != nil || len(session.Offers) != 1 {
			t.Fatalf("expected one offer: %+v, %v", session, err)
		}
		if body := call(other, "GET", "/api/session", "", 200); !strings.Contains(body, `"offers":[]`) {
			t.Fatalf("private offer leaked to another phone: %s", body)
		}
		return session.Offers[0]
	}
	first := offer(1)
	decision, download := "/api/offers/"+first.ID, "/api/files/"+first.Files[0].ID
	call(chosen, "GET", download, "", 404)
	call(chosen, "HEAD", download, "", 404)
	call(other, "POST", decision, `{"state":"accepted"}`, 404)
	call(chosen, "POST", decision, `{"state":"declined"}`, 200)
	call(chosen, "POST", decision, `{"state":"declined"}`, 200)
	call(chosen, "POST", decision, `{"state":"accepted"}`, 409)
	call(chosen, "GET", download, "", 404)

	second := offer(2)
	decision, download = "/api/offers/"+second.ID, "/api/files/"+second.Files[0].ID
	receipt := `{"id":"` + second.Files[0].ID + `","state":"received"}`
	call(chosen, "POST", "/api/receipt", receipt, 404)
	call(chosen, "POST", decision, `{"state":"accepted"}`, 200)
	call(chosen, "POST", decision, `{"state":"accepted"}`, 200)
	if body := call(chosen, "GET", "/api/files", "", 200); !strings.Contains(body, second.Files[0].ID) {
		t.Fatalf("accepted file missing: %s", body)
	}
	if body := call(other, "GET", "/api/files", "", 200); strings.TrimSpace(body) != "[]" {
		t.Fatalf("private files leaked: %s", body)
	}
	call(other, "GET", download, "", 404)
	call(other, "POST", "/api/receipt", receipt, 404)
	if data := call(chosen, "GET", download, "", 200); data != "only for you" {
		t.Fatalf("incorrect download: %q", data)
	}
	call(chosen, "POST", "/api/receipt", receipt, 200)
}
