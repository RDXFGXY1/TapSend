package relay

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestOfflineRelayAndAuthenticatedReceipt(t *testing.T) {
	destination, sender, shared, inbox := t.TempDir(), t.TempDir(), t.TempDir(), t.TempDir()
	pass, err := Create(destination, shared)
	if err != nil {
		t.Fatal(err)
	}
	if err := Connect(sender, pass); err != nil {
		t.Fatal(err)
	}
	original := filepath.Join(sender, "private-note.txt")
	body := bytes.Repeat([]byte("private body of the document\n"), 16000)
	if err := os.WriteFile(original, body, 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := Send(sender, original); err != nil {
		t.Fatal(err)
	}
	check := func(want string) {
		t.Helper()
		state, err := Status(sender)
		if err != nil || !strings.Contains(state, want) {
			t.Fatalf("status %q, want %q, error %v", state, want, err)
		}
	}
	check("holding")
	r, err := loadRoute(sender, "sender")
	if err != nil {
		t.Fatal(err)
	}
	var d delivery
	if err := load(sender, "relay-delivery.json", &d); err != nil {
		t.Fatal(err)
	}
	staged := filepath.Join(r.Folder, d.ID+".relay")
	ciphertext, err := os.ReadFile(staged)
	if err != nil || bytes.Contains(ciphertext, []byte("private")) {
		t.Fatalf("relay had plaintext or could not be read: %v", err)
	}
	receipt := filepath.Join(r.Folder, d.ID+".saved")
	if err := os.WriteFile(receipt, []byte("fake receipt"), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := Status(sender); err == nil {
		t.Fatal("accepted a fabricated save receipt")
	}
	if err := os.Remove(receipt); err != nil {
		t.Fatal(err)
	}
	var name string
	Collect(destination, inbox, func(file string) { name = file })
	data, err := os.ReadFile(filepath.Join(inbox, "Relay", name))
	if err != nil || !bytes.Equal(data, body) {
		t.Fatalf("destination did not save the original file: %v", err)
	}
	check("SAVED")
	if _, err := os.Stat(staged); !os.IsNotExist(err) {
		t.Fatal("relay retained the payload after acknowledgement")
	}
}
