package server

import (
	"bytes"
	"crypto/rand"
	"crypto/subtle"
	"crypto/tls"
	"encoding/hex"
	"errors"
	"image/jpeg"
	"io"
	"log"
	"net"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"
)

const (
	screenMaxFrame = 512 << 10
	screenLease    = 8 * time.Second
	screenWait     = 500 * time.Millisecond
)

type liveScreen struct {
	sync.Mutex
	port        int
	peer        string
	viewKey     string
	capture     string
	phase       string
	viewedAt    time.Time
	phoneAt     time.Time
	frameNumber uint64
	frame       []byte
	changed     chan struct{}
	redPen      bool
	arrow       string
	arrowUntil  time.Time
}

// Call while holding the screen lock. Slow viewers always get the newest frame.
func (s *liveScreen) notify() {
	if s.changed != nil {
		close(s.changed)
		s.changed = nil
	}
}

func (a *App) startScreenListener(keys tls.Certificate) (func(), error) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, err
	}
	mux := http.NewServeMux()
	mux.HandleFunc("GET /screen", a.handleScreenView)
	mux.HandleFunc("POST /screen/grab/{kind}", a.handleScreenGrab)
	mux.HandleFunc("POST /screen/arrow", a.handleScreenArrow)
	viewer := &http.Server{
		Handler: mux, ReadHeaderTimeout: 4 * time.Second,
		WriteTimeout: 5 * time.Second, IdleTimeout: 15 * time.Second,
		TLSConfig: &tls.Config{Certificates: []tls.Certificate{keys}},
	}
	a.screen.Lock()
	a.screen.port = listener.Addr().(*net.TCPAddr).Port
	a.screen.Unlock()
	go func() { _ = viewer.ServeTLS(listener, "", "") }()
	return func() {
		a.closeScreen()
		_ = viewer.Close()
		_ = listener.Close()
	}, nil
}

func screenKey() (string, error) {
	var key [32]byte
	_, err := rand.Read(key[:])
	return hex.EncodeToString(key[:]), err
}

