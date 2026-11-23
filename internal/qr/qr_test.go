package qr

import (
	"bytes"
	"image/png"
	"strings"
	"testing"
)

func TestPairingQRHasImageAndTerminalForms(t *testing.T) {
	value := "https://192.0.2.10:8443/#pairing-code~certificate-pin"
	data, err := PNG(value)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(data))
	if err != nil || image.Bounds().Dx() < 100 {
		t.Fatalf("invalid QR PNG: %v", err)
	}
	terminal, err := Terminal(value)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(terminal, "\x1b[47m") || !strings.Contains(terminal, "█") || !strings.Contains(terminal, "\x1b[0m") {
		t.Fatal("QR terminal rendering is missing modules or reset sequences")
	}
}
