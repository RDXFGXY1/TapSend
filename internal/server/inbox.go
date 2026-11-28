package server

import (
	"encoding/base64"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"
)

// Uploads are staged privately and published only after the entire body arrives.
func (a *App) handleUpload(w http.ResponseWriter, r *http.Request) {
	name := filepath.Base(strings.ReplaceAll(r.Header.Get("X-File-Name"), "\\", "/"))
	if name == "." || name == "" || name == "/" {
		http.Error(w, "File name required", 400)
		return
	}
	dir := a.cfg.InboxDirectory
	if dir == "" {
		dir = filepath.Join(a.cfg.DataDirectory, "inbox")
	}
	dropToken := r.Header.Get("X-TapSend-Folder-Drop")
	if dropToken != "" {
		folder, token := a.folderDropFor(a.requestPeer(r))
		if token == "" || token != dropToken {
			http.Error(w, "The folder drop changed or expired. Choose the destination again.", http.StatusConflict)
			return
		}
		dir = folder
		if encoded := r.Header.Get("X-TapSend-File-Name"); encoded != "" {
			decoded, err := base64.RawURLEncoding.DecodeString(encoded)
			if err != nil || !utf8.Valid(decoded) {
				http.Error(w, "File name is invalid", http.StatusBadRequest)
				return
			}
			name = string(decoded)
		}
		if name == "" || len(name) > 240 || strings.ContainsAny(name, "/\\:\x00") ||
			strings.TrimRight(name, ". ") != name || !filepath.IsLocal(name) {
			http.Error(w, "File name cannot be used in the drop folder", http.StatusBadRequest)
			return
		}
	} else if err := os.MkdirAll(dir, 0700); err != nil {
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
	telemetry := newTransferTelemetry(name, "RX", r.ContentLength)
	defer telemetry.finish("failed")
	limit := a.cfg.UploadLimitGiB
	if limit == 0 { // Existing callers constructing Config directly retain the previous limit.
		limit = 8
	}
	n, err := io.Copy(telemetryWriter{f, telemetry}, http.MaxBytesReader(w, r.Body, int64(limit)<<30))
	if err != nil {
		http.Error(w, fmt.Sprintf("Upload interrupted or exceeds %d GiB", limit), 400)
		return
	}
	if err = f.Close(); err != nil {
		http.Error(w, "Cannot save file", 500)
		return
	}
	destination := f.Name() + "-" + name
	if dropToken != "" {
		destination, err = publishFolderDrop(f.Name(), dir, name)
	} else {
		err = os.Rename(f.Name(), destination)
	}
	if err != nil {
		http.Error(w, "Cannot save file", 500)
		return
	}
	telemetry.finish("saved")
	log.Printf("RECEIVED %q (%d bytes) saved to %q", name, n, destination)
	if strings.HasPrefix(name, "note-") && strings.HasSuffix(name, ".txt") && n <= 4096 {
		if data, err := os.ReadFile(destination); err == nil {
			log.Printf("CHAT phone: %q", string(data))
		}
	}
	from := a.pairing.DeviceName(strings.TrimPrefix(r.Header.Get("Authorization"), "Bearer "))
	a.recordFrom(name, "saved-to-inbox", n, from)
	reply := map[string]string{"saved": fmt.Sprintf("%s (%d bytes)", name, n)}
	if dropToken != "" {
		reply["folder"] = dir
		reply["name"] = filepath.Base(destination)
	}
	writeJSON(w, reply)
}
