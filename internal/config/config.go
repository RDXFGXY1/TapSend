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
	ListenAddress   string
	AdvertiseHost   string
	DataDirectory   string
	InboxDirectory  string
	StartDirectory  string
	ShowHidden      bool
	DirectJoinLimit int
	UploadLimitGiB  int
	PortalEnabled   bool
	SharedFiles     []string
}

type Settings struct {
	ListenAddress   string `json:"listen_address"`
	AdvertiseHost   string `json:"advertise_host,omitempty"`
	InboxDirectory  string `json:"inbox_directory"`
	StartDirectory  string `json:"start_directory,omitempty"`
	ShowHidden      bool   `json:"show_hidden"`
	DirectJoinLimit *int   `json:"direct_join_limit,omitempty"`
	DataDirectory   string `json:"data_directory,omitempty"`
	UploadLimitGiB  *int   `json:"upload_limit_gib,omitempty"`
	PortalEnabled   *bool  `json:"portal_enabled,omitempty"`
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
		ListenAddress:   ":8443",
		DataDirectory:   data,
		InboxDirectory:  filepath.Join(data, "inbox"),
		DirectJoinLimit: 2,
		UploadLimitGiB:  8,
		PortalEnabled:   true,
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
	cfg.AdvertiseHost = settings.AdvertiseHost
	cfg.StartDirectory = settings.StartDirectory
	cfg.ShowHidden = settings.ShowHidden
	if settings.DataDirectory != "" {
		cfg.DataDirectory = settings.DataDirectory
	}
	if settings.DirectJoinLimit != nil {
		cfg.DirectJoinLimit = *settings.DirectJoinLimit
	}
	if settings.UploadLimitGiB != nil {
		cfg.UploadLimitGiB = *settings.UploadLimitGiB
	}
	if settings.PortalEnabled != nil {
		cfg.PortalEnabled = *settings.PortalEnabled
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
	if cfg.StartDirectory != "" && !filepath.IsAbs(cfg.StartDirectory) {
		return fmt.Errorf("start directory must be an absolute path")
	}
	if cfg.AdvertiseHost != "" {
		if strings.ContainsAny(cfg.AdvertiseHost, " \t\r\n/?#@[]") || (strings.Contains(cfg.AdvertiseHost, ":") && net.ParseIP(cfg.AdvertiseHost) == nil) {
			return fmt.Errorf("pairing host must be an IP address or hostname without a port")
		}
	}
	if cfg.DirectJoinLimit < 1 || cfg.DirectJoinLimit > 8 {
		return fmt.Errorf("direct join limit must be 1–8")
	}
	if cfg.DataDirectory == "" || !filepath.IsAbs(cfg.DataDirectory) {
		return fmt.Errorf("data directory must be an absolute path")
	}
	if cfg.UploadLimitGiB < 1 || cfg.UploadLimitGiB > 64 {
		return fmt.Errorf("upload limit must be 1–64 GiB")
	}
	return nil
}

func Save(cfg Config) error {
	if err := Validate(cfg); err != nil {
		return err
	}
	if cfg.StartDirectory != "" {
		info, err := os.Stat(cfg.StartDirectory)
		if err != nil || !info.IsDir() {
			return fmt.Errorf("start directory must be an existing folder")
		}
	}
	path, err := SettingsPath()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		return err
	}
	limit := cfg.DirectJoinLimit
	uploadLimit, portalEnabled := cfg.UploadLimitGiB, cfg.PortalEnabled
	data, err := json.MarshalIndent(Settings{
		ListenAddress: cfg.ListenAddress, AdvertiseHost: cfg.AdvertiseHost,
		InboxDirectory: cfg.InboxDirectory, StartDirectory: cfg.StartDirectory,
		ShowHidden: cfg.ShowHidden, DirectJoinLimit: &limit,
		DataDirectory: cfg.DataDirectory, UploadLimitGiB: &uploadLimit, PortalEnabled: &portalEnabled,
	}, "", "  ")
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

// PrepareStorage checks both destinations before onboarding commits preferences.
func PrepareStorage(cfg Config) error {
	if err := Validate(cfg); err != nil {
		return err
	}
	for _, dir := range []string{cfg.InboxDirectory, cfg.DataDirectory} {
		if err := os.MkdirAll(dir, 0700); err != nil {
			return fmt.Errorf("cannot create folder %q: %w", dir, err)
		}
		file, err := os.CreateTemp(dir, ".tapsend-setup-*")
		if err != nil {
			return fmt.Errorf("folder %q is not writable: %w", dir, err)
		}
		err = file.Close()
		removeErr := os.Remove(file.Name())
		if err != nil {
			return err
		}
		if removeErr != nil {
			return removeErr
		}
	}
	return nil
}
