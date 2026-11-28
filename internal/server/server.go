package server

import (
	"context"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"embed"
	"encoding/base64"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"io"
	"io/fs"
	"log"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"tapsend/internal/certificates"
	"tapsend/internal/config"
	"tapsend/internal/devices"
	filecatalog "tapsend/internal/files"
	"tapsend/internal/pairing"
	"tapsend/internal/relay"
	"tapsend/internal/sendstatus"
)

//go:embed web/*
var webFiles embed.FS

type App struct {
	cfg          config.Config
	certificates certificates.Paths
	catalog      *filecatalog.Catalog
	publishMu    sync.Mutex
	offers       []*fileOffer
	scratchMu    sync.Mutex
	scratchpad   []storedSnippet
	paste        pasteStack
	drop         folderDrop
	screen       liveScreen
	screenPin    string
	pairing      *pairing.Manager
	pairingCode  string
	pairingMu    sync.Mutex
	tlsPin       string
	manualKey    string
	host         string
	session      sessionState
	targeted     bool
	targetID     string
	targetName   string
	statusMu     sync.Mutex
	received     map[string]bool
	done         chan struct{}
	doneOnce     sync.Once
}

func New(cfg config.Config) (*App, error) {
	certs, err := certificates.Ensure(cfg.DataDirectory)
	if err != nil {
		return nil, err
	}
	catalog, err := filecatalog.New(cfg.SharedFiles)
	if err != nil {
		return nil, err
	}
	store, err := devices.Open(cfg.DataDirectory)
	if err != nil {
		return nil, err
	}
	manager := pairing.New(store)
	manager.SetDirectJoinLimit(cfg.DirectJoinLimit)
	certificate, err := os.ReadFile(certs.Certificate)
	if err != nil {
		return nil, err
	}
	block, _ := pem.Decode(certificate)
	if block == nil {
		return nil, fmt.Errorf("invalid server certificate PEM")
	}
	if cfg.AdvertiseHost != "" {
		parsed, err := x509.ParseCertificate(block.Bytes)
		if err != nil {
			return nil, fmt.Errorf("parse server certificate: %w", err)
		}
		if err := parsed.VerifyHostname(cfg.AdvertiseHost); err != nil {
			return nil, fmt.Errorf("pairing host %q is not covered by the current server certificate: %w", cfg.AdvertiseHost, err)
		}
	}
	digest := sha256.Sum256(block.Bytes)
	app := &App{cfg: cfg, certificates: certs, catalog: catalog, pairing: manager,
		pairingCode: manager.NewPairingCode(), host: localHost(cfg.ListenAddress, cfg.AdvertiseHost), tlsPin: base64.RawURLEncoding.EncodeToString(digest[:16]),
		manualKey: shortFingerprint(digest[:8]), screenPin: fmt.Sprintf("%x", digest)}
	if err := app.restoreOffers(); err != nil {
		return nil, fmt.Errorf("could not read the outbox: %w", err)
	}
	if err := app.restoreScratchpad(); err != nil {
		return nil, fmt.Errorf("could not read the scratchpad: %w", err)
	}
	if err := app.restorePasteStack(); err != nil {
		return nil, fmt.Errorf("could not read the Paste Stack: %w", err)
	}
	return app, nil
}
func (a *App) CAPath() string { return a.certificates.CA }
func (a *App) ManualCode() string {
	code := a.pairing.ManualCode()
	return code[:4] + "-" + code[4:]
}
func (a *App) ManualKey() string {
	return a.manualKey[:4] + "-" + a.manualKey[4:8] + "-" + a.manualKey[8:]
}

const manualAlphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"

