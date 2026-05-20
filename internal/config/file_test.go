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
