package server

import (
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"
)

// Uploads are staged privately and published only after the entire body arrives.
func (a *App) handleUpload(w http.ResponseWriter, r *http.Request) {
	name := filepath.Base(strings.ReplaceAll(r.Header.Get("X-File-Name"), "\\", "/"))
	if name == "." || name == "" || name == "/" {
		http.Error(w, "File name required", 400)
		return
	}
	dir := filepath.Join(a.cfg.DataDirectory, "inbox")
	if err := os.MkdirAll(dir, 0700); err != nil {
		http.Error(w, "Inbox unavailable", 500)
		return
	}
	f, err := os.CreateTemp(dir, "incoming-*")
	if err != nil {
		http.Error(w, "Cannot create file", 500)
		return
	}
	defer os.Remove(f.Name())
	defer f.Close()
	n, err := io.Copy(f, http.MaxBytesReader(w, r.Body, 8<<30))
	if err != nil {
		http.Error(w, "Upload interrupted or exceeds 8 GiB", 400)
		return
	}
	if err = f.Close(); err != nil {
		http.Error(w, "Cannot save file", 500)
		return
	}
	destination := f.Name() + "-" + name
	if err = os.Rename(f.Name(), destination); err != nil {
		http.Error(w, "Cannot save file", 500)
		return
	}
	log.Printf("RECEIVED %q (%d bytes) saved to %q", name, n, destination)
	if strings.HasPrefix(name, "note-") && strings.HasSuffix(name, ".txt") && n <= 4096 {
		if data, err := os.ReadFile(destination); err == nil {
			log.Printf("CHAT phone: %q", string(data))
		}
	}
	a.record(name, "saved-to-inbox", n)
	writeJSON(w, map[string]string{"saved": fmt.Sprintf("%s (%d bytes)", name, n)})
}
