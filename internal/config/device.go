package config

import (
	"crypto/rand"
	"encoding/hex"
	"os"
	"path/filepath"
	"strings"
)

func DefaultDeviceName() string {
	host, err := os.Hostname()
	if err != nil || strings.TrimSpace(host) == "" {
		return "copi-device"
	}
	return host
}

func DeviceID() (string, error) {
	dir, err := os.UserConfigDir()
	if err != nil {
		return randomID()
	}
	path := filepath.Join(dir, "copi", "device_id")

	if data, err := os.ReadFile(path); err == nil {
		if id := strings.TrimSpace(string(data)); id != "" {
			return id, nil
		}
	}

	id, err := randomID()
	if err != nil {
		return "", err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return id, nil
	}
	_ = os.WriteFile(path, []byte(id+"\n"), 0o600)
	return id, nil
}

func randomID() (string, error) {
	var data [16]byte
	if _, err := rand.Read(data[:]); err != nil {
		return "", err
	}
	return "dev-" + hex.EncodeToString(data[:]), nil
}