func shortFingerprint(digest []byte) string {
	var key [13]byte
	for i := 0; i < len(key); i++ {
		value := 0
		for bit := 0; bit < 5; bit++ {
			position := i*5 + bit
			if position < 64 && digest[position/8]&(1<<(7-position%8)) != 0 {
				value |= 1 << (4 - bit)
			}
		}
		key[i] = manualAlphabet[value]
	}
	return string(key[:])
}
func (a *App) PairingURL() string {
	a.pairingMu.Lock()
	defer a.pairingMu.Unlock()
	return fmt.Sprintf("https://%s/#%s~%s", a.host, a.pairingCode, a.tlsPin)
}
func (a *App) RefreshPairingURL() string {
	a.pairingMu.Lock()
	defer a.pairingMu.Unlock()
	a.pairingCode = a.pairing.NewPairingCode()
	return fmt.Sprintf("https://%s/#%s~%s", a.host, a.pairingCode, a.tlsPin)
}
func (a *App) SelectRecipient(id, name string) error {
	if err := a.pairing.Select(id); err != nil {
		return err
	}
	a.targeted, a.targetID, a.targetName = true, id, name
	a.received = make(map[string]bool)
	a.done = make(chan struct{})
	a.setStatus("waiting for device")
	return nil
}

func (a *App) Done() <-chan struct{}      { return a.done }
func (a *App) SetSendStatus(state string) { a.setStatus(state) }

func (a *App) setStatus(state string) {
	if !a.targeted {
		return
	}
	a.statusMu.Lock()
	defer a.statusMu.Unlock()
	files := make([]string, 0)
	for _, file := range a.catalog.Public() {
		files = append(files, file.Name)
	}
	if err := sendstatus.Write(a.cfg.DataDirectory, sendstatus.Status{Device: a.targetName, State: state, Files: files}); err != nil {
		log.Printf("STATUS save failed: %s", err)
	}
}

func (a *App) Run() error {
	return a.RunContext(context.Background())
}

func (a *App) RunContext(ctx context.Context) error {
	mux := http.NewServeMux()
	assets, _ := fs.Sub(webFiles, "web")
	mux.Handle("GET /", http.FileServer(http.FS(assets)))
	mux.HandleFunc("GET /portal.html", a.portalOnly(a.localOnly(http.FileServer(http.FS(assets)).ServeHTTP)))
	mux.HandleFunc("GET /api/portal", a.portalOnly(a.localOnly(a.handlePortal)))
	mux.HandleFunc("GET /api/portal/qr", a.portalOnly(a.localOnly(a.handlePortalQR)))
	mux.HandleFunc("POST /api/portal/refresh", a.portalOnly(a.localOnly(a.handlePortalRefresh)))
	mux.HandleFunc("POST /api/pair", a.handlePair)
	mux.HandleFunc("POST /api/challenge", a.handleChallenge)
	mux.HandleFunc("POST /api/verify", a.handleVerify)
	mux.HandleFunc("POST /api/upload", a.requireAuth(a.handleUpload))
	mux.HandleFunc("GET /api/folder-drop", a.requireAuth(a.handleFolderDrop))
	mux.HandleFunc("POST /api/receipt", a.requireAuth(a.handleReceipt))
	mux.HandleFunc("GET /api/session", a.requireAuth(a.handleSession))
	mux.HandleFunc("GET /api/messages", a.requireAuth(a.handleMessages))
	mux.HandleFunc("POST /api/messages", a.requireAuth(a.handlePhoneMessage))
	mux.HandleFunc("POST /api/heartbeat", a.requireAuth(func(w http.ResponseWriter, r *http.Request) {
		var report struct {
			RTT      float64
			Received int64
			Speed    float64
		}
		if !decode(w, r, &report) {
			return
		}
		log.Printf("HEALTH phone=%s rtt=%.0fms received=%dB download=%.0fB/s", r.RemoteAddr, report.RTT, report.Received, report.Speed)
		writeJSON(w, map[string]bool{"ok": true})
	}))
	mux.HandleFunc("GET /api/files", a.requireAuth(a.handleFiles))
	mux.HandleFunc("GET /api/files/{id}", a.requireAuth(a.handleDownload))
	mux.HandleFunc("POST /api/offers/{id}", a.requireAuth(a.handleOfferDecision))
	mux.HandleFunc("GET /api/scratchpad", a.requireAuth(a.handleScratchpad))
	mux.HandleFunc("POST /api/scratchpad", a.requireAuth(a.handleScratchpadPost))
	mux.HandleFunc("POST /api/paste-stack/{id}", a.requireAuth(a.handlePasteUpload))
	mux.HandleFunc("POST /api/screen/start", a.requireAuth(a.handleScreenStart))
	mux.HandleFunc("POST /api/screen/frame/{id}", a.requireAuth(a.handleScreenFrame))
	mux.HandleFunc("POST /api/screen/stop/{id}", a.requireAuth(a.handleScreenStop))
	keyPair, err := tls.LoadX509KeyPair(a.certificates.Certificate, a.certificates.Key)
	if err != nil {
		return err
	}
	listener, err := net.Listen("tcp", a.cfg.ListenAddress)
	if err != nil {
		return err
	}
	defer listener.Close()
	if stopViewer, err := a.startScreenListener(keyPair); err != nil {
		log.Printf("Screen viewer unavailable: %s", err)
	} else {
		defer stopViewer()
	}
	server := &http.Server{
		Addr: a.cfg.ListenAddress, Handler: securityHeaders(mux), ReadHeaderTimeout: 5e9,
		TLSConfig: &tls.Config{Certificates: []tls.Certificate{keyPair}},
	}
	if ctx.Done() != nil {
		go func() {
			<-ctx.Done()
			shutdown, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()
			_ = server.Shutdown(shutdown)
		}()
	}
	a.reportRestoredOffers()
	a.reportScratchpad()
	a.pasteSnapshot()
	relayContext, stopRelay := context.WithCancel(ctx)
	defer stopRelay()
	go a.collectRelay(relayContext)
	log.Printf("TUI_READY")
	err = server.ServeTLS(listener, "", "")
	if err == http.ErrServerClosed {
		return nil
	}
	return err
}

