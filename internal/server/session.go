package server

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"sync"
	"time"
)

type message struct {
	ID   int64     `json:"id"`
	From string    `json:"from"`
	Text string    `json:"text"`
	Time time.Time `json:"time"`
}
type sessionState struct {
	sync.Mutex
	stopped  bool
	active   map[uint64]context.CancelFunc
	next     uint64
	messages []message
}

func (s *sessionState) stop() {
	s.Lock()
	defer s.Unlock()
	s.stopped = true
	for _, cancel := range s.active {
		cancel()
	}
	log.Printf("TRANSFER STOPPED by computer")
}
func (s *sessionState) resume() {
	s.Lock()
	s.stopped = false
	s.Unlock()
	log.Printf("TRANSFER RESUMED by computer")
}
func (s *sessionState) begin(ctx context.Context) (context.Context, func(), bool) {
	s.Lock()
	defer s.Unlock()
	if s.stopped {
		return nil, nil, false
	}
	if s.active == nil {
		s.active = make(map[uint64]context.CancelFunc)
	}
	s.next++
	id := s.next
	next, cancel := context.WithCancel(ctx)
	s.active[id] = cancel
	return next, func() { s.Lock(); delete(s.active, id); s.Unlock(); cancel() }, true
}
func (s *sessionState) post(from, text string) {
	s.Lock()
	defer s.Unlock()
	id := int64(1)
	if len(s.messages) > 0 {
		id = s.messages[len(s.messages)-1].ID + 1
	}
	s.messages = append(s.messages, message{id, from, text, time.Now()})
	if len(s.messages) > 100 {
		s.messages = s.messages[len(s.messages)-100:]
	}
}
func (a *App) handleSession(w http.ResponseWriter, r *http.Request) {
	a.pastePhoneSeen(a.requestPeer(r))
	a.session.Lock()
	stopped := a.session.stopped
	a.session.Unlock()
	writeJSON(w, map[string]any{
		"stopped": stopped, "targeted": a.targeted, "offers": a.pendingOffers(a.requestPeer(r)),
		"scratchpad": true, "uploadLimitGiB": a.cfg.UploadLimitGiB,
	})
}
func (a *App) handleMessages(w http.ResponseWriter, r *http.Request) {
	a.session.Lock()
	defer a.session.Unlock()
	items := make([]message, 0)
	for _, m := range a.session.messages {
		if m.ID > 0 {
			items = append(items, m)
		}
	}
	writeJSON(w, items)
}
func (a *App) handlePhoneMessage(w http.ResponseWriter, r *http.Request) {
	var payload struct {
		Text string `json:"text"`
	}
	if err := json.NewDecoder(http.MaxBytesReader(w, r.Body, 4096)).Decode(&payload); err != nil || len(payload.Text) == 0 || len(payload.Text) > 2000 {
		http.Error(w, "Message must be 1–2000 bytes", 400)
		return
	}
	a.session.post("phone", payload.Text)
	log.Printf("MESSAGE phone: %q", payload.Text)
	writeJSON(w, map[string]bool{"ok": true})
}