func (a *App) handleScreenGrab(w http.ResponseWriter, r *http.Request) {
	key := r.Header.Get("X-TapSend-View")
	s := &a.screen
	s.Lock()
	allowed := s.viewKey != "" && subtle.ConstantTimeCompare([]byte(key), []byte(s.viewKey)) == 1
	s.Unlock()
	if !allowed {
		http.Error(w, "Screen viewer closed", http.StatusGone)
		return
	}
	kind := r.PathValue("kind")
	var limit int64
	switch kind {
	case "image":
		limit = pasteMaxImage
	case "text":
		limit = pasteMaxText
	default:
		http.Error(w, "Unsupported grab", http.StatusBadRequest)
		return
	}
	_ = http.NewResponseController(w).SetReadDeadline(time.Now().Add(4 * time.Second))
	data, err := io.ReadAll(http.MaxBytesReader(w, r.Body, limit))
	if err != nil {
		http.Error(w, "Grab exceeds its size limit", http.StatusRequestEntityTooLarge)
		return
	}
	id, err := screenKey()
	if err != nil {
		http.Error(w, "Could not create Paste Stack item", http.StatusServiceUnavailable)
		return
	}
	if status, err := a.storePaste(id, kind, "Screen grab", "Screen Share", data); err != nil {
		http.Error(w, err.Error(), status)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func (a *App) handleScreenArrow(w http.ResponseWriter, r *http.Request) {
	var points []int
	if !decode(w, r, &points) {
		return
	}
	if len(points) != 4 {
		http.Error(w, "An arrow needs two points", http.StatusBadRequest)
		return
	}
	parts := make([]string, 4)
	for i, coordinate := range points {
		if coordinate < 0 || coordinate > 1000 {
			http.Error(w, "Arrow is outside the screen", http.StatusBadRequest)
			return
		}
		parts[i] = strconv.Itoa(coordinate)
	}
	key := r.Header.Get("X-TapSend-View")
	s := &a.screen
	s.Lock()
	defer s.Unlock()
	if s.viewKey == "" || subtle.ConstantTimeCompare([]byte(key), []byte(s.viewKey)) != 1 {
		http.Error(w, "Screen viewer closed", http.StatusGone)
		return
	}
	if !s.redPen || s.phase != "sharing" || time.Since(s.phoneAt) > screenLease {
		http.Error(w, "The phone has not enabled Live Red Pen", http.StatusConflict)
		return
	}
	s.arrow, s.arrowUntil = strings.Join(parts, ","), time.Now().Add(2*time.Second)
	w.WriteHeader(http.StatusNoContent)
}

func (a *App) armScreen(peer string) error {
	if _, ok := a.pairing.Device(peer); !ok {
		return errors.New("choose a paired phone")
	}
	key, err := screenKey()
	if err != nil {
		return err
	}
	s := &a.screen
	s.Lock()
	defer s.Unlock()
	if s.port == 0 {
		return errors.New("local screen viewer is not ready")
	}
	s.peer, s.viewKey, s.capture, s.phase = peer, key, "", "waiting"
	s.viewedAt, s.phoneAt = time.Now(), time.Time{}
	s.frameNumber, s.frame = 0, nil
	s.redPen, s.arrow = false, ""
	s.notify()
	log.Printf("TUI_SCREEN port=%d key=%s pin=%s", s.port, key, a.screenPin)
	return nil
}

func (a *App) closeScreen() {
	s := &a.screen
	s.Lock()
	defer s.Unlock()
	s.peer, s.viewKey, s.capture, s.phase = "", "", "", ""
	s.frame = nil
	s.notify()
}

func (a *App) revokeScreen(peer string) {
	s := &a.screen
	s.Lock()
	defer s.Unlock()
	if s.peer == peer {
		s.peer, s.viewKey, s.capture, s.phase = "", "", "", ""
		s.frame = nil
		s.notify()
	}
}

func (a *App) handleScreenStart(w http.ResponseWriter, r *http.Request) {
	var request struct {
		ID     string
		RedPen bool
	}
	if !decode(w, r, &request) {
		return
	}
	if decoded, err := hex.DecodeString(request.ID); err != nil || len(decoded) != 16 {
		http.Error(w, "Invalid screen session", http.StatusBadRequest)
		return
	}
	peer := a.requestPeer(r)
	s := &a.screen
	s.Lock()
	defer s.Unlock()
	if peer == "" || peer != s.peer || time.Since(s.viewedAt) > screenLease {
		http.Error(w, "Open Screen Share on the PC and select this phone", http.StatusConflict)
		return
	}
	if s.capture != "" && request.ID != s.capture && time.Since(s.phoneAt) <= screenLease {
		http.Error(w, "This phone already has an active screen share", http.StatusConflict)
		return
	}
	if s.capture != request.ID {
		s.frame = nil
		s.arrow = ""
	}
	s.redPen = request.RedPen
	s.capture, s.phase, s.phoneAt = request.ID, "sharing", time.Now()
	s.notify()
	writeJSON(w, map[string]bool{"ok": true})
}

func (s *liveScreen) accepts(peer, capture string) bool {
	return peer != "" && peer == s.peer && capture != "" && capture == s.capture && time.Since(s.viewedAt) <= screenLease
}

func (a *App) handleScreenFrame(w http.ResponseWriter, r *http.Request) {
	peer, capture := a.requestPeer(r), r.PathValue("id")
	s := &a.screen
	s.Lock()
	valid := s.accepts(peer, capture)
	s.Unlock()
	if !valid {
		http.Error(w, "The desktop closed the screen viewer", http.StatusGone)
		return
	}
	controller := http.NewResponseController(w)
	_ = controller.SetReadDeadline(time.Now().Add(4 * time.Second))
	frame, err := io.ReadAll(http.MaxBytesReader(w, r.Body, screenMaxFrame))
	_ = controller.SetReadDeadline(time.Time{})
	if err != nil {
		http.Error(w, "Screen frame is too large or incomplete", http.StatusBadRequest)
		return
	}
	if len(frame) != 0 {
		size, err := jpeg.DecodeConfig(bytes.NewReader(frame))
		if err != nil || size.Width <= 0 || size.Height <= 0 || size.Width > 1280 || size.Height > 1280 {
			http.Error(w, "Invalid screen frame", http.StatusBadRequest)
			return
		}
	}
	if s.redPen && s.arrow != "" && time.Now().Before(s.arrowUntil) {
		w.Header().Set("X-TapSend-Arrow", s.arrow)
		s.arrow = ""
	}
	s.Lock()
	defer s.Unlock()
	if !s.accepts(peer, capture) {
		http.Error(w, "The desktop closed the screen viewer", http.StatusGone)
		return
	}
	s.phase, s.phoneAt = "sharing", time.Now()
	if len(frame) != 0 {
		s.frame = frame
		s.frameNumber++
		s.notify()
	}
	w.WriteHeader(http.StatusNoContent)
}

func (a *App) handleScreenStop(w http.ResponseWriter, r *http.Request) {
	peer, capture := a.requestPeer(r), r.PathValue("id")
	s := &a.screen
	s.Lock()
	defer s.Unlock()
	if peer != "" && peer == s.peer && capture == s.capture {
		s.capture, s.phase, s.frame = "", "stopped", nil
		s.notify()
	}
	w.WriteHeader(http.StatusNoContent)
}

func (a *App) handleScreenView(w http.ResponseWriter, r *http.Request) {
	key := r.Header.Get("X-TapSend-View")
	after, _ := strconv.ParseUint(r.URL.Query().Get("after"), 10, 64)
	previousPhase := r.URL.Query().Get("phase")
	wait := r.URL.Query().Get("wait") == "1"
	w.Header().Set("X-TapSend-Screen-Wait", "1")
	s := &a.screen
	s.Lock()
	if wait && s.viewKey != "" && subtle.ConstantTimeCompare([]byte(key), []byte(s.viewKey)) == 1 &&
		previousPhase == s.phase && (len(s.frame) == 0 || s.frameNumber == after) {
		s.viewedAt = time.Now()
		if s.changed == nil {
			s.changed = make(chan struct{})
		}
		changed := s.changed
		s.Unlock()
		timer := time.NewTimer(screenWait)
		defer timer.Stop()
		select {
		case <-changed:
		case <-timer.C:
		case <-r.Context().Done():
			return
		}
		s.Lock()
	}
	if s.viewKey == "" || subtle.ConstantTimeCompare([]byte(key), []byte(s.viewKey)) != 1 {
		s.Unlock()
		http.Error(w, "Screen viewer closed", http.StatusGone)
		return
	}
	s.viewedAt = time.Now()
	if s.phase == "sharing" && time.Since(s.phoneAt) > screenLease {
		s.capture, s.phase, s.frame = "", "stopped", nil
		s.notify()
	}
	frame, seq, phase := s.frame, s.frameNumber, s.phase
	s.Unlock()
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-TapSend-Screen", phase)
	if len(frame) == 0 || seq == after {
		w.WriteHeader(http.StatusNoContent)
		return
	}
	w.Header().Set("Content-Type", "image/jpeg")
	w.Header().Set("X-TapSend-Frame", strconv.FormatUint(seq, 10))
	w.Write(frame)
}
