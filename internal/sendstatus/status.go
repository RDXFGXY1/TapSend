package sendstatus

import (
	"encoding/json"
	"os"
	"path/filepath"
	"time"
)

type Status struct {
	Device    string    `json:"device"`
	State     string    `json:"state"`
	Files     []string  `json:"files"`
	UpdatedAt time.Time `json:"updated_at"`
}

func Path(dataDirectory string) string { return filepath.Join(dataDirectory, "send-status.json") }

func Read(dataDirectory string) (Status, error) {
	data, err := os.ReadFile(Path(dataDirectory))
	if err != nil {
		return Status{}, err
	}
	var state Status
	err = json.Unmarshal(data, &state)
	return state, err
}

func Write(dataDirectory string, state Status) error {
	if err := os.MkdirAll(dataDirectory, 0700); err != nil {
		return err
	}
	state.UpdatedAt = time.Now()
	data, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return err
	}
	temporary, err := os.CreateTemp(dataDirectory, "send-status-*")
	if err != nil {
		return err
	}
	defer os.Remove(temporary.Name())
	if err := temporary.Chmod(0600); err != nil {
		temporary.Close()
		return err
	}
	if _, err := temporary.Write(data); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	return os.Rename(temporary.Name(), Path(dataDirectory))
}
