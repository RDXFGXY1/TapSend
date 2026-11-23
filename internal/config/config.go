package config

import (
	"encoding/json"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
)

type Config struct {
	ListenAddress  string
	DataDirectory  string
	InboxDirectory string
	SharedFiles    []string
}

type Settings struct {
	ListenAddress  string `json:"listen_address"`
	InboxDirectory string `json:"inbox_directory"`
}

func Default() Config {
	home, _ := os.UserHomeDir()
	data := filepath.Join(home, ".local", "share", "tapsend")
	if runtime.GOOS == "windows" {
		if configDir, err := os.UserConfigDir(); err == nil {
			data = filepath.Join(configDir, "TapSend", "data")
		}
	}
	return Config{
		ListenAddress:  ":8443",
		DataDirectory:  data,
		InboxDirectory: filepath.Join(data, "inbox"),
	}
}

func SettingsPath() (string, error) {
	directory, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	name := "tapsend"
	if runtime.GOOS == "windows" {
		name = "TapSend"
	}
	return filepath.Join(directory, name, "settings.json"), nil
}

func Load() (Config, error) {
	cfg := Default()
	path, err := SettingsPath()
	if err != nil {
		return cfg, err
	}
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return cfg, nil
	}
	if err != nil {
		return cfg, err
	}
	var settings Settings
	if err := json.Unmarshal(data, &settings); err != nil {
		return cfg, fmt.Errorf("read settings: %w", err)
	}
	if settings.ListenAddress != "" {
		cfg.ListenAddress = settings.ListenAddress
	}
	if settings.InboxDirectory != "" {
		cfg.InboxDirectory = settings.InboxDirectory
	}
	if err := Validate(cfg); err != nil {
		return cfg, err
	}
	return cfg, nil
}

func Validate(cfg Config) error {
	host, port, err := net.SplitHostPort(cfg.ListenAddress)
	if err != nil {
		return fmt.Errorf("invalid listen address: %w", err)
	}
	if host != "" && net.ParseIP(host) == nil && host != "localhost" {
		return fmt.Errorf("listen host must be an IP address or localhost")
	}
	number, err := strconv.Atoi(port)
	if err != nil || number < 1 || number > 65535 {
		return fmt.Errorf("listen port must be 1–65535")
	}
	if strings.TrimSpace(cfg.InboxDirectory) == "" || !filepath.IsAbs(cfg.InboxDirectory) {
		return fmt.Errorf("inbox directory must be an absolute path")
	}
	return nil
}

func Save(cfg Config) error {
	if err := Validate(cfg); err != nil {
		return err
	}
	path, err := SettingsPath()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		return err
	}
	data, err := json.MarshalIndent(Settings{cfg.ListenAddress, cfg.InboxDirectory}, "", "  ")
	if err != nil {
		return err
	}
	temporary, err := os.CreateTemp(filepath.Dir(path), "settings-*")
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
	return os.Rename(temporary.Name(), path)
}
