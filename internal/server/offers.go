package server

import (
	"bytes"
	"errors"
	"fmt"
	"log"
	"net/http"
	"os"
	"strings"
	"time"

	filecatalog "tapsend/internal/files"
)

type offerFile struct {
	Request uint64
	Path    string
}

type storedFile struct {
	filecatalog.Item
	Modified time.Time
	State    string
}

type fileOffer struct {
	ID      string
	Peer    string
	PeerKey []byte
	State   string
	Seen    bool
	Files   []storedFile
}

type publicOffer struct {
	ID    string                   `json:"id"`
	Files []filecatalog.PublicItem `json:"files"`
}

func (offer *fileOffer) public() publicOffer {
	result := publicOffer{ID: offer.ID, Files: make([]filecatalog.PublicItem, 0, len(offer.Files))}
	for _, file := range offer.Files {
		result.Files = append(result.Files, filecatalog.PublicItem{ID: file.ID, Name: file.Name, Size: file.Size})
	}
	return result
}

func (a *App) saveOffers() error {
	return writeState(a.cfg.DataDirectory, "outbox.json", a.offers)
}

func (a *App) restoreOffers() error {
	var stored []*fileOffer
	if err := readState(a.cfg.DataDirectory, "outbox.json", &stored); err != nil {
		return err
	}
	for _, offer := range stored {
		if offer == nil {
			continue
		}
		device, ok := a.pairing.Device(offer.Peer)
		if ok && bytes.Equal(device.PublicKey, offer.PeerKey) {
			a.offers = append(a.offers, offer)
		}
	}
	return nil
}

func (a *App) reportRestoredOffers() {
	a.publishMu.Lock()
	defer a.publishMu.Unlock()
	for _, offer := range a.offers {
		device, ok := a.pairing.Device(offer.Peer)
		if !ok {
			continue
		}
		for _, file := range offer.Files {
			state := file.State
			if state == "" {
				state = offer.State
				if state == "pending" && !offer.Seen {
					state = "waiting"
				}
			}
			log.Printf("TUI_RESTORE file=%s peer=%x name=%x path=%x size=%d state=%s",
				file.ID, offer.Peer, device.Name, file.Path, file.Size, state)
		}
	}
}

func (a *App) offerFiles(peer string, files []offerFile) error {
	device, ok := a.pairing.Device(peer)
	if !ok {
		return errors.New("device is no longer paired")
	}
	if len(files) == 0 {
		return errors.New("choose at least one file")
	}
	items, err := filecatalog.New(nil)
	if err != nil {
		return err
	}
	entries := make([]storedFile, 0, len(files))
	for _, file := range files {
		item, added, err := items.Add(file.Path)
		if err != nil {
			return fmt.Errorf("%s: %w", file.Path, err)
		}
		if !added {
			return errors.New("the same file was selected twice")
		}
		info, err := os.Stat(item.Path)
		if err != nil || info.Size() != item.Size || !info.Mode().IsRegular() {
			return fmt.Errorf("file changed while being queued: %s", item.Name)
		}
		entries = append(entries, storedFile{Item: item, Modified: info.ModTime()})
	}
	// Publish the files and their acknowledgements together, before the phone can respond.
	a.publishMu.Lock()
	defer a.publishMu.Unlock()
	offer := &fileOffer{ID: entries[0].ID, Files: entries, Peer: peer, PeerKey: device.PublicKey, State: "pending"}
	previous := a.offers
	// Keep active offers and a short list of completed offers for the queue.
	a.offers = make([]*fileOffer, 0, len(previous)+1)
	for i, old := range previous {
		if i >= len(previous)-50 || old.active() {
			a.offers = append(a.offers, old)
		}
	}
	a.offers = append(a.offers, offer)
	if err := a.saveOffers(); err != nil {
		a.offers = previous
		return fmt.Errorf("could not save the outbox: %w", err)
	}
	for i, item := range entries {
		log.Printf("TUI_SHARE request=%d file=%s size=%d state=waiting", files[i].Request, item.ID, item.Size)
	}
	return nil
}

func (a *App) requestPeer(r *http.Request) string {
	token := strings.TrimPrefix(r.Header.Get("Authorization"), "Bearer ")
	if token == "" {
		return ""
	}
	return a.pairing.DeviceID(token)
}

func (offer *fileOffer) active() bool {
	if offer.State == "pending" {
		return true
	}
	if offer.State == "accepted" {
		for _, file := range offer.Files {
			if file.State != "received" && file.State != "unconfirmed" {
				return true
			}
		}
	}
	return false
}

func (a *App) pendingOffers(peer string) []publicOffer {
	a.publishMu.Lock()
	defer a.publishMu.Unlock()
	result := make([]publicOffer, 0)
	for _, offer := range a.offers {
		if offer.Peer == peer && offer.State == "pending" {
			result = append(result, offer.public())
			if !offer.Seen {
				offer.Seen = true
				for _, file := range offer.Files {
					log.Printf("TUI_OFFER file=%s peer=%x state=seen", file.ID, peer)
				}
			}
		}
	}
	// The seen indicator is informational. Acceptance, decline and receipts
	// are persisted immediately. A fresh poll can restore this indicator.
	return result
}

func (a *App) handleOfferDecision(w http.ResponseWriter, r *http.Request) {
	var decision struct{ State string }
	if !decode(w, r, &decision) {
		return
	}
	if decision.State != "accepted" && decision.State != "declined" {
		http.Error(w, "Choose accept or decline", http.StatusBadRequest)
		return
	}
	peer, id := a.requestPeer(r), r.PathValue("id")
	a.publishMu.Lock()
	defer a.publishMu.Unlock()
	for _, offer := range a.offers {
		if offer.ID != id || offer.Peer != peer || offer.State == "revoked" {
			continue
		}
		if offer.State != "pending" && offer.State != decision.State {
			http.Error(w, "This offer has already been answered", http.StatusConflict)
			return
		}
		if offer.State == "pending" {
			offer.State = decision.State
			if err := a.saveOffers(); err != nil {
				offer.State = "pending"
				http.Error(w, "The computer could not save your response. Try again.", http.StatusServiceUnavailable)
				return
			}
			for _, file := range offer.Files {
				log.Printf("TUI_OFFER file=%s peer=%x state=%s", file.ID, peer, offer.State)
			}
		}
		writeJSON(w, map[string]bool{"ok": true})
		return
	}
	http.NotFound(w, r)
}

func (a *App) saveOfferReceipt(id, peer, state string) error {
	a.publishMu.Lock()
	defer a.publishMu.Unlock()
	for _, offer := range a.offers {
		if offer.Peer != peer || offer.State != "accepted" {
			continue
		}
		for i := range offer.Files {
			file := &offer.Files[i]
			if file.ID != id {
				continue
			}
			if file.State == "received" || file.State == state {
				return nil
			}
			previous := file.State
			file.State = state
			if err := a.saveOffers(); err != nil {
				file.State = previous
				return err
			}
			return nil
		}
	}
	return nil
}
