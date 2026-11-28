package server

import (
	"bytes"
	"errors"
	"log"
	"net/http"
	"strings"
)

type snippet struct {
	ID   uint64 `json:"id"`
	From string `json:"from"`
	Text string `json:"text"`
}

type storedSnippet struct {
	snippet
	Peer    string
	PeerKey []byte
}

func (a *App) restoreScratchpad() error {
	return readState(a.cfg.DataDirectory, "scratchpad.json", &a.scratchpad)
}

func (a *App) reportScratchpad() {
	a.scratchMu.Lock()
	defer a.scratchMu.Unlock()
	for _, entry := range a.scratchpad {
		device, ok := a.pairing.Device(entry.Peer)
		if ok && bytes.Equal(device.PublicKey, entry.PeerKey) {
			logSnippet(entry, device.Name)
		}
	}
}

func logSnippet(entry storedSnippet, name string) {
	log.Printf("TUI_SNIPPET from=%s device=%x text=%x", entry.From, name, entry.Text)
}

func (a *App) addSnippet(peer, from, text string) error {
	if strings.TrimSpace(text) == "" || len(text) > 2000 || strings.ContainsRune(text, '\x00') {
		return errors.New("use 1–2000 bytes of text")
	}
	device, ok := a.pairing.Device(peer)
	if !ok {
		return errors.New("choose a paired device")
	}
	a.scratchMu.Lock()
	defer a.scratchMu.Unlock()
	id := uint64(1)
	if len(a.scratchpad) > 0 {
		id = a.scratchpad[len(a.scratchpad)-1].ID + 1
	}
	entry := storedSnippet{snippet: snippet{ID: id, From: from, Text: text}, Peer: peer, PeerKey: device.PublicKey}
	keep := a.scratchpad
	if len(keep) >= 100 {
		keep = keep[len(keep)-99:]
	}
	next := append(append([]storedSnippet(nil), keep...), entry)
	if err := writeState(a.cfg.DataDirectory, "scratchpad.json", next); err != nil {
		return err
	}
	a.scratchpad = next
	logSnippet(entry, device.Name)
	return nil
}

func (a *App) handleScratchpad(w http.ResponseWriter, r *http.Request) {
	peer := a.requestPeer(r)
	device, _ := a.pairing.Device(peer)
	a.scratchMu.Lock()
	rows := make([]snippet, 0)
	for _, entry := range a.scratchpad {
		if entry.Peer == peer && bytes.Equal(device.PublicKey, entry.PeerKey) {
			rows = append(rows, entry.snippet)
		}
	}
	a.scratchMu.Unlock()
	if len(rows) > 12 {
		rows = rows[len(rows)-12:]
	}
	writeJSON(w, rows)
}

func (a *App) handleScratchpadPost(w http.ResponseWriter, r *http.Request) {
	var value struct{ Text string }
	if !decode(w, r, &value) {
		return
	}
	if err := a.addSnippet(a.requestPeer(r), "phone", value.Text); err != nil {
		http.Error(w, "Use 1–2000 bytes of text, or check storage on the computer.", http.StatusBadRequest)
		return
	}
	writeJSON(w, map[string]bool{"ok": true})
}
