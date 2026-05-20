package config

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

const (
	DefaultRelayAddr        = "0.0.0.0:9527"
	DefaultLANListenAddr    = "0.0.0.0:9528"
	DefaultLANMulticastAddr = "239.255.27.42:9529"
	DefaultLogFormat        = "text"
	DefaultClientInterval   = "500ms"
	DefaultLongPollWait     = "30s"
	DefaultLANInterval      = "500ms"
)

type Config struct {
	Token     string       `json:"token,omitempty"`
	LogFormat string       `json:"log_format,omitempty"`
	Device    DeviceConfig `json:"device,omitempty"`
	Relay     RelayConfig  `json:"relay,omitempty"`
	Client    ClientConfig `json:"client,omitempty"`
	LAN       LANConfig    `json:"lan,omitempty"`
}

type DeviceConfig struct {
	ID   string `json:"id,omitempty"`
	Name string `json:"name,omitempty"`
}

type RelayConfig struct {
	Addr string `json:"addr,omitempty"`
}

type ClientConfig struct {
	RelayURL     string `json:"relay_url,omitempty"`
	Interval     string `json:"interval,omitempty"`
	LongPollWait string `json:"long_poll_wait,omitempty"`
}

type LANConfig struct {
	ListenAddr    string `json:"listen_addr,omitempty"`
	AdvertiseURL  string `json:"advertise_url,omitempty"`
	MulticastAddr string `json:"multicast_addr,omitempty"`
	Interval      string `json:"interval,omitempty"`
}

func Default() Config {
	return Config{
		LogFormat: DefaultLogFormat,
		Device: DeviceConfig{
			Name: DefaultDeviceName(),
		},
		Relay: RelayConfig{
			Addr: DefaultRelayAddr,
		},
		Client: ClientConfig{
			Interval:     DefaultClientInterval,
			LongPollWait: DefaultLongPollWait,
		},
		LAN: LANConfig{
			ListenAddr:    DefaultLANListenAddr,
			MulticastAddr: DefaultLANMulticastAddr,
			Interval:      DefaultLANInterval,
		},
	}
}

func DefaultPath() (string, error) {
	dir, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "copi", "config.json"), nil
}

func Load(path string) (Config, string, error) {
	resolved, err := resolvePath(path)
	if err != nil {
		return Config{}, "", err
	}

	cfg := Default()
	data, err := os.ReadFile(resolved)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return cfg, resolved, nil
		}
		return Config{}, resolved, err
	}

	if err := json.Unmarshal(data, &cfg); err != nil {
		return Config{}, resolved, fmt.Errorf("load config %s: %w", resolved, err)
	}
	cfg.ApplyDefaults()
	return cfg, resolved, nil
}

func (c *Config) ApplyDefaults() {
	if c.LogFormat == "" {
		c.LogFormat = DefaultLogFormat
	}
	if c.Device.Name == "" {
		c.Device.Name = DefaultDeviceName()
	}
	if c.Relay.Addr == "" {
		c.Relay.Addr = DefaultRelayAddr
	}
	if c.Client.Interval == "" {
		c.Client.Interval = DefaultClientInterval
	}
	if c.Client.LongPollWait == "" {
		c.Client.LongPollWait = DefaultLongPollWait
	}
	if c.LAN.ListenAddr == "" {
		c.LAN.ListenAddr = DefaultLANListenAddr
	}
	if c.LAN.MulticastAddr == "" {
		c.LAN.MulticastAddr = DefaultLANMulticastAddr
	}
	if c.LAN.Interval == "" {
		c.LAN.Interval = DefaultLANInterval
	}
}

func resolvePath(path string) (string, error) {
	if path != "" {
		return path, nil
	}
	return DefaultPath()
}
