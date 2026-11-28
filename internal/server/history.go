package server

import (
	"encoding/json"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"sync"
	"time"
)

var historyLock sync.Mutex

type historyEntry struct {
	Time  time.Time `json:"time"`
	Name  string    `json:"name"`
	State string    `json:"state"`
	Bytes int64     `json:"bytes"`
	From  string    `json:"from,omitempty"`
}

func (a *App) record(name, state string, bytes int64) {
	a.recordFrom(name, state, bytes, "")
}

func (a *App) recordFrom(name, state string, bytes int64, from string) {
	historyLock.Lock()
	defer historyLock.Unlock()
	path := filepath.Join(a.cfg.DataDirectory, "history.json")
	var entries []historyEntry
	data, err := os.ReadFile(path)
	if err == nil && json.Unmarshal(data, &entries) != nil {
		return
	}
	entries = append(entries, historyEntry{Time: time.Now(), Name: name, State: state, Bytes: bytes, From: from})
	if len(entries) > 500 {
		entries = entries[len(entries)-500:]
	}
	data, err = json.MarshalIndent(entries, "", "  ")
	if err != nil {
		return
	}
	temporary, err := os.CreateTemp(a.cfg.DataDirectory, "history-*")
	if err != nil {
		return
	}
	defer os.Remove(temporary.Name())
	if _, err = temporary.Write(data); err != nil {
		temporary.Close()
		return
	}
	if temporary.Close() != nil {
		return
	}
	_ = os.Rename(temporary.Name(), path)
}

func (a *App) handleReceipt(w http.ResponseWriter, r *http.Request) {
	var receipt struct {
		ID              string
		State           string
		SaveUnconfirmed bool
	}
	if !decode(w, r, &receipt) {
		return
	}
	peer := a.requestPeer(r)
	item, ok := a.sharedItem(receipt.ID, peer)
	if !ok {
		http.NotFound(w, r)
		return
	}
	if receipt.State != "received" && receipt.State != "failed" && receipt.State != "cancelled" {
		http.Error(w, "Invalid receipt", 400)
		return
	}
	state := receipt.State
	if receipt.State == "received" && receipt.SaveUnconfirmed {
		state = "unconfirmed"
	}
	if err := a.saveOfferReceipt(item.ID, peer, state); err != nil {
		http.Error(w, "The computer could not save your receipt. Try again.", http.StatusServiceUnavailable)
		return
	}
	a.record(item.Name, receipt.State, item.Size)
	log.Printf("TUI_RECEIPT file=%s peer=%x state=%s", item.ID, peer, state)
	if a.targeted {
		a.statusMu.Lock()
		if receipt.State == "received" {
			a.received[item.ID] = true
		}
		complete := len(a.received) == len(a.catalog.Public())
		a.statusMu.Unlock()
		if complete {
			a.setStatus("saved")
			a.doneOnce.Do(func() { close(a.done) })
		} else if receipt.State != "received" {
			a.setStatus(receipt.State)
		}
	}
	writeJSON(w, map[string]bool{"ok": true})
}
