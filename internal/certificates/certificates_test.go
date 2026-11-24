package certificates

import (
	"bytes"
	"crypto/x509"
	"encoding/pem"
	"net"
	"os"
	"testing"
)

func TestNewNetworkIPRenewsServerCertificateAndKeepsCA(t *testing.T) {
	directory := t.TempDir()
	firstIP := net.ParseIP("192.168.1.10")
	secondIP := net.ParseIP("192.168.1.20")
	paths, err := ensureForIPs(directory, []net.IP{firstIP})
	if err != nil {
		t.Fatal(err)
	}
	firstCA, err := os.ReadFile(paths.CA)
	if err != nil {
		t.Fatal(err)
	}
	firstServer, err := os.ReadFile(paths.Certificate)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := ensureForIPs(directory, []net.IP{secondIP}); err != nil {
		t.Fatal(err)
	}
	secondCA, err := os.ReadFile(paths.CA)
	if err != nil {
		t.Fatal(err)
	}
	secondServer, err := os.ReadFile(paths.Certificate)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(firstCA, secondCA) {
		t.Fatal("changing IP replaced the trusted CA")
	}
	if bytes.Equal(firstServer, secondServer) {
		t.Fatal("server certificate did not change for the new IP")
	}
	block, _ := pem.Decode(secondServer)
	cert, err := x509.ParseCertificate(block.Bytes)
	if err != nil {
		t.Fatal(err)
	}
	if err := cert.VerifyHostname(secondIP.String()); err != nil {
		t.Fatal(err)
	}
	if _, err := ensureForIPs(directory, []net.IP{secondIP}); err != nil {
		t.Fatal(err)
	}
	unchanged, err := os.ReadFile(paths.Certificate)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(secondServer, unchanged) {
		t.Fatal("certificate changed again without a network change")
	}
}

func TestLegacyCertificateWithoutCAKeyRotatesOnNewIP(t *testing.T) {
	directory := t.TempDir()
	paths, err := ensureForIPs(directory, []net.IP{net.ParseIP("192.168.1.10")})
	if err != nil {
		t.Fatal(err)
	}
	firstCA, err := os.ReadFile(paths.CA)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(paths.CAKey); err != nil {
		t.Fatal(err)
	}
	if _, err := ensureForIPs(directory, []net.IP{net.ParseIP("192.168.1.20")}); err != nil {
		t.Fatal(err)
	}
	secondCA, err := os.ReadFile(paths.CA)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Equal(firstCA, secondCA) {
		t.Fatal("legacy CA was reused without its signing key")
	}
	if _, err := os.Stat(paths.CAKey); err != nil {
		t.Fatal("new CA signing key was not saved:", err)
	}
}