func (a *App) collectRelay(ctx context.Context) {
	inbox := a.cfg.InboxDirectory
	if inbox == "" {
		inbox = filepath.Join(a.cfg.DataDirectory, "inbox")
	}
	ticker := time.NewTicker(3 * time.Second)
	defer ticker.Stop()
	for {
		relay.Collect(a.cfg.DataDirectory, inbox, func(name string) {
			path := filepath.Join(inbox, "Relay", name)
			if info, err := os.Stat(path); err == nil {
				log.Printf("RECEIVED %q (%d bytes) saved to %q", name, info.Size(), path)
				a.recordFrom(name, "saved-to-inbox", info.Size(), "File Relay")
			}
		})
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
		}
	}
}

type pairRequest struct{ Code, DeviceID, Name, PublicKey string }

func (a *App) handlePair(w http.ResponseWriter, r *http.Request) {
	var request pairRequest
	if !decode(w, r, &request) {
		return
	}
	key, err := base64.RawURLEncoding.DecodeString(request.PublicKey)
	if err == nil {
		err = a.pairing.Pair(request.Code, request.DeviceID, request.Name, key)
	}
	if err != nil {
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}
	writeJSON(w, map[string]bool{"paired": true})
	log.Printf("PAIRED phone %q", request.Name)
}
func (a *App) handleChallenge(w http.ResponseWriter, r *http.Request) {
	var request struct{ DeviceID string }
	if !decode(w, r, &request) {
		return
	}
	challenge, err := a.pairing.Challenge(request.DeviceID)
	if err != nil {
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}
	writeJSON(w, map[string]string{"challenge": challenge})
}
func (a *App) handleVerify(w http.ResponseWriter, r *http.Request) {
	var request struct{ DeviceID, Signature string }
	if !decode(w, r, &request) {
		return
	}
	signature, err := base64.RawURLEncoding.DecodeString(request.Signature)
	var token string
	var join pairing.JoinStatus
	if err == nil {
		token, join, err = a.pairing.Verify(request.DeviceID, signature)
	}
	if err != nil {
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}
	writeJSON(w, map[string]string{"token": token})
	if device, ok := a.pairing.Device(request.DeviceID); ok {
		if join == pairing.JoinPending || join == pairing.JoinAllowed {
			a.logDevicePicker()
			log.Printf("TUI_PAIR_RESULT %s %x", join, device.Name)
		}
	}
	if join == pairing.JoinPending {
		log.Printf("JOIN REQUEST %s — sender approval needed", request.DeviceID)
	} else if join == pairing.JoinAllowed {
		log.Printf("CONNECTED authenticated device %q", request.DeviceID)
		if a.targeted && request.DeviceID == a.targetID {
			a.setStatus("offered")
		}
	}
}
func (a *App) handleFiles(w http.ResponseWriter, r *http.Request) {
	public := a.catalog.Public()
	peer := a.requestPeer(r)
	a.publishMu.Lock()
	for _, offer := range a.offers {
		if offer.Peer == peer && offer.State == "accepted" {
			public = append(public, offer.public().Files...)
		}
	}
	a.publishMu.Unlock()
	writeJSON(w, public)
}
func (a *App) handleDownload(w http.ResponseWriter, r *http.Request) {
	ctx, done, ok := a.session.begin(r.Context())
	if !ok {
		http.Error(w, "Transfer stopped on computer", http.StatusConflict)
		return
	}
	defer done()
	r = r.WithContext(ctx)
	peer := a.requestPeer(r)
	item, ok := a.sharedItem(r.PathValue("id"), peer)
	if !ok {
		http.NotFound(w, r)
		return
	}
	source, err := os.Open(item.Path)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	defer source.Close()
	info, err := source.Stat()
	if err != nil || !info.Mode().IsRegular() {
		http.NotFound(w, r)
		return
	}
	if !item.Modified.IsZero() && (info.Size() != item.Size || !item.Modified.Equal(info.ModTime())) {
		log.Printf("TUI_OFFER file=%s peer=%x state=changed", item.ID, peer)
		http.Error(w, "The original file changed. Ask the sender to offer it again.", http.StatusConflict)
		return
	}
	w.Header().Set("ETag", fmt.Sprintf(`"%s-%x-%x"`, item.ID, info.Size(), info.ModTime().UnixNano()))
	w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=%q", item.Name))
	if r.Method == http.MethodHead {
		http.ServeContent(w, r, item.Name, info.ModTime(), source)
		return
	}
	log.Printf("SENDING %q to %s", item.Name, r.RemoteAddr)
	a.setStatus("downloading")
	writer := &transferWriter{ResponseWriter: w, started: time.Now(), last: time.Now(), total: item.Size}
	lane, _ := strconv.Atoi(r.Header.Get("X-TapSend-Stream"))
	if lane < 1 || lane > 4 {
		lane = 1
	}
	writer.lane = lane
	writer.telemetry = startTransferTelemetry(item.Name, "TX", -1, item.ID, peer)
	log.Printf("STREAM START lane=%d file=%q range=%q", lane, item.Name, r.Header.Get("Range"))
	http.ServeContent(writer, r, item.Name, info.ModTime(), source)
	writer.report()
	state := "served"
	if writer.status >= 400 || writer.writeError != nil {
		state = "failed"
	}
	if writer.telemetry.total > 0 && writer.sent < writer.telemetry.total {
		state = "interrupted"
	}
	if r.Context().Err() != nil {
		state = "interrupted"
	}
	writer.telemetry.finish(state)
	log.Printf("REQUEST FINISHED %q", item.Name)
}

