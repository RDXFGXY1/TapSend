package server

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"fmt"
	"image"
	_ "image/jpeg"
	_ "image/png"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
	"unicode"
	"unicode/utf8"
)

const (
	pasteMaxImage  = 16 << 20
	pasteMaxText   = 16 << 10
	pasteMaxPixels = 12_000_000
)

type pasteEntry struct {
	Key        string
	Kind       string
	Name       string
	Device     string
	Preview    string
	Bytes      int
	HoldFolder string `json:",omitempty"`
	HoldPeer   string `json:",omitempty"`
	FilePath   string `json:",omitempty"`
}

type pasteManifest struct {
	Entries []pasteEntry
	Copied  []string
}

type pasteStack struct {
	sync.Mutex
	manifest pasteManifest
	seen     map[string]time.Time
}

func (a *App) pasteDirectory() string {
	return filepath.Join(a.cfg.DataDirectory, "paste-content")
}

func (a *App) restorePasteStack() error {
	return readState(a.cfg.DataDirectory, "paste-stack.json", &a.paste.manifest)
}

func (a *App) pasteSnapshot() {
	a.paste.Lock()
	defer a.paste.Unlock()
	a.logPasteLocked()
}

func (a *App) logPasteLocked() {
	var fields strings.Builder
	for _, entry := range a.paste.manifest.Entries {
		if entry.HoldFolder != "" || entry.HoldPeer != "" {
			continue
		}
		path := filepath.Join(a.pasteDirectory(), entry.Key+".clip")
		if entry.FilePath != "" {
			path = entry.FilePath
		}
		fmt.Fprintf(&fields, " %s:%s:%x:%x:%x:%x", entry.Key, entry.Kind, entry.Name, entry.Device, entry.Preview, path)
	}
	log.Printf("TUI_PASTE%s", fields.String())
}

func (a *App) holdPaste(id, folder, peer, file string) error {
	if peer != "" {
		if _, ok := a.pairing.Device(peer); !ok {
			return errors.New("choose a paired phone first")
		}
	} else if !filepath.IsAbs(folder) {
		return errors.New("choose an open folder")
	}
	a.paste.Lock()
	defer a.paste.Unlock()
	next := a.paste.manifest
	next.Entries = append([]pasteEntry(nil), next.Entries...)
	if file != "" {
		info, err := os.Stat(file)
		if err != nil || !filepath.IsAbs(file) || !info.Mode().IsRegular() {
			return errors.New("focus a regular file to hold")
		}
		if len(next.Entries) >= 64 {
			return errors.New("Paste Stack is full")
		}
		key, err := screenKey()
		if err != nil {
			return err
		}
		next.Entries = append(next.Entries, pasteEntry{Key: key, Kind: "file", Name: pastePreview(info.Name()),
			Device: "Hold This for Me", FilePath: file, HoldFolder: folder, HoldPeer: peer})
		return a.savePasteLocked(next)
	}
	for i, entry := range next.Entries {
		if entry.Key == id {
			next.Entries[i].HoldFolder, next.Entries[i].HoldPeer = folder, peer
			return a.savePasteLocked(next)
		}
	}
	return errors.New("Paste Stack item is no longer available")
}

func (a *App) releasePastes(folder, peer string, all bool) {
	a.paste.Lock()
	defer a.paste.Unlock()
	next := a.paste.manifest
	next.Entries = append([]pasteEntry(nil), next.Entries...)
	count := 0
	for i, entry := range next.Entries {
		if entry.HoldFolder == "" && entry.HoldPeer == "" {
			continue
		}
		if all || (folder != "" && entry.HoldFolder == folder) || (peer != "" && entry.HoldPeer == peer) {
			next.Entries[i].HoldFolder, next.Entries[i].HoldPeer = "", ""
			count++
		}
	}
	if count > 0 {
		if err := a.savePasteLocked(next); err != nil {
			log.Printf("Could not restore held items: %s", err)
		} else {
			log.Printf("TUI_DEVICE_RESULT paste-ready ok")
		}
	}
}

func (a *App) pastePhoneSeen(peer string) {
	if peer == "" {
		return
	}
	a.paste.Lock()
	if a.paste.seen == nil {
		a.paste.seen = make(map[string]time.Time)
	}
	previous := a.paste.seen[peer]
	a.paste.seen[peer] = time.Now()
	a.paste.Unlock()
	if time.Since(previous) >= 12*time.Second {
		a.releasePastes("", peer, false)
	}
}

