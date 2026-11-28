package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestSettingsPersistAndValidate(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Setenv("APPDATA", t.TempDir())
	} else {
		t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	}
	cfg := Default()
	cfg.ListenAddress = "127.0.0.1:9666"
	cfg.InboxDirectory = filepath.Join(t.TempDir(), "incoming")
	cfg.StartDirectory = t.TempDir()
	cfg.ShowHidden = true
	cfg.AdvertiseHost = "192.0.2.42"
	cfg.DirectJoinLimit = 4
	cfg.DataDirectory = filepath.Join(t.TempDir(), "private")
	cfg.UploadLimitGiB = 12
	cfg.PortalEnabled = false
	if err := Save(cfg); err != nil {
		t.Fatal(err)
	}
	loaded, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	if loaded.ListenAddress != cfg.ListenAddress || loaded.InboxDirectory != cfg.InboxDirectory || loaded.StartDirectory != cfg.StartDirectory || loaded.ShowHidden != cfg.ShowHidden || loaded.AdvertiseHost != cfg.AdvertiseHost || loaded.DirectJoinLimit != cfg.DirectJoinLimit || loaded.DataDirectory != cfg.DataDirectory || loaded.UploadLimitGiB != cfg.UploadLimitGiB || loaded.PortalEnabled != cfg.PortalEnabled {
		t.Fatalf("settings did not persist: %+v", loaded)
	}
	loaded.ListenAddress = ":0"
	if err := Save(loaded); err == nil {
		t.Fatal("invalid port accepted")
	}
}

func TestLegacySettingsKeepDirectJoinDefault(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Setenv("APPDATA", t.TempDir())
	} else {
		t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	}
	path, err := SettingsPath()
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		t.Fatal(err)
	}
	legacy, err := json.Marshal(Settings{ListenAddress: ":8443", InboxDirectory: filepath.Join(t.TempDir(), "incoming")})
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, legacy, 0600); err != nil {
		t.Fatal(err)
	}
	loaded, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	if loaded.DirectJoinLimit != 2 {
		t.Fatalf("legacy direct join limit = %d", loaded.DirectJoinLimit)
	}
	if loaded.UploadLimitGiB != 8 || !loaded.PortalEnabled {
		t.Fatalf("legacy upload and portal defaults lost: %+v", loaded)
	}
}

func TestMissingStartingFolderDoesNotBlockExistingSettings(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Setenv("APPDATA", t.TempDir())
	} else {
		t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	}
	folder := t.TempDir()
	cfg := Default()
	cfg.StartDirectory = folder
	if err := Save(cfg); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(folder); err != nil {
		t.Fatal(err)
	}
	if _, err := Load(); err != nil {
		t.Fatalf("removed starting folder blocked settings: %v", err)
	}
	if err := Save(cfg); err == nil {
		t.Fatal("accepted missing starting folder on save")
	}
}
