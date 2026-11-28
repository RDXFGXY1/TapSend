package server

import (
	"encoding/json"
	"net"
	"net/http"
	"os"
	"path/filepath"

	"tapsend/internal/qr"
)

// Portal controls and arrival details are visible only in a browser on the computer.
func (a *App) portalOnly(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if !a.cfg.PortalEnabled {
			http.Error(w, "Portal Mode is disabled in Settings", http.StatusNotFound)
			return
		}
		next(w, r)
	}
}

func (a *App) localOnly(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		host, _, err := net.SplitHostPort(r.RemoteAddr)
		if err != nil || !net.ParseIP(host).IsLoopback() {
			http.Error(w, "Open Portal Mode on the computer", http.StatusForbidden)
			return
		}
		next(w, r)
	}
}

func (a *App) portalPairingURL() string {
	if a.pairing.PairingExpired() {
		return a.RefreshPairingURL()
	}
	return a.PairingURL()
}

func (a *App) handlePortal(w http.ResponseWriter, r *http.Request) {
	path := filepath.Join(a.cfg.DataDirectory, "history.json")
	historyLock.Lock()
	data, err := os.ReadFile(path)
	historyLock.Unlock()
	var entries []historyEntry
	if err == nil {
		_ = json.Unmarshal(data, &entries)
	}
	var arrivals []historyEntry
	for _, entry := range entries {
		if entry.State == "saved-to-inbox" {
			arrivals = append(arrivals, entry)
		}
	}
	if len(arrivals) > 20 {
		arrivals = arrivals[len(arrivals)-20:]
	}
	if arrivals == nil {
		arrivals = []historyEntry{}
	}
	writeJSON(w, map[string]any{"pairingURL": a.portalPairingURL(), "arrivals": arrivals})
}

func (a *App) handlePortalQR(w http.ResponseWriter, r *http.Request) {
	output, err := qr.PNG(a.portalPairingURL())
	if err != nil {
		http.Error(w, "QR unavailable; use the pairing link", http.StatusServiceUnavailable)
		return
	}
	w.Header().Set("Content-Type", "image/png")
	_, _ = w.Write(output)
}

func (a *App) handlePortalRefresh(w http.ResponseWriter, r *http.Request) {
	if origin := r.Header.Get("Origin"); origin != "" && origin != "https://"+r.Host {
		http.Error(w, "Origin rejected", http.StatusForbidden)
		return
	}
	writeJSON(w, map[string]string{"pairingURL": a.RefreshPairingURL()})
}
