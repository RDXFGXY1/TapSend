package server

import (
	"os"
	"path/filepath"
	"testing"

	"tapsend/internal/config"
	"tapsend/internal/devices"
	"tapsend/internal/sendstatus"
)

func TestTargetedSendRequiresPairedDeviceAndRecordsOffer(t *testing.T) {
	directory := t.TempDir()
	dataDirectory := filepath.Join(directory, "data")
	path := filepath.Join(directory, "photo.jpg")
	if err := os.WriteFile(path, []byte("photo"), 0600); err != nil {
		t.Fatal(err)
	}
	store, err := devices.Open(dataDirectory)
	if err != nil {
		t.Fatal(err)
	}
	if err := store.Put(devices.Device{ID: "phone-id", Name: "My Phone"}); err != nil {
		t.Fatal(err)
	}
	app, err := New(config.Config{ListenAddress: ":8443", DataDirectory: dataDirectory, SharedFiles: []string{path}})
	if err != nil {
		t.Fatal(err)
	}
	if err := app.SelectRecipient("unknown", "Unknown"); err == nil {
		t.Fatal("unpaired device accepted")
	}
	if err := app.SelectRecipient("phone-id", "My Phone"); err != nil {
		t.Fatal(err)
	}
	state, err := sendstatus.Read(dataDirectory)
	if err != nil {
		t.Fatal(err)
	}
	if state.Device != "My Phone" || state.State != "waiting for device" || len(state.Files) != 1 || state.Files[0] != "photo.jpg" {
		t.Fatalf("unexpected send status: %+v", state)
	}
}
