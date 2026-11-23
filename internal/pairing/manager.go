package pairing

import (
	"crypto/ed25519"
	"crypto/rand"
	"encoding/base64"
	"errors"
	"fmt"
	"math/big"
	"sort"
	"strings"
	"sync"
	"time"

	"tapsend/internal/devices"
)

type expiringValue struct {
	Value   string
	Expires time.Time
}
type Manager struct {
	mu          sync.Mutex
	pairing     expiringValue
	manualCode  string
	manualTries int
	challenges  map[string]expiringValue
	sessions    map[string]expiringValue
	devices     *devices.Store
	recipient   string
	joined      map[string]bool
	pending     map[string]bool
	denied      map[string]bool
}

type JoinStatus string

const (
	JoinAllowed JoinStatus = "allowed"
	JoinPending JoinStatus = "pending"
	JoinDenied  JoinStatus = "denied"
)

func New(store *devices.Store) *Manager {
	return &Manager{devices: store, challenges: make(map[string]expiringValue), sessions: make(map[string]expiringValue),
		joined: make(map[string]bool), pending: make(map[string]bool), denied: make(map[string]bool)}
}
func (m *Manager) NewPairingCode() string {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.pairing = expiringValue{randomToken(16), time.Now().Add(5 * time.Minute)}
	for {
		number, err := rand.Int(rand.Reader, big.NewInt(100000000))
		if err != nil {
			panic(err)
		}
		code := fmt.Sprintf("%08d", number.Int64())
		if code != m.manualCode {
			m.manualCode = code
			break
		}
	}
	m.manualTries = 0
	return m.pairing.Value
}
func (m *Manager) ManualCode() string {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.manualCode
}
func (m *Manager) Pair(code, id, name string, publicKey []byte) error {
	m.mu.Lock()
	current := time.Now().Before(m.pairing.Expires)
	valid := current && code == m.pairing.Value
	if !valid && len(code) == 8 && strings.IndexFunc(code, func(r rune) bool { return r < '0' || r > '9' }) == -1 {
		valid = current && code == m.manualCode && m.manualTries < 10
		if !valid && current {
			m.manualTries++
		}
	}
	m.mu.Unlock()
	if !valid {
		return errors.New("pairing code is invalid, expired, or locked; press r on the sender for a new code")
	}
	if len(publicKey) != ed25519.PublicKeySize {
		return errors.New("invalid Ed25519 public key")
	}
	return m.devices.Put(devices.Device{ID: id, Name: name, PublicKey: publicKey, PairedAt: time.Now()})
}
func (m *Manager) Challenge(deviceID string) (string, error) {
	if _, ok := m.devices.Get(deviceID); !ok {
		return "", errors.New("unknown device")
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	value := randomToken(32)
	m.challenges[deviceID] = expiringValue{value, time.Now().Add(time.Minute)}
	return value, nil
}
func (m *Manager) Verify(deviceID string, signature []byte) (string, JoinStatus, error) {
	device, ok := m.devices.Get(deviceID)
	if !ok {
		return "", "", errors.New("unknown device")
	}
	m.mu.Lock()
	challenge, ok := m.challenges[deviceID]
	delete(m.challenges, deviceID)
	m.mu.Unlock()
	if !ok || time.Now().After(challenge.Expires) {
		return "", "", errors.New("challenge expired")
	}
	if !ed25519.Verify(device.PublicKey, []byte(challenge.Value), signature) {
		return "", "", errors.New("signature rejected")
	}
	token := randomToken(32)
	m.mu.Lock()
	m.sessions[token] = expiringValue{deviceID, time.Now().Add(15 * time.Minute)}
	status := m.joinLocked(deviceID)
	m.mu.Unlock()
	return token, status, nil
}

func (m *Manager) joinLocked(id string) JoinStatus {
	if m.denied[id] {
		return JoinDenied
	}
	if m.joined[id] {
		return JoinAllowed
	}
	if m.pending[id] {
		return JoinPending
	}
	if len(m.joined) < 2 {
		m.joined[id] = true
		return JoinAllowed
	}
	m.pending[id] = true
	return JoinPending
}

func (m *Manager) Access(token string) JoinStatus {
	m.mu.Lock()
	defer m.mu.Unlock()
	session, ok := m.sessions[token]
	if ok && time.Now().Before(session.Expires) {
		_, trusted := m.devices.Get(session.Value)
		if !trusted || (m.recipient != "" && m.recipient != session.Value) || m.denied[session.Value] {
			return JoinDenied
		}
		if m.joined[session.Value] {
			return JoinAllowed
		}
		if m.pending[session.Value] {
			return JoinPending
		}
	}
	delete(m.sessions, token)
	return JoinDenied
}
func (m *Manager) Devices() []devices.Device { return m.devices.List() }
func (m *Manager) Pending() []devices.Device {
	m.mu.Lock()
	defer m.mu.Unlock()
	var result []devices.Device
	for id := range m.pending {
		if device, ok := m.devices.Get(id); ok {
			result = append(result, device)
		}
	}
	sort.Slice(result, func(i, j int) bool { return result[i].PairedAt.Before(result[j].PairedAt) })
	return result
}
func (m *Manager) Approve(id string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	resolved, err := m.pendingIDLocked(id)
	if err != nil {
		return err
	}
	delete(m.pending, resolved)
	m.joined[resolved] = true
	return nil
}
func (m *Manager) Deny(id string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	resolved, err := m.pendingIDLocked(id)
	if err != nil {
		return err
	}
	delete(m.pending, resolved)
	m.denied[resolved] = true
	return nil
}
func (m *Manager) pendingIDLocked(id string) (string, error) {
	if m.pending[id] {
		return id, nil
	}
	if len(id) < 6 {
		return "", errors.New("enter at least 6 characters of a pending device ID")
	}
	match := ""
	for candidate := range m.pending {
		if len(candidate) >= len(id) && candidate[:len(id)] == id {
			if match != "" {
				return "", errors.New("device ID prefix is ambiguous")
			}
			match = candidate
		}
	}
	if match == "" {
		return "", errors.New("device is not waiting for approval")
	}
	return match, nil
}
func (m *Manager) Revoke(id string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if err := m.devices.Delete(id); err != nil {
		return err
	}
	delete(m.joined, id)
	delete(m.pending, id)
	delete(m.denied, id)
	for token, session := range m.sessions {
		if session.Value == id {
			delete(m.sessions, token)
		}
	}
	return nil
}
func (m *Manager) Select(id string) error {
	if id == "all" {
		id = ""
	}
	if id != "" {
		if _, ok := m.devices.Get(id); !ok {
			return errors.New("unknown device")
		}
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	m.recipient = id
	return nil
}
func (m *Manager) Rename(id, name string) error {
	d, ok := m.devices.Get(id)
	if !ok {
		return errors.New("unknown device")
	}
	if name == "" || len(name) > 80 {
		return errors.New("name must be 1–80 bytes")
	}
	d.Name = name
	return m.devices.Put(d)
}
func randomToken(size int) string {
	data := make([]byte, size)
	_, _ = rand.Read(data)
	return base64.RawURLEncoding.EncodeToString(data)
}
