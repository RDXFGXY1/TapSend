package server

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestShareAPKServesOnlyTheQRDownload(t *testing.T) {
	const payload = "test APK content"
	path := filepath.Join(t.TempDir(), "build.apk")
	if err := os.WriteFile(path, []byte(payload), 0o600); err != nil {
		t.Fatal(err)
	}
	ctx, stop := context.WithCancel(context.Background())
	defer stop()
	output := make(chan string, 1)
	finished := make(chan error, 1)
	go func() {
		finished <- ShareAPK(ctx, path, "127.0.0.1", apkTestOutput(output))
	}()
	var link, ca string
	select {
	case printed := <-output:
		lines := strings.Split(printed, "\n")
		for _, line := range lines {
			if value, ok := strings.CutPrefix(line, "Download: "); ok {
				link = value
			}
			if value, ok := strings.CutPrefix(line, "Local CA certificate: "); ok {
				ca = value
			}
		}
	case err := <-finished:
		t.Fatalf("share exited before printing the QR: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("share did not print the QR")
	}
	certificate, err := os.ReadFile(ca)
	if err != nil {
		t.Fatal(err)
	}
	pool := x509.NewCertPool()
	if !pool.AppendCertsFromPEM(certificate) {
		t.Fatal("could not trust test CA")
	}
	client := &http.Client{
		Transport: &http.Transport{TLSClientConfig: &tls.Config{RootCAs: pool}},
		Timeout:   5 * time.Second,
	}
	defer client.CloseIdleConnections()
	get := func(url string, status int) (*http.Response, string) {
		t.Helper()
		response, err := client.Get(url)
		if err != nil {
			t.Fatal(err)
		}
		defer response.Body.Close()
		data, err := io.ReadAll(response.Body)
		if err != nil || response.StatusCode != status {
			t.Fatalf("GET %s got HTTP %d, read error %v", url, response.StatusCode, err)
		}
		return response, string(data)
	}
	response, data := get(link, http.StatusOK)
	if data != payload || response.Header.Get("Content-Type") != "application/vnd.android.package-archive" {
		t.Fatal("APK response contents or content type did not match")
	}
	get(strings.TrimSuffix(link, "TapSend-Android-debug.apk")+"other.apk", http.StatusNotFound)
	stop()
	select {
	case err := <-finished:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("share did not stop")
	}
}

type apkTestOutput chan string

func (out apkTestOutput) Write(data []byte) (int, error) {
	select {
	case out <- string(data):
		return len(data), nil
	default:
		return 0, fmt.Errorf("unexpected extra output")
	}
}
