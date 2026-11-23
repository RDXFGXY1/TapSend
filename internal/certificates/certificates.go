package certificates

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"math/big"
	"net"
	"os"
	"path/filepath"
	"time"
)

type Paths struct{ CA, Certificate, Key string }

func Ensure(directory string) (Paths, error) {
	paths := Paths{
		CA:          filepath.Join(directory, "tapsend-ca.crt"),
		Certificate: filepath.Join(directory, "server.crt"),
		Key:         filepath.Join(directory, "server.key"),
	}
	if pair, err := tls.LoadX509KeyPair(paths.Certificate, paths.Key); err == nil {
		cert, err := x509.ParseCertificate(pair.Certificate[0])
		if err == nil && cert.PublicKeyAlgorithm == x509.ECDSA {
			return paths, nil
		}
		// Preserve the previous certificate set before migrating legacy Ed25519 TLS.
		for _, path := range []string{paths.CA, paths.Certificate, paths.Key} {
			data, err := os.ReadFile(path)
			if err != nil {
				return paths, err
			}
			if err := os.WriteFile(path+".legacy-backup", data, 0600); err != nil {
				return paths, err
			}
		}
	}
	if err := os.MkdirAll(directory, 0700); err != nil {
		return paths, err
	}

	now := time.Now()
	caPrivate, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return paths, err
	}
	caTemplate := &x509.Certificate{
		SerialNumber: randomSerial(), Subject: pkix.Name{CommonName: "TapSend Local CA"},
		NotBefore: now.Add(-time.Hour), NotAfter: now.AddDate(10, 0, 0),
		IsCA: true, BasicConstraintsValid: true,
		KeyUsage: x509.KeyUsageCertSign | x509.KeyUsageDigitalSignature,
	}
	caDER, err := x509.CreateCertificate(rand.Reader, caTemplate, caTemplate, &caPrivate.PublicKey, caPrivate)
	if err != nil {
		return paths, err
	}

	serverPrivate, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return paths, err
	}
	serverTemplate := &x509.Certificate{
		SerialNumber: randomSerial(), Subject: pkix.Name{CommonName: "TapSend"},
		NotBefore: now.Add(-time.Hour), NotAfter: now.AddDate(2, 0, 0),
		KeyUsage:    x509.KeyUsageDigitalSignature,
		ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
		DNSNames:    []string{"localhost", "tapsend.local"},
		IPAddresses: localIPs(),
	}
	serverDER, err := x509.CreateCertificate(rand.Reader, serverTemplate, caTemplate, &serverPrivate.PublicKey, caPrivate)
	if err != nil {
		return paths, err
	}

	if err := writePEM(paths.CA, "CERTIFICATE", caDER, 0644); err != nil {
		return paths, err
	}
	if err := writePEM(paths.Certificate, "CERTIFICATE", serverDER, 0644); err != nil {
		return paths, err
	}
	keyDER, err := x509.MarshalPKCS8PrivateKey(serverPrivate)
	if err != nil {
		return paths, err
	}
	if err := writePEM(paths.Key, "PRIVATE KEY", keyDER, 0600); err != nil {
		return paths, err
	}
	return paths, nil
}

func randomSerial() *big.Int {
	limit := new(big.Int).Lsh(big.NewInt(1), 128)
	n, _ := rand.Int(rand.Reader, limit)
	return n
}

func localIPs() []net.IP {
	result := []net.IP{net.ParseIP("127.0.0.1"), net.ParseIP("::1")}
	addresses, _ := net.InterfaceAddrs()
	for _, address := range addresses {
		if network, ok := address.(*net.IPNet); ok && !network.IP.IsLoopback() {
			result = append(result, network.IP)
		}
	}
	return result
}

func writePEM(path, kind string, data []byte, mode os.FileMode) error {
	return os.WriteFile(path, pem.EncodeToMemory(&pem.Block{Type: kind, Bytes: data}), mode)
}
