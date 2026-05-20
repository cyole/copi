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

func TestLoadConfigFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.json")
	data := []byte(`{
  "token": "secret",
  "log_format": "json",
  "client": {
    "relay_url": "http://127.0.0.1:9527",
    "interval": "250ms"
  },
  "lan": {
    "listen_addr": "127.0.0.1:0"
  }
}
`)
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}

	cfg, _, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Token != "secret" {
		t.Fatalf("token = %q", cfg.Token)
	}
	if cfg.LogFormat != "json" {
		t.Fatalf("log_format = %q", cfg.LogFormat)
	}
	if cfg.Client.RelayURL != "http://127.0.0.1:9527" {
		t.Fatalf("client.relay_url = %q", cfg.Client.RelayURL)
	}
	if cfg.Client.Interval != "250ms" {
		t.Fatalf("client.interval = %q", cfg.Client.Interval)
	}
	if cfg.Client.LongPollWait != DefaultLongPollWait {
		t.Fatalf("client.long_poll_wait = %q, want %q", cfg.Client.LongPollWait, DefaultLongPollWait)
	}
	if cfg.LAN.ListenAddr != "127.0.0.1:0" {
		t.Fatalf("lan.listen_addr = %q", cfg.LAN.ListenAddr)
	}
	if cfg.LAN.MulticastAddr != DefaultLANMulticastAddr {
		t.Fatalf("lan.multicast_addr = %q, want %q", cfg.LAN.MulticastAddr, DefaultLANMulticastAddr)
	}
}
