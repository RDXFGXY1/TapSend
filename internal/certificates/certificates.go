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

type Paths struct{ CA, CAKey, Certificate, Key string }

func Ensure(directory string) (Paths, error) {
	return ensureForIPs(directory, localIPs())
}

func ensureForIPs(directory string, ips []net.IP) (Paths, error) {
	paths := Paths{
		CA:          filepath.Join(directory, "tapsend-ca.crt"),
		CAKey:       filepath.Join(directory, "tapsend-ca.key"),
		Certificate: filepath.Join(directory, "server.crt"),
		Key:         filepath.Join(directory, "server.key"),
	}
	if pair, err := tls.LoadX509KeyPair(paths.Certificate, paths.Key); err == nil {
		cert, err := x509.ParseCertificate(pair.Certificate[0])
		if err == nil && cert.PublicKeyAlgorithm == x509.ECDSA && coversIPs(cert, ips) {
			return paths, nil
		}
		// Keep the old trust material when a changed network or legacy key requires replacement.
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
	caPrivate, caCert := loadCA(paths)
	if caPrivate == nil {
		var err error
		caPrivate, err = ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
		if err != nil {
			return paths, err
		}
	}
	caTemplate := &x509.Certificate{
		SerialNumber: randomSerial(), Subject: pkix.Name{CommonName: "TapSend Local CA"},
		NotBefore: now.Add(-time.Hour), NotAfter: now.AddDate(10, 0, 0),
		IsCA: true, BasicConstraintsValid: true,
		KeyUsage: x509.KeyUsageCertSign | x509.KeyUsageDigitalSignature,
	}
	if caCert != nil {
		caTemplate = caCert
	} else {
		caDER, err := x509.CreateCertificate(rand.Reader, caTemplate, caTemplate, &caPrivate.PublicKey, caPrivate)
		if err != nil {
			return paths, err
		}
		if err := writePEM(paths.CA, "CERTIFICATE", caDER, 0644); err != nil {
			return paths, err
		}
		keyDER, err := x509.MarshalPKCS8PrivateKey(caPrivate)
		if err != nil {
			return paths, err
		}
		if err := writePEM(paths.CAKey, "PRIVATE KEY", keyDER, 0600); err != nil {
			return paths, err
		}
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
		IPAddresses: ips,
	}
	serverDER, err := x509.CreateCertificate(rand.Reader, serverTemplate, caTemplate, &serverPrivate.PublicKey, caPrivate)
	if err != nil {
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

func coversIPs(cert *x509.Certificate, ips []net.IP) bool {
	for _, ip := range ips {
		if cert.VerifyHostname(ip.String()) != nil {
			return false
		}
	}
	return true
}

func loadCA(paths Paths) (*ecdsa.PrivateKey, *x509.Certificate) {
	keyPEM, keyErr := os.ReadFile(paths.CAKey)
	certPEM, certErr := os.ReadFile(paths.CA)
	if keyErr != nil || certErr != nil {
		return nil, nil
	}
	keyBlock, _ := pem.Decode(keyPEM)
	certBlock, _ := pem.Decode(certPEM)
	if keyBlock == nil || certBlock == nil {
		return nil, nil
	}
	keyAny, keyErr := x509.ParsePKCS8PrivateKey(keyBlock.Bytes)
	cert, certErr := x509.ParseCertificate(certBlock.Bytes)
	key, ok := keyAny.(*ecdsa.PrivateKey)
	if keyErr != nil || certErr != nil || !ok || !cert.IsCA || !key.PublicKey.Equal(cert.PublicKey) {
		return nil, nil
	}
	return key, cert
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
		if network, ok := address.(*net.IPNet); ok {
			if ip := network.IP.To4(); ip != nil && ip.IsPrivate() {
				result = append(result, ip)
			}
		}
	}
	return result
}

func writePEM(path, kind string, data []byte, mode os.FileMode) error {
	return os.WriteFile(path, pem.EncodeToMemory(&pem.Block{Type: kind, Bytes: data}), mode)
}