func (a *App) sharedItem(id, peer string) (storedFile, bool) {
	a.publishMu.Lock()
	defer a.publishMu.Unlock()
	if item, ok := a.catalog.Get(id); ok {
		return storedFile{Item: item}, true
	}
	for _, offer := range a.offers {
		if offer.Peer == peer && offer.State == "accepted" {
			for _, file := range offer.Files {
				if file.ID == id {
					return file, true
				}
			}
		}
	}
	return storedFile{}, false
}

type transferWriter struct {
	http.ResponseWriter
	started, last         time.Time
	sent, previous, total int64
	lane                  int
	telemetry             *transferTelemetry
	status                int
	writeError            error
}

func (w *transferWriter) WriteHeader(status int) {
	if w.status != 0 {
		return
	}
	w.status = status
	if w.telemetry != nil {
		if total, err := strconv.ParseInt(w.Header().Get("Content-Length"), 10, 64); err == nil {
			w.telemetry.total = total
		}
		switch status {
		case http.StatusOK:
			w.telemetry.offset = 0
		case http.StatusPartialContent:
			if before, _, found := strings.Cut(strings.TrimPrefix(w.Header().Get("Content-Range"), "bytes "), "-"); found {
				if offset, err := strconv.ParseInt(before, 10, 64); err == nil {
					w.telemetry.offset = offset
				}
			}
		}
	}
	w.ResponseWriter.WriteHeader(status)
}

