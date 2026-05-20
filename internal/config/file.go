package config

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
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

func Init(path string, force bool) (string, error) {
	resolved, err := resolvePath(path)
	if err != nil {
		return "", err
	}
	if !force {
		if _, err := os.Stat(resolved); err == nil {
			return "", fmt.Errorf("config already exists: %s", resolved)
		} else if err != nil && !errors.Is(err, os.ErrNotExist) {
			return "", err
		}
	}

	cfg := Default()
	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return "", err
	}
	data = append(data, '\n')

	if err := os.MkdirAll(filepath.Dir(resolved), 0o700); err != nil {
		return "", err
	}
	if err := os.WriteFile(resolved, data, 0o600); err != nil {
		return "", err
	}
	return resolved, nil
}

func Save(path string, cfg Config) (string, error) {
	resolved, err := resolvePath(path)
	if err != nil {
		return "", err
	}
	cfg.ApplyDefaults()

	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return "", err
	}
	data = append(data, '\n')

	if err := os.MkdirAll(filepath.Dir(resolved), 0o700); err != nil {
		return "", err
	}
	if err := os.WriteFile(resolved, data, 0o600); err != nil {
		return "", err
	}
	return resolved, nil
}

func Get(cfg Config, key string) (string, error) {
	switch key {
	case "token":
		return cfg.Token, nil
	case "log_format":
		return cfg.LogFormat, nil
	case "device.id":
		return cfg.Device.ID, nil
	case "device.name":
		return cfg.Device.Name, nil
	case "relay.addr":
		return cfg.Relay.Addr, nil
	case "client.relay_url":
		return cfg.Client.RelayURL, nil
	case "client.interval":
		return cfg.Client.Interval, nil
	case "client.long_poll_wait":
		return cfg.Client.LongPollWait, nil
	case "lan.listen_addr":
		return cfg.LAN.ListenAddr, nil
	case "lan.advertise_url":
		return cfg.LAN.AdvertiseURL, nil
	case "lan.multicast_addr":
		return cfg.LAN.MulticastAddr, nil
	case "lan.interval":
		return cfg.LAN.Interval, nil
	default:
		return "", fmt.Errorf("unknown config key: %s", key)
	}
}

func Set(cfg *Config, key, value string) error {
	switch key {
	case "token":
		cfg.Token = value
	case "log_format":
		if !validLogFormat(value) {
			return fmt.Errorf("invalid log_format %q, expected text or json", value)
		}
		cfg.LogFormat = value
	case "device.id":
		cfg.Device.ID = value
	case "device.name":
		cfg.Device.Name = value
	case "relay.addr":
		cfg.Relay.Addr = value
	case "client.relay_url":
		cfg.Client.RelayURL = value
	case "client.interval":
		if err := validateDuration(key, value); err != nil {
			return err
		}
		cfg.Client.Interval = value
	case "client.long_poll_wait":
		if err := validateDuration(key, value); err != nil {
			return err
		}
		cfg.Client.LongPollWait = value
	case "lan.listen_addr":
		cfg.LAN.ListenAddr = value
	case "lan.advertise_url":
		cfg.LAN.AdvertiseURL = value
	case "lan.multicast_addr":
		cfg.LAN.MulticastAddr = value
	case "lan.interval":
		if err := validateDuration(key, value); err != nil {
			return err
		}
		cfg.LAN.Interval = value
	default:
		return fmt.Errorf("unknown config key: %s", key)
	}
	cfg.ApplyDefaults()
	return nil
}

func validLogFormat(value string) bool {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "text", "json":
		return true
	default:
		return false
	}
}

func validateDuration(key, value string) error {
	if _, err := time.ParseDuration(value); err != nil {
		return fmt.Errorf("invalid %s duration %q: %w", key, value, err)
	}
	return nil
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