func (a *App) savePasteLocked(next pasteManifest) error {
	if err := writeState(a.cfg.DataDirectory, "paste-stack.json", next); err != nil {
		return err
	}
	a.paste.manifest = next
	a.logPasteLocked()
	return nil
}

func (a *App) handlePasteUpload(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if raw, err := hex.DecodeString(id); err != nil || len(raw) != 16 {
		http.Error(w, "Invalid Paste Stack item", http.StatusBadRequest)
		return
	}
	kind := r.Header.Get("X-TapSend-Kind")
	limit := int64(pasteMaxText)
	switch kind {
	case "image":
		limit = pasteMaxImage
	case "text":
	default:
		http.Error(w, "Paste Stack accepts images and text", http.StatusBadRequest)
		return
	}
	peer := a.requestPeer(r)
	if peer == "" {
		http.Error(w, "Pair again before sending", http.StatusUnauthorized)
		return
	}
	digest := sha256.Sum256([]byte(peer + "\x00" + strings.ToLower(id)))
	key := hex.EncodeToString(digest[:])
	a.paste.Lock()
	exists := a.paste.manifest.contains(key)
	full := len(a.paste.manifest.Entries) >= 64
	a.paste.Unlock()
	if exists {
		writeJSON(w, map[string]bool{"ok": true})
		return
	}
	if full {
		http.Error(w, "Paste Stack is full. Copy some items on the computer.", http.StatusConflict)
		return
	}
	a.acceptPaste(w, r, key, kind, limit)
}

func (manifest pasteManifest) contains(key string) bool {
	for _, done := range manifest.Copied {
		if done == key {
			return true
		}
	}
	for _, entry := range manifest.Entries {
		if entry.Key == key {
			return true
		}
	}
	return false
}

func (a *App) acceptPaste(w http.ResponseWriter, r *http.Request, key, kind string, limit int64) {
	controller := http.NewResponseController(w)
	_ = controller.SetReadDeadline(time.Now().Add(60 * time.Second))
	b, err := io.ReadAll(http.MaxBytesReader(w, r.Body, limit))
	_ = controller.SetReadDeadline(time.Time{})
	if err != nil {
		http.Error(w, "Paste Stack item exceeds its size limit", http.StatusRequestEntityTooLarge)
		return
	}
	label, _ := base64.RawURLEncoding.DecodeString(r.Header.Get("X-TapSend-Label"))
	device := a.pairing.DeviceName(strings.TrimPrefix(r.Header.Get("Authorization"), "Bearer "))
	if status, err := a.storePaste(key, kind, string(label), device, b); err != nil {
		http.Error(w, err.Error(), status)
		return
	}
	writeJSON(w, map[string]bool{"ok": true})
}

func (a *App) storePaste(key, kind, label, device string, b []byte) (int, error) {
	entry := pasteEntry{Key: key, Kind: kind, Name: pastePreview(label), Device: pastePreview(device)}
	var err error
	if kind == "image" {
		var width, height int
		b, width, height, err = pasteDIB(b)
		if err != nil {
			return http.StatusUnprocessableEntity, err
		}
		if entry.Name == "" {
			entry.Name = "Screenshot"
		}
		entry.Preview = fmt.Sprintf("%d × %d", width, height)
	} else {
		if !utf8.Valid(b) || bytes.Contains(b, []byte{0}) || strings.TrimSpace(string(b)) == "" {
			return http.StatusUnprocessableEntity, errors.New("Paste Stack text must contain readable text")
		}
		entry.Name = "Text"
		entry.Preview = pastePreview(string(b))
	}
	entry.Bytes = len(b)
	a.paste.Lock()
	defer a.paste.Unlock()
	current := a.paste.manifest
	if current.contains(key) {
		return http.StatusOK, nil
	}
	bytesUsed := len(b)
	for _, pending := range current.Entries {
		bytesUsed += pending.Bytes
	}
	if len(current.Entries) >= 64 || bytesUsed > 256<<20 {
		return http.StatusConflict, errors.New("Paste Stack is full. Copy some items on the computer.")
	}
	if err = os.MkdirAll(a.pasteDirectory(), 0700); err == nil {
		path := filepath.Join(a.pasteDirectory(), key+".clip")
		if err = os.WriteFile(path, b, 0600); err == nil {
			next := pasteManifest{Entries: append(append([]pasteEntry(nil), current.Entries...), entry), Copied: current.Copied}
			err = a.savePasteLocked(next)
		}
		if err != nil {
			_ = os.Remove(path)
		}
	}
	if err != nil {
		return http.StatusServiceUnavailable, errors.New("The computer could not store the Paste Stack item")
	}
	return http.StatusOK, nil
}

