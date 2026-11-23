package server

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"tapsend/internal/config"
)

func TestReceiveOnlyServerSavesUploadWithoutSharedFiles(t *testing.T) {
	root := t.TempDir()
	inbox := filepath.Join(root, "received")
	app, err := New(config.Config{ListenAddress: ":8443", DataDirectory: filepath.Join(root, "data"), InboxDirectory: inbox})
	if err != nil {
		t.Fatal(err)
	}
	if len(app.catalog.Public()) != 0 {
		t.Fatal("receive-only server should share no computer files")
	}
	request := httptest.NewRequest(http.MethodPost, "/api/upload", strings.NewReader("from phone"))
	request.Header.Set("X-File-Name", "photo.txt")
	response := httptest.NewRecorder()
	app.handleUpload(response, request)
	if response.Code != http.StatusOK {
		t.Fatalf("upload failed: %d %s", response.Code, response.Body.String())
	}
	files, err := filepath.Glob(filepath.Join(inbox, "incoming-*-photo.txt"))
	if err != nil || len(files) != 1 {
		t.Fatalf("uploaded file missing: %v, %v", files, err)
	}
	contents, err := os.ReadFile(files[0])
	if err != nil || string(contents) != "from phone" {
		t.Fatalf("wrong saved contents: %q, %v", contents, err)
	}
}
