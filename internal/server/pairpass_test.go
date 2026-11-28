package server

import (
	"bytes"
	"encoding/base64"
	"encoding/binary"
	"net"
	"strings"
	"testing"
)

func TestPairingPassContainsTheExactLocalAddressInvitationAndPin(t *testing.T) {
	invite := bytes.Repeat([]byte{0x4d}, 16)
	pin := bytes.Repeat([]byte{0xf1}, 16)
	link := "https://192.168.1.25:8443/#" + base64.RawURLEncoding.EncodeToString(invite) + "~" + base64.RawURLEncoding.EncodeToString(pin)
	pass := pairingPass(link)
	if !strings.HasPrefix(pass, "TS1-") {
		t.Fatalf("missing pass: %q", pass)
	}
	wire, err := base64.RawURLEncoding.DecodeString(strings.TrimPrefix(pass, "TS1-"))
	if err != nil || len(wire) != 39 || wire[0] != 1 {
		t.Fatalf("bad pass data: %v", err)
	}
	if net.IP(wire[1:5]).String() != "192.168.1.25" || binary.BigEndian.Uint16(wire[5:7]) != 8443 ||
		!bytes.Equal(wire[7:23], invite) || !bytes.Equal(wire[23:], pin) {
		t.Fatal("pairing pass changed the address or authentication material")
	}
	if pairingPass(strings.Replace(link, "192.168.1.25", "127.0.0.1", 1)) != "" {
		t.Fatal("loopback is not a usable phone pairing address")
	}
}
