package config

import (
	"path/filepath"
	"runtime"
	"testing"
)

func TestSettingsPersistAndValidate(t *testing.T) {
	if runtime.GOOS == "windows" { t.Setenv("APPDATA", t.TempDir()) } else { t.Setenv("XDG_CONFIG_HOME", t.TempDir()) }
	cfg := Default()
	cfg.ListenAddress = "127.0.0.1:9666"
	cfg.InboxDirectory = filepath.Join(t.TempDir(), "incoming")
	if err := Save(cfg); err != nil {
		t.Fatal(err)
	}
	loaded, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	if loaded.ListenAddress != cfg.ListenAddress || loaded.InboxDirectory != cfg.InboxDirectory {
		t.Fatalf("settings did not persist: %+v", loaded)
	}
	loaded.ListenAddress = ":0"
	if err := Save(loaded); err == nil {
		t.Fatal("invalid port accepted")
	}
}
