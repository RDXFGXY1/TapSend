package config

import (
	"os"
	"path/filepath"
)

type Config struct {
	ListenAddress string
	DataDirectory string
	SharedFiles   []string
}

func Default() Config {
	home, _ := os.UserHomeDir()
	return Config{
		ListenAddress: ":8443",
		DataDirectory: filepath.Join(home, ".local", "share", "tapsend"),
	}
}
