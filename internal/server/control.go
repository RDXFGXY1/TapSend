package server

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"strings"

	filecatalog "tapsend/internal/files"
)

// Desktop administration is accepted only through the parent process's stdin.
func (a *App) Control(input io.Reader) {
	scanner := bufio.NewScanner(input)
	scanner.Buffer(make([]byte, 4096), 4*1024*1024)
	for scanner.Scan() {
		var command struct {
			Action, ID, Name, Text string
			Files                  []offerFile
		}
		if err := json.Unmarshal(scanner.Bytes(), &command); err != nil {
			log.Printf("CONTROL invalid command")
			continue
		}
		var err error
		switch command.Action {
		case "screen-open":
			err = a.armScreen(command.ID)
			if err != nil {
				log.Printf("TUI_SCREEN_ERROR %x", err.Error())
			}
		case "screen-close":
			a.closeScreen()
		case "folder-drop":
			err = a.armFolderDrop(command.ID, command.Name)
		case "paste-hold-folder":
			err = a.holdPaste(command.ID, command.Name, "", "")
		case "paste-hold-device":
			err = a.holdPaste(command.ID, "", command.Name, "")
		case "hold-file-folder":
			err = a.holdPaste("", command.ID, "", command.Name)
		case "hold-file-device":
			err = a.holdPaste("", "", command.ID, command.Name)
		case "paste-folder-opened":
			a.releasePastes(command.Name, "", false)
		case "paste-release-all":
			a.releasePastes("", "", true)
		case "paste-copied", "paste-up", "paste-down", "paste-skip":
			err = a.changePaste(command.ID, command.Action)
			if err == nil {
				log.Printf("TUI_PASTE_RESULT %s %s ok", command.ID, command.Action)
			} else {
				log.Printf("TUI_PASTE_RESULT %s %s %x", command.ID, command.Action, err.Error())
			}
		case "snippet":
			err = a.addSnippet(command.ID, "computer", command.Text)
			if err == nil {
				log.Printf("TUI_SNIPPET_RESULT ok")
			} else {
				log.Printf("TUI_SNIPPET_RESULT %x", err.Error())
			}
		case "offer":
			err = a.offerFiles(command.ID, command.Files)
			if err != nil {
				for _, file := range command.Files {
					log.Printf("TUI_SHARE request=%d state=failed error=%x", file.Request, err.Error())
				}
			}
		case "add":
			a.publishMu.Lock()
			var item filecatalog.Item
			var added bool
			item, added, err = a.catalog.Add(command.Name)
			if command.ID != "" {
				if err == nil {
					log.Printf("TUI_SHARE request=%s file=%s size=%d state=available", command.ID, item.ID, item.Size)
				} else {
					log.Printf("TUI_SHARE request=%s state=failed error=%x", command.ID, err.Error())
				}
			}
			a.publishMu.Unlock()
			if err == nil {
				if added {
					log.Printf("FILE ADDED %q (%d bytes)", item.Name, item.Size)
				} else {
					log.Printf("FILE ALREADY SHARED %q", item.Name)
				}
			}
		case "pairing":
			url := a.RefreshPairingURL()
			log.Printf("TapSend pairing URL: %s", url)
			log.Printf("TapSend manual details: %s %s", a.ManualCode(), a.ManualKey())
			if pass := pairingPass(url); pass != "" {
				log.Printf("TUI_PAIR_PASS url=%x pass=%s", url, pass)
			}
		case "devices":
			for _, d := range a.pairing.Devices() {
				log.Printf("DEVICE %s %q", d.ID, d.Name)
			}
			for _, d := range a.pairing.Pending() {
				log.Printf("JOIN REQUEST %s %q — approve %s or deny %s", d.ID, d.Name, d.ID, d.ID)
			}
			a.logDevicePicker()
		case "approve":
			err = a.pairing.Approve(command.ID)
			if err == nil {
				log.Printf("JOIN APPROVED %s", command.ID)
			}
		case "deny":
			err = a.pairing.Deny(command.ID)
			if err == nil {
				log.Printf("JOIN DENIED %s", command.ID)
			}
		case "revoke", "clear-devices":
			a.publishMu.Lock()
			all := command.Action == "clear-devices"
			if all {
				err = a.pairing.RevokeAll()
			} else {
				err = a.pairing.Revoke(command.ID)
			}
			if err == nil {
				if all {
					a.closeScreen()
				} else {
					a.revokeScreen(command.ID)
				}
				for _, offer := range a.offers {
					if all || offer.Peer == command.ID {
						offer.State = "revoked"
						for _, file := range offer.Files {
							log.Printf("TUI_OFFER file=%s peer=%x state=revoked", file.ID, offer.Peer)
						}
					}
				}
				if saveErr := a.saveOffers(); saveErr != nil {
					log.Printf("Could not update outbox: %s", saveErr)
				}
			}
			a.publishMu.Unlock()
		case "recipient":
			err = a.pairing.Select(command.ID)
		case "rename":
			err = a.pairing.Rename(command.ID, command.Name)
		case "stop":
			a.session.stop()
		case "resume":
			a.session.resume()
		case "message":
			if command.Name == "" || len(command.Name) > 2000 {
				log.Printf("CONTROL message requires 1–2000 bytes")
				continue
			}
			a.session.post("computer", command.Name)
			log.Printf("MESSAGE computer: %q", command.Name)
		default:
			log.Printf("CONTROL unknown action")
			continue
		}
		switch command.Action {
		case "approve", "deny", "rename", "revoke", "clear-devices", "folder-drop",
			"paste-hold-folder", "paste-hold-device", "hold-file-folder", "hold-file-device":
			if !strings.Contains(command.Action, "hold-") && command.Action != "folder-drop" {
				a.logDevicePicker()
			}
			if err != nil {
				log.Printf("TUI_DEVICE_RESULT %s %x", command.Action, err.Error())
			} else {
				log.Printf("TUI_DEVICE_RESULT %s ok", command.Action)
			}
		}
		if err != nil {
			log.Printf("CONTROL %s failed: %s", command.Action, err)
		} else {
			log.Printf("CONTROL %s applied", command.Action)
		}
	}
}

func (a *App) logDevicePicker() {
	var snapshot strings.Builder
	pending := make(map[string]bool)
	for _, device := range a.pairing.Pending() {
		pending[device.ID] = true
	}
	for _, device := range a.pairing.Devices() {
		fmt.Fprintf(&snapshot, " %x:%x:%t", device.ID, device.Name, pending[device.ID])
	}
	log.Printf("TUI_RECIPIENTS%s", snapshot.String())
}
