package server

import (
	"encoding/base64"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestFolderDropConfinesWritesAndKeepsDuplicates(t *testing.T) {
	app, token := pairedScreenFixture(t)
	dir := t.TempDir()
	if err := app.armFolderDrop("phone", dir); err != nil {
		t.Fatal(err)
	}
	_, ticket := app.folderDropFor("phone")
	upload := func(name, supplied string, want int) {
		t.Helper()
		request := httptest.NewRequest("POST", "/api/upload", strings.NewReader("new"))
		request.Header.Set("Authorization", "Bearer "+token)
		request.Header.Set("X-File-Name", "fallback.txt")
		request.Header.Set("X-TapSend-File-Name", base64.RawURLEncoding.EncodeToString([]byte(name)))
		request.Header.Set("X-TapSend-Folder-Drop", supplied)
		w := httptest.NewRecorder()
		app.requireAuth(app.handleUpload)(w, request)
		if w.Code != want {
			t.Fatalf("upload got %d, expected %d: %s", w.Code, want, w.Body)
		}
	}
	if err := os.WriteFile(filepath.Join(dir, "report.txt"), []byte("original"), 0600); err != nil {
		t.Fatal(err)
	}
	upload("report.txt", ticket, 200)
	for name, want := range map[string]string{"report.txt": "original", "report (2).txt": "new"} {
		data, err := os.ReadFile(filepath.Join(dir, name))
		if err != nil || string(data) != want {
			t.Fatalf("%s = %q, %v", name, data, err)
		}
	}
	upload("../outside.txt", ticket, 400)
	upload("report.txt", "stale-ticket", 409)
	app.drop.expires = time.Now().Add(-time.Second)
	upload("report.txt", ticket, 409)
}

func TestHeldPastesSurviveRestartAndReturnOnTheirOwnTrigger(t *testing.T) {
	app, _ := pairedScreenFixture(t)
	for _, key := range []string{"for-folder", "for-phone"} {
		if _, err := app.storePaste(key, "text", "", "phone", []byte(key)); err != nil {
			t.Fatal(err)
		}
	}
	folder := t.TempDir()
	if err := app.holdPaste("for-folder", folder, "", ""); err != nil {
		t.Fatal(err)
	}
	if err := app.holdPaste("for-phone", "", "phone", ""); err != nil {
		t.Fatal(err)
	}
	var restored App
	restored.cfg = app.cfg
	if err := restored.restorePasteStack(); err != nil {
		t.Fatal(err)
	}
	restored.releasePastes(t.TempDir(), "", false)
	if restored.paste.manifest.Entries[0].HoldFolder != folder {
		t.Fatal("a different folder released the reminder")
	}
	restored.pastePhoneSeen("other")
	if restored.paste.manifest.Entries[1].HoldPeer != "phone" {
		t.Fatal("a different phone released the reminder")
	}
	restored.releasePastes(folder, "", false)
	restored.pastePhoneSeen("phone")
	for _, entry := range restored.paste.manifest.Entries {
		if entry.HoldFolder != "" || entry.HoldPeer != "" {
			t.Fatal("the matching trigger did not release the reminder")
		}
	}
}
