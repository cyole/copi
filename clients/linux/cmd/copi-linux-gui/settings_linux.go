//go:build linux

package main

import (
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

const (
	appID            = "com.cyole.copi"
	appName          = "Copi"
	configFileName   = "linux-gui.json"
	defaultRelayURL  = "http://127.0.0.1:9527"
	iconFileName     = "com.cyole.copi-symbolic.svg"
	installedIcon    = "/usr/share/icons/hicolor/scalable/apps/com.cyole.copi.svg"
	installedCLI     = "/usr/bin/copi"
	installedGUI     = "/usr/bin/copi-linux-gui"
	desktopEntryName = "com.cyole.copi.desktop"
)

type syncMode string

const (
	modeRelay syncMode = "relay"
	modeLAN   syncMode = "lan"
)

type appSettings struct {
	Mode          syncMode `json:"mode"`
	DeviceName    string   `json:"device_name"`
	DeviceID      string   `json:"device_id"`
	RelayURL      string   `json:"relay_url"`
	AccessToken   string   `json:"access_token"`
	SyncKey       string   `json:"sync_key"`
	LaunchAtLogin bool     `json:"launch_at_login"`
}

func defaultSettings() appSettings {
	hostname, _ := os.Hostname()
	if strings.TrimSpace(hostname) == "" {
		hostname = "Linux"
	}

	return appSettings{
		Mode:          modeRelay,
		DeviceName:    hostname,
		DeviceID:      generateDeviceID(),
		RelayURL:      defaultRelayURL,
		AccessToken:   "",
		SyncKey:       generateSyncKey(),
		LaunchAtLogin: false,
	}
}

func loadSettings() (appSettings, error) {
	settings := defaultSettings()
	path, err := settingsPath()
	if err != nil {
		return settings, err
	}

	data, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return settings, nil
		}
		return settings, err
	}
	if err := json.Unmarshal(data, &settings); err != nil {
		return defaultSettings(), err
	}
	settings.applyDefaults()
	return settings, nil
}

func (s *appSettings) applyDefaults() {
	if s.Mode != modeRelay && s.Mode != modeLAN {
		s.Mode = modeRelay
	}
	if strings.TrimSpace(s.DeviceName) == "" {
		hostname, _ := os.Hostname()
		if strings.TrimSpace(hostname) == "" {
			hostname = "Linux"
		}
		s.DeviceName = hostname
	}
	if strings.TrimSpace(s.DeviceID) == "" {
		s.DeviceID = generateDeviceID()
	}
	if strings.TrimSpace(s.RelayURL) == "" {
		s.RelayURL = defaultRelayURL
	}
	if strings.TrimSpace(s.SyncKey) == "" {
		s.SyncKey = generateSyncKey()
	}
}

func (s appSettings) save() error {
	path, err := settingsPath()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	if err := os.WriteFile(path, data, 0o600); err != nil {
		return err
	}
	return syncAutostart(s.LaunchAtLogin)
}

func (s appSettings) modeTitle() string {
	if s.Mode == modeLAN {
		return "局域网"
	}
	return "中转模式"
}

func (s appSettings) secret() string {
	if s.Mode == modeLAN {
		return s.SyncKey
	}
	return s.AccessToken
}

func (s appSettings) runnable() bool {
	return s.Mode == modeLAN || strings.TrimSpace(s.RelayURL) != ""
}

func settingsPath() (string, error) {
	dir, err := configHome()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "copi", configFileName), nil
}

func autostartPath() (string, error) {
	dir, err := configHome()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "autostart", desktopEntryName), nil
}

func configHome() (string, error) {
	if dir := strings.TrimSpace(os.Getenv("XDG_CONFIG_HOME")); dir != "" {
		return dir, nil
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".config"), nil
}

func generateDeviceID() string {
	return "linux-" + randomUUID()
}

func generateSyncKey() string {
	bytes := make([]byte, 24)
	if _, err := rand.Read(bytes); err != nil {
		return randomUUID() + randomUUID()
	}
	return base64.StdEncoding.EncodeToString(bytes)
}

func randomUUID() string {
	bytes := make([]byte, 16)
	if _, err := rand.Read(bytes); err != nil {
		return fmt.Sprintf("%x", os.Getpid())
	}
	bytes[6] = (bytes[6] & 0x0f) | 0x40
	bytes[8] = (bytes[8] & 0x3f) | 0x80
	return fmt.Sprintf("%x-%x-%x-%x-%x", bytes[0:4], bytes[4:6], bytes[6:8], bytes[8:10], bytes[10:])
}

func syncAutostart(enabled bool) error {
	path, err := autostartPath()
	if err != nil {
		return err
	}
	if !enabled {
		if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
			return err
		}
		return nil
	}

	executable, err := os.Executable()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}

	content := strings.Join([]string{
		"[Desktop Entry]",
		"Type=Application",
		"Name=Copi",
		"Exec=" + desktopExecQuote(executable),
		"Icon=com.cyole.copi",
		"Terminal=false",
		"X-GNOME-Autostart-enabled=true",
		"Categories=Utility;",
		"",
	}, "\n")
	return os.WriteFile(path, []byte(content), 0o644)
}

func desktopExecQuote(value string) string {
	value = strings.ReplaceAll(value, `\`, `\\`)
	value = strings.ReplaceAll(value, `"`, `\"`)
	return `"` + value + `"`
}

func resolveCLIPath() (string, error) {
	candidates := []string{
		os.Getenv("COPI_CLI"),
		siblingPath("copi"),
		filepath.Join(executableParentDir(), "build", "copi"),
		installedCLI,
	}
	if path, err := exec.LookPath("copi"); err == nil {
		candidates = append(candidates, path)
	}

	for _, candidate := range candidates {
		candidate = strings.TrimSpace(candidate)
		if candidate == "" {
			continue
		}
		if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("找不到 copi CLI，请先运行 clients/linux/run.sh 或安装 copi")
}

func resolveIconPath() string {
	candidates := []string{
		os.Getenv("COPI_ICON"),
		filepath.Join(executableParentDir(), "assets", iconFileName),
		filepath.Join(executableParentDir(), "share", "icons", "hicolor", "scalable", "apps", "com.cyole.copi.svg"),
		installedIcon,
		"/usr/share/pixmaps/com.cyole.copi.svg",
	}
	for _, candidate := range candidates {
		candidate = strings.TrimSpace(candidate)
		if candidate == "" {
			continue
		}
		if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
			return candidate
		}
	}
	return ""
}

func siblingPath(name string) string {
	executable, err := os.Executable()
	if err != nil {
		return name
	}
	return filepath.Join(filepath.Dir(executable), name)
}

func executableParentDir() string {
	executable, err := os.Executable()
	if err != nil {
		return "."
	}
	return filepath.Dir(filepath.Dir(executable))
}
