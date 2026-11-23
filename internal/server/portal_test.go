package server

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"

	"tapsend/internal/config"
)

func TestPortalShowsOnlySavedArrivalsToLocalComputer(t *testing.T) {
	root := t.TempDir()
	app, err := New(config.Config{ListenAddress: ":8443", DataDirectory: filepath.Join(root, "data"), InboxDirectory: filepath.Join(root, "inbox")})
	if err != nil {
		t.Fatal(err)
	}
	app.record("unfinished.txt", "failed", 10)
	request := httptest.NewRequest(http.MethodPost, "/api/upload", strings.NewReader("hello"))
	request.Header.Set("X-File-Name", "landed.txt")
	response := httptest.NewRecorder()
	app.handleUpload(response, request)
	if response.Code != http.StatusOK {
		t.Fatalf("upload failed: %d %s", response.Code, response.Body.String())
	}
	portal := httptest.NewRecorder()
	local := httptest.NewRequest(http.MethodGet, "/api/portal", nil)
	local.RemoteAddr = "127.0.0.1:12345"
	app.localOnly(app.handlePortal)(portal, local)
	if portal.Code != http.StatusOK {
		t.Fatalf("local portal failed: %d %s", portal.Code, portal.Body.String())
	}
	var state struct {
		PairingURL string         `json:"pairingURL"`
		Arrivals   []historyEntry `json:"arrivals"`
	}
	if err := json.Unmarshal(portal.Body.Bytes(), &state); err != nil {
		t.Fatal(err)
	}
	if state.PairingURL == "" || len(state.Arrivals) != 1 || state.Arrivals[0].Name != "landed.txt" || state.Arrivals[0].Bytes != 5 {
		t.Fatalf("unexpected portal state: %+v", state)
	}
	remote := httptest.NewRequest(http.MethodGet, "/api/portal", nil)
	remote.RemoteAddr = "192.0.2.10:12345"
	denied := httptest.NewRecorder()
	app.localOnly(app.handlePortal)(denied, remote)
	if denied.Code != http.StatusForbidden || strings.Contains(denied.Body.String(), "landed.txt") {
		t.Fatalf("remote portal was not denied: %d %s", denied.Code, denied.Body.String())
	}
}
