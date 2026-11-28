package server

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
	"time"

	"tapsend/internal/config"
)

func TestDownloadResumeRequiresSameFileVersion(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "video.bin")
	if err := os.WriteFile(path, []byte("abcdef"), 0600); err != nil {
		t.Fatal(err)
	}
	app, err := New(config.Config{ListenAddress: ":8443", DataDirectory: filepath.Join(root, "data")})
	if err != nil {
		t.Fatal(err)
	}
	item, _, err := app.catalog.Add(path)
	if err != nil {
		t.Fatal(err)
	}
	request := func(method, match string) *httptest.ResponseRecorder {
		r := httptest.NewRequest(method, "/api/files/"+item.ID, nil)
		r.SetPathValue("id", item.ID)
		if match != "" {
			r.Header.Set("If-Match", match)
			r.Header.Set("Range", "bytes=3-")
		}
		w := httptest.NewRecorder()
		app.handleDownload(w, r)
		return w
	}
	head := request(http.MethodHead, "")
	tag := head.Header().Get("ETag")
	if head.Code != http.StatusOK || tag == "" || head.Header().Get("Content-Length") != "6" || head.Body.Len() != 0 {
		t.Fatalf("invalid download metadata: %d %v %q", head.Code, head.Header(), head.Body.String())
	}
	part := request(http.MethodGet, tag)
	if part.Code != http.StatusPartialContent || part.Header().Get("Content-Range") != "bytes 3-5/6" || part.Body.String() != "def" {
		t.Fatalf("invalid resumed response: %d %v %q", part.Code, part.Header(), part.Body.String())
	}
	if err := os.WriteFile(path, []byte("ghijkl"), 0600); err != nil {
		t.Fatal(err)
	}
	changed := time.Now().Add(-time.Hour)
	if err := os.Chtimes(path, changed, changed); err != nil {
		t.Fatal(err)
	}
	stale := request(http.MethodGet, tag)
	if stale.Code != http.StatusPreconditionFailed {
		t.Fatalf("stale resume should be rejected: %d %q", stale.Code, stale.Body.String())
	}
}
