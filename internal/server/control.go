package server

import (
	"bufio"
	"encoding/json"
	"io"
	"log"

	filecatalog "tapsend/internal/files"
)

// Desktop administration is accepted only through the parent process's stdin.
func (a *App) Control(input io.Reader) {
	scanner := bufio.NewScanner(input)
	for scanner.Scan() {
		var command struct{ Action, ID, Name string }
		if err := json.Unmarshal(scanner.Bytes(), &command); err != nil {
			log.Printf("CONTROL invalid command")
			continue
		}
		var err error
		switch command.Action {
		case "add":
			var item filecatalog.Item
			var added bool
			item, added, err = a.catalog.Add(command.Name)
			if err == nil {
				if added {
					log.Printf("FILE ADDED %q (%d bytes)", item.Name, item.Size)
				} else {
					log.Printf("FILE ALREADY SHARED %q", item.Name)
				}
			}
		case "pairing":
			log.Printf("TapSend pairing URL: %s", a.RefreshPairingURL())
			log.Printf("TapSend manual details: %s %s", a.ManualCode(), a.ManualKey())
		case "devices":
			for _, d := range a.pairing.Devices() {
				log.Printf("DEVICE %s %q", d.ID, d.Name)
			}
			for _, d := range a.pairing.Pending() {
				log.Printf("JOIN REQUEST %s %q — approve %s or deny %s", d.ID, d.Name, d.ID, d.ID)
			}
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
		case "revoke":
			err = a.pairing.Revoke(command.ID)
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
		if err != nil {
			log.Printf("CONTROL %s failed: %s", command.Action, err)
		} else {
			log.Printf("CONTROL %s applied", command.Action)
		}
	}
}
