package server

import (
	"crypto/sha256"
	"encoding/base64"
	"encoding/pem"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"tapsend/internal/config"
)

func TestPairingURLCarriesCurrentCertificatePin(t *testing.T) {
	directory := t.TempDir()
	file := filepath.Join(directory, "sample.txt")
	if err := os.WriteFile(file, []byte("sample"), 0600); err != nil {
		t.Fatal(err)
	}
	app, err := New(config.Config{ListenAddress: ":8443", DataDirectory: filepath.Join(directory, "data"), SharedFiles: []string{file}})
	if err != nil {
		t.Fatal(err)
	}
	first, err := url.Parse(app.PairingURL())
	if err != nil {
		t.Fatal(err)
	}
	firstManualCode := app.ManualCode()
	second, err := url.Parse(app.RefreshPairingURL())
	if err != nil {
		t.Fatal(err)
	}
	if current := app.PairingURL(); current != second.String() {
		t.Fatalf("current pairing URL was not updated: %q != %q", current, second.String())
	}
	parts := strings.Split(first.Fragment, "~")
	newParts := strings.Split(second.Fragment, "~")
	if len(parts) != 2 || len(newParts) != 2 || len(parts[0]) != 22 || parts[0] == newParts[0] || parts[1] != newParts[1] {
		t.Fatalf("unexpected pairing URL fragments: %q and %q", first.Fragment, second.Fragment)
	}
	certificate, err := os.ReadFile(app.certificates.Certificate)
	if err != nil {
		t.Fatal(err)
	}
	block, _ := pem.Decode(certificate)
	if block == nil {
		t.Fatal("server certificate is not PEM")
	}
	digest := sha256.Sum256(block.Bytes)
	if parts[1] != base64.RawURLEncoding.EncodeToString(digest[:16]) {
		t.Fatal("pairing URL pin does not match server certificate")
	}
	if strings.ReplaceAll(app.ManualKey(), "-", "") != shortFingerprint(digest[:8]) {
		t.Fatal("manual key does not match server certificate")
	}
	if len(strings.ReplaceAll(app.ManualCode(), "-", "")) != 8 || app.ManualCode() == firstManualCode {
		t.Fatal("manual code was not refreshed")
	}
}

func TestManualFingerprintEncoding(t *testing.T) {
	if got := shortFingerprint([]byte{0, 1, 2, 3, 4, 5, 6, 7}); got != "000G40R40M30E" {
		t.Fatalf("manual key encoding changed: %s", got)
	}
}

func TestPairingHostMustMatchCertificate(t *testing.T) {
	cfg := config.Default()
	cfg.DataDirectory = t.TempDir()
	cfg.AdvertiseHost = "unlisted.example"
	if _, err := New(cfg); err == nil || !strings.Contains(err.Error(), "not covered") {
		t.Fatalf("expected certificate mismatch, got %v", err)
	}
	cfg.AdvertiseHost = "127.0.0.1"
	app, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(app.PairingURL(), "127.0.0.1:8443") {
		t.Fatalf("wrong pairing host: %s", app.PairingURL())
	}
}
