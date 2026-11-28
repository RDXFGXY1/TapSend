package devices

import (
	"encoding/json"
	"errors"
	"maps"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"time"
)

type Device struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	PublicKey []byte    `json:"public_key"`
	PairedAt  time.Time `json:"paired_at"`
}
type Store struct {
	path    string
	mu      sync.RWMutex
	devices map[string]Device
}

func Open(directory string) (*Store, error) {
	if err := os.MkdirAll(directory, 0700); err != nil {
		return nil, err
	}
	s := &Store{path: filepath.Join(directory, "devices.json"), devices: make(map[string]Device)}
	data, err := os.ReadFile(s.path)
	if err == nil {
		err = json.Unmarshal(data, &s.devices)
	}
	if os.IsNotExist(err) {
		err = nil
	}
	if s.devices == nil {
		s.devices = make(map[string]Device)
	}
	return s, err
}
func (s *Store) Put(device Device) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.putLocked(device)
}
func (s *Store) Register(device Device) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, exists := s.devices[device.ID]; exists {
		return errors.New("device ID is already paired")
	}
	return s.putLocked(device)
}
func (s *Store) putLocked(device Device) error {
	next := maps.Clone(s.devices)
	next[device.ID] = device
	return s.replaceLocked(next)
}
func (s *Store) Get(id string) (Device, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	device, ok := s.devices[id]
	return device, ok
}
func (s *Store) List() []Device {
	s.mu.RLock()
	defer s.mu.RUnlock()
	result := make([]Device, 0, len(s.devices))
	for _, d := range s.devices {
		result = append(result, d)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Name < result[j].Name })
	return result
}
func (s *Store) Delete(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	next := make(map[string]Device, len(s.devices))
	for k, v := range s.devices {
		if k != id {
			next[k] = v
		}
	}
	return s.replaceLocked(next)
}

func (s *Store) Clear() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.replaceLocked(make(map[string]Device))
}

func (s *Store) replaceLocked(next map[string]Device) error {
	data, err := json.MarshalIndent(next, "", "  ")
	if err != nil {
		return err
	}
	file, err := os.CreateTemp(filepath.Dir(s.path), ".devices-*")
	if err != nil {
		return err
	}
	defer os.Remove(file.Name())
	_, err = file.Write(data)
	if closeErr := file.Close(); err == nil {
		err = closeErr
	}
	if err != nil {
		return err
	}
	if err = os.Rename(file.Name(), s.path); err != nil {
		return err
	}
	s.devices = next
	return nil
}
