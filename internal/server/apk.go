package server

import (
	"context"
	"crypto/rand"
	"crypto/tls"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"tapsend/internal/certificates"
	"tapsend/internal/qr"
)

// ShareAPK serves a single build over a private link until the installer is closed.
func ShareAPK(ctx context.Context, path, host string, output io.Writer) error {
	if !strings.EqualFold(filepath.Ext(path), ".apk") {
		return errors.New("choose an .apk file to share")
	}
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() || info.Size() == 0 {
		return errors.New("choose a nonempty APK file")
	}

	if host == "" {
		host, _, _ = net.SplitHostPort(localHost(":0", ""))
	}
	if ip := net.ParseIP(host); ip.To4() == nil || !(ip.IsPrivate() || ip.IsLoopback()) {
		return errors.New("no private IPv4 address for the APK share; connect to your local network or use -apk-host")
	}
	certDir, err := os.MkdirTemp("", "tapsend-apk-")
	if err != nil {
		return err
	}
	defer os.RemoveAll(certDir)
	keys, err := certificates.Ensure(certDir)
	if err != nil {
		return err
	}
	listener, err := net.Listen("tcp4", net.JoinHostPort(host, "0"))
	if err != nil {
		return err
	}
	defer listener.Close()

	downloadPath := "/" + rand.Text() + "/TapSend-Android-debug.apk"
	link := "https://" + listener.Addr().String() + downloadPath
	picture, err := qr.Terminal(link)
	if err != nil {
		return err
	}
	mux := http.NewServeMux()
	mux.HandleFunc("GET "+downloadPath, func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/vnd.android.package-archive")
		w.Header().Set("Content-Disposition", "attachment; filename=TapSend-Android-debug.apk")
		http.ServeContent(w, r, filepath.Base(path), info.ModTime(), io.NewSectionReader(file, 0, info.Size()))
	})
	web := &http.Server{
		Handler: securityHeaders(mux), ReadHeaderTimeout: 5 * time.Second,
		TLSConfig: &tls.Config{MinVersion: tls.VersionTLS12},
	}
	cancel := context.AfterFunc(ctx, func() { _ = web.Close() })
	defer cancel()
	if _, err := fmt.Fprintf(output, "\n%s\nDownload: %s\nLocal CA certificate: %s\nPress Ctrl+C to stop sharing.\n", picture, link, keys.CA); err != nil {
		return err
	}
	if err := web.ServeTLS(listener, keys.Certificate, keys.Key); !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
}