// Reuse larger copy buffers while preserving ServeContent's range handling.
var transferBuffers = sync.Pool{New: func() any { b := make([]byte, 256*1024); return &b }}

var throughput = struct {
	sync.Mutex
	bytes, previous int64
	last            time.Time
}{last: time.Now()}

func (w *transferWriter) ReadFrom(r io.Reader) (int64, error) {
	buffer := transferBuffers.Get().(*[]byte)
	defer transferBuffers.Put(buffer)
	// Hide ReadFrom here to avoid recursive dispatch through io.CopyBuffer.
	return io.CopyBuffer(struct{ io.Writer }{w}, r, *buffer)
}

func (w *transferWriter) Write(data []byte) (int, error) {
	if w.status == 0 {
		w.WriteHeader(http.StatusOK)
	}
	n, err := w.ResponseWriter.Write(data)
	if err != nil {
		w.writeError = err
	}
	w.sent += int64(n)
	if w.telemetry != nil {
		if total, err := strconv.ParseInt(w.Header().Get("Content-Length"), 10, 64); err == nil {
			w.telemetry.total = total
		}
		w.telemetry.add(n)
	}
	throughput.Lock()
	throughput.bytes += int64(n)
	if elapsed := time.Since(throughput.last).Seconds(); elapsed >= 1 {
		log.Printf("TRANSFER session-sent=%.2fMiB upload=%.0fB/s", float64(throughput.bytes)/1048576, float64(throughput.bytes-throughput.previous)/elapsed)
		throughput.previous = throughput.bytes
		throughput.last = time.Now()
	}
	throughput.Unlock()
	if time.Since(w.last) >= time.Second {
		w.report()
	}
	return n, err
}
func (w *transferWriter) report() {
	elapsed := time.Since(w.last).Seconds()
	if elapsed <= 0 {
		return
	}
	log.Printf("STREAM lane=%d speed=%.0fB/s bytes=%d", w.lane, float64(w.sent-w.previous)/elapsed, w.sent)
	w.previous = w.sent
	w.last = time.Now()
}
func (a *App) requireAuth(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		token := strings.TrimPrefix(r.Header.Get("Authorization"), "Bearer ")
		access := a.pairing.Access(token)
		if access == pairing.JoinPending {
			http.Error(w, "Waiting for sender approval. Ask the sender to run: approve DEVICE_ID", http.StatusForbidden)
			return
		}
		if token == "" || access != pairing.JoinAllowed {
			log.Printf("AUTH REJECTED peer=%s", r.RemoteAddr)
			http.Error(w, "authentication required", http.StatusUnauthorized)
			return
		}
		next(w, r)
	}
}
func securityHeaders(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("Referrer-Policy", "no-referrer")
		w.Header().Set("Cache-Control", "no-store")
		next.ServeHTTP(w, r)
	})
}
func decode(w http.ResponseWriter, r *http.Request, value any) bool {
	defer r.Body.Close()
	if err := json.NewDecoder(http.MaxBytesReader(w, r.Body, 1<<20)).Decode(value); err != nil {
		http.Error(w, "invalid request", 400)
		return false
	}
	return true
}
func writeJSON(w http.ResponseWriter, value any) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(value)
}
func localHost(listen, advertise string) string {
	port := "8443"
	if host, p, err := net.SplitHostPort(listen); err == nil {
		port = p
		if advertise != "" {
			return net.JoinHostPort(advertise, port)
		}
		if host == "localhost" {
			return net.JoinHostPort(host, port)
		}
		if ip := net.ParseIP(host); ip != nil && !ip.IsUnspecified() {
			return net.JoinHostPort(host, port)
		}
	}
	connections, _ := net.InterfaceAddrs()
	for _, connection := range connections {
		if ipnet, ok := connection.(*net.IPNet); ok && ipnet.IP.To4() != nil && ipnet.IP.IsPrivate() {
			return net.JoinHostPort(ipnet.IP.String(), port)
		}
	}
	host, _ := os.Hostname()
	return net.JoinHostPort(host, port)
}
