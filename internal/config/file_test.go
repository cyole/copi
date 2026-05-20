package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadMissingConfigReturnsDefaults(t *testing.T) {
	path := filepath.Join(t.TempDir(), "missing.json")
	cfg, resolved, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	if resolved != path {
		t.Fatalf("resolved = %q, want %q", resolved, path)
	}
	if cfg.Relay.Addr != DefaultRelayAddr {
		t.Fatalf("relay addr = %q, want %q", cfg.Relay.Addr, DefaultRelayAddr)
	}
}

func TestInitAndLoadConfig(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.json")
	written, err := Init(path, false)
	if err != nil {
		t.Fatal(err)
	}
	if written != path {
		t.Fatalf("written = %q, want %q", written, path)
	}

	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(data) == 0 {
		t.Fatal("expected config file content")
	}

	cfg, _, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.LAN.MulticastAddr != DefaultLANMulticastAddr {
		t.Fatalf("multicast = %q, want %q", cfg.LAN.MulticastAddr, DefaultLANMulticastAddr)
	}
}

func TestSetGetAndSaveConfig(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.json")
	cfg := Default()

	if err := Set(&cfg, "client.server_url", "http://127.0.0.1:9527"); err != nil {
		t.Fatal(err)
	}
	if err := Set(&cfg, "token", "secret"); err != nil {
		t.Fatal(err)
	}

	if _, err := Save(path, cfg); err != nil {
		t.Fatal(err)
	}

	loaded, _, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	value, err := Get(loaded, "client.server_url")
	if err != nil {
		t.Fatal(err)
	}
	if value != "http://127.0.0.1:9527" {
		t.Fatalf("client.server_url = %q", value)
	}

	token, err := Get(loaded, "token")
	if err != nil {
		t.Fatal(err)
	}
	if token != "secret" {
		t.Fatalf("token = %q", token)
	}
}

func TestSetRejectsUnknownKey(t *testing.T) {
	cfg := Default()
	if err := Set(&cfg, "unknown", "value"); err == nil {
		t.Fatal("expected unknown key error")
	}
}

func TestSetRejectsInvalidValues(t *testing.T) {
	cfg := Default()
	if err := Set(&cfg, "log_format", "xml"); err == nil {
		t.Fatal("expected invalid log format error")
	}
	if err := Set(&cfg, "client.interval", "soon"); err == nil {
		t.Fatal("expected invalid duration error")
	}
}
