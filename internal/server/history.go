package server

import (
	"encoding/json"
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
}

func (a *App) record(name, state string, bytes int64) {
	historyLock.Lock()
	defer historyLock.Unlock()
	path := filepath.Join(a.cfg.DataDirectory, "history.json")
	var entries []historyEntry
	data, err := os.ReadFile(path)
	if err == nil && json.Unmarshal(data, &entries) != nil {
		return
	}
	entries = append(entries, historyEntry{time.Now(), name, state, bytes})
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
		ID    string
		State string
	}
	if !decode(w, r, &receipt) {
		return
	}
	item, ok := a.catalog.Get(receipt.ID)
	if !ok {
		http.NotFound(w, r)
		return
	}
	if receipt.State != "received" && receipt.State != "failed" && receipt.State != "cancelled" {
		http.Error(w, "Invalid receipt", 400)
		return
	}
	a.record(item.Name, receipt.State, item.Size)
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
