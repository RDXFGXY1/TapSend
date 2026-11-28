package server

import (
	"encoding/base64"
	"encoding/binary"
	"net"
	"net/url"
	"strconv"
	"strings"
)

// A single pasteable pass carries the same address, invitation and pinned
// certificate as the QR. Its expiry is enforced by the existing pairing code.
func pairingPass(link string) string {
	parsed, err := url.Parse(link)
	if err != nil || parsed.Scheme != "https" {
		return ""
	}
	host, portValue, err := net.SplitHostPort(parsed.Host)
	if err != nil {
		return ""
	}
	ip := net.ParseIP(host)
	port, err := strconv.ParseUint(portValue, 10, 16)
	if err != nil || port == 0 || ip.To4() == nil || !ip.IsPrivate() {
		return ""
	}
	code, pk, ok := strings.Cut(parsed.Fragment, "~")
	invite, err := base64.RawURLEncoding.DecodeString(code)
	pin, pinErr := base64.RawURLEncoding.DecodeString(pk)
	if !ok || err != nil || pinErr != nil || len(invite) != 16 || len(pin) != 16 {
		return ""
	}
	var data [39]byte
	data[0] = 1
	copy(data[1:5], ip.To4())
	binary.BigEndian.PutUint16(data[5:7], uint16(port))
	copy(data[7:23], invite)
	copy(data[23:], pin)
	return "TS1-" + base64.RawURLEncoding.EncodeToString(data[:])
}