func pastePreview(text string) string {
	var preview strings.Builder
	count := 0
	for _, ch := range strings.TrimSpace(text) {
		if count == 64 {
			break
		}
		if unicode.IsControl(ch) || unicode.IsSpace(ch) {
			ch = ' '
		}
		preview.WriteRune(ch)
		count++
	}
	return preview.String()
}

// Windows CF_DIB uses a BITMAPINFOHEADER followed by bottom-up BGR pixel rows.
func pasteDIB(data []byte) ([]byte, int, int, error) {
	cfg, _, err := image.DecodeConfig(bytes.NewReader(data))
	if err != nil || cfg.Width <= 0 || cfg.Height <= 0 || cfg.Width > pasteMaxPixels/cfg.Height {
		return nil, 0, 0, errors.New("Use a PNG or JPEG screenshot up to 12 megapixels")
	}
	bmp, _, err := image.Decode(bytes.NewReader(data))
	if err != nil {
		return nil, 0, 0, errors.New("The screenshot could not be decoded")
	}
	bounds := bmp.Bounds()
	if bounds.Dx() != cfg.Width || bounds.Dy() != cfg.Height {
		return nil, 0, 0, errors.New("Unexpected screenshot dimensions")
	}
	stride := (cfg.Width*3 + 3) &^ 3
	pixels := make([]byte, 40+stride*cfg.Height)
	binary.LittleEndian.PutUint32(pixels[0:], 40)
	binary.LittleEndian.PutUint32(pixels[4:], uint32(cfg.Width))
	binary.LittleEndian.PutUint32(pixels[8:], uint32(cfg.Height))
	binary.LittleEndian.PutUint16(pixels[12:], 1)
	binary.LittleEndian.PutUint16(pixels[14:], 24)
	binary.LittleEndian.PutUint32(pixels[20:], uint32(stride*cfg.Height))
	for y := 0; y < cfg.Height; y++ {
		row := pixels[40+y*stride:]
		for x := 0; x < cfg.Width; x++ {
			r, g, b, a := bmp.At(bounds.Min.X+x, bounds.Max.Y-1-y).RGBA()
			white := 0xffff - a
			row[x*3], row[x*3+1], row[x*3+2] = byte((b+white)>>8), byte((g+white)>>8), byte((r+white)>>8)
		}
	}
	return pixels, cfg.Width, cfg.Height, nil
}

func (a *App) changePaste(id, action string) error {
	a.paste.Lock()
	defer a.paste.Unlock()
	current := a.paste.manifest
	for i, entry := range current.Entries {
		if entry.Key != id {
			continue
		}
		next := pasteManifest{Entries: append([]pasteEntry(nil), current.Entries...), Copied: current.Copied}
		switch action {
		case "paste-copied":
			next.Entries = append(next.Entries[:i], next.Entries[i+1:]...)
			next.Copied = append(append([]string(nil), current.Copied...), id)
			if len(next.Copied) > 4096 {
				next.Copied = next.Copied[len(next.Copied)-4096:]
			}
		case "paste-up":
			if i > 0 {
				next.Entries[i], next.Entries[i-1] = next.Entries[i-1], next.Entries[i]
			}
		case "paste-down":
			if i+1 < len(next.Entries) {
				next.Entries[i], next.Entries[i+1] = next.Entries[i+1], next.Entries[i]
			}
		case "paste-skip":
			copy(next.Entries[i:], next.Entries[i+1:])
			next.Entries[len(next.Entries)-1] = entry
		}
		if err := a.savePasteLocked(next); err != nil {
			return err
		}
		if action == "paste-copied" {
			_ = os.Remove(filepath.Join(a.pasteDirectory(), id+".clip"))
		}
		return nil
	}
	return errors.New("Paste Stack item is no longer available")
}
