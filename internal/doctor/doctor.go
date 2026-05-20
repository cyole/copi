package doctor

import (
	"context"
	"fmt"
	"net"
	"net/http"
	"net/url"
	"os"
	"strings"
	"time"

	"github.com/cyole/copi/internal/clipboard"
	"github.com/cyole/copi/internal/config"
	"github.com/cyole/copi/internal/eventlog"
)

type Status string

const (
	StatusOK      Status = "ok"
	StatusWarn    Status = "warn"
	StatusFail    Status = "fail"
	StatusSkipped Status = "skipped"
)

type Options struct {
	ConfigPath     string
	Config         config.Config
	Mode           string
	ServerURL      string
	Timeout        time.Duration
	CheckClipboard bool
	Clipboard      clipboard.Provider
}

type Report struct {
	OK         bool      `json:"ok"`
	Mode       string    `json:"mode"`
	ConfigPath string    `json:"config_path"`
	Checks     []Check   `json:"checks"`
	Summary    Summary   `json:"summary"`
	Generated  time.Time `json:"generated"`
}

type Summary struct {
	OK      int `json:"ok"`
	Warn    int `json:"warn"`
	Fail    int `json:"fail"`
	Skipped int `json:"skipped"`
}

type Check struct {
	Name    string         `json:"name"`
	Status  Status         `json:"status"`
	Message string         `json:"message"`
	Details map[string]any `json:"details,omitempty"`
}

func Run(ctx context.Context, opts Options) Report {
	if opts.Mode == "" {
		opts.Mode = "all"
	}
	if opts.Timeout <= 0 {
		opts.Timeout = 3 * time.Second
	}
	if opts.Clipboard == nil {
		opts.Clipboard = clipboard.NewSystem()
	}

	report := Report{
		OK:         true,
		Mode:       opts.Mode,
		ConfigPath: opts.ConfigPath,
		Generated:  time.Now().UTC(),
	}

	if !validMode(opts.Mode) {
		report.add(Check{Name: "doctor.mode", Status: StatusFail, Message: "mode must be one of all, relay, client, or lan"})
		return report
	}

	report.add(checkConfigPath(opts.ConfigPath))
	report.add(checkLogFormat(opts.Config.LogFormat))
	report.add(checkDurations(opts.Config))
	report.add(checkToken(opts.Config.Token))

	switch opts.Mode {
	case "all", "client":
		report.add(checkServerURL(ctx, opts.Config, opts.ServerURL, opts.Timeout))
	case "relay":
		report.add(checkRelayListen(opts.Config.Relay.Addr))
	}

	switch opts.Mode {
	case "all", "lan":
		report.add(checkLANListen(opts.Config.LAN.ListenAddr))
		report.add(checkMulticast(opts.Config.LAN.MulticastAddr))
	}

	if opts.CheckClipboard {
		report.add(checkClipboardRead(opts.Clipboard))
	} else {
		report.add(Check{
			Name:    "clipboard.read",
			Status:  StatusSkipped,
			Message: "clipboard check is skipped; pass --clipboard to run it",
		})
	}

	return report
}

func validMode(mode string) bool {
	switch mode {
	case "all", "relay", "client", "lan":
		return true
	default:
		return false
	}
}

func (r *Report) add(check Check) {
	r.Checks = append(r.Checks, check)
	switch check.Status {
	case StatusOK:
		r.Summary.OK++
	case StatusWarn:
		r.Summary.Warn++
	case StatusFail:
		r.Summary.Fail++
		r.OK = false
	case StatusSkipped:
		r.Summary.Skipped++
	}
}

func checkConfigPath(path string) Check {
	if strings.TrimSpace(path) == "" {
		return Check{Name: "config.path", Status: StatusFail, Message: "config path is empty"}
	}
	details := map[string]any{"path": path}
	if _, err := os.Stat(path); err != nil {
		if os.IsNotExist(err) {
			return Check{Name: "config.path", Status: StatusWarn, Message: "config file does not exist; defaults will be used", Details: details}
		}
		return Check{Name: "config.path", Status: StatusFail, Message: err.Error(), Details: details}
	}
	return Check{Name: "config.path", Status: StatusOK, Message: "config file is readable", Details: details}
}

func checkLogFormat(format string) Check {
	if _, err := eventlog.ParseFormat(format); err != nil {
		return Check{Name: "config.log_format", Status: StatusFail, Message: err.Error()}
	}
	return Check{Name: "config.log_format", Status: StatusOK, Message: "log format is valid", Details: map[string]any{"value": format}}
}

func checkDurations(cfg config.Config) Check {
	values := map[string]string{
		"client.interval":       cfg.Client.Interval,
		"client.long_poll_wait": cfg.Client.LongPollWait,
		"lan.interval":          cfg.LAN.Interval,
	}
	for key, value := range values {
		if _, err := time.ParseDuration(value); err != nil {
			return Check{Name: "config.durations", Status: StatusFail, Message: fmt.Sprintf("%s is invalid: %v", key, err)}
		}
	}
	return Check{Name: "config.durations", Status: StatusOK, Message: "durations are valid"}
}

func checkToken(token string) Check {
	if strings.TrimSpace(token) == "" {
		return Check{Name: "security.token", Status: StatusWarn, Message: "token is not configured"}
	}
	return Check{Name: "security.token", Status: StatusOK, Message: "token is configured"}
}

func checkServerURL(ctx context.Context, cfg config.Config, override string, timeout time.Duration) Check {
	serverURL := strings.TrimSpace(override)
	if serverURL == "" {
		serverURL = strings.TrimSpace(cfg.Client.RelayURL)
	}
	if serverURL == "" {
		return Check{Name: "client.relay_url", Status: StatusSkipped, Message: "relay URL is not configured"}
	}
	if !strings.Contains(serverURL, "://") {
		serverURL = "http://" + serverURL
	}
	parsed, err := url.Parse(serverURL)
	if err != nil || parsed.Host == "" {
		return Check{Name: "client.relay_url", Status: StatusFail, Message: "relay URL is invalid", Details: map[string]any{"relay_url": serverURL}}
	}

	healthURL := strings.TrimRight(parsed.String(), "/") + "/health"
	reqCtx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	req, err := http.NewRequestWithContext(reqCtx, http.MethodGet, healthURL, nil)
	if err != nil {
		return Check{Name: "client.relay_health", Status: StatusFail, Message: err.Error(), Details: map[string]any{"url": healthURL}}
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return Check{Name: "client.relay_health", Status: StatusWarn, Message: err.Error(), Details: map[string]any{"url": healthURL}}
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return Check{Name: "client.relay_health", Status: StatusWarn, Message: fmt.Sprintf("relay returned %s", resp.Status), Details: map[string]any{"url": healthURL}}
	}
	return Check{Name: "client.relay_health", Status: StatusOK, Message: "relay health check passed", Details: map[string]any{"url": healthURL}}
}

func checkRelayListen(addr string) Check {
	if strings.TrimSpace(addr) == "" {
		return Check{Name: "relay.listen", Status: StatusFail, Message: "relay listen address is empty"}
	}
	listener, err := net.Listen("tcp", addr)
	if err != nil {
		return Check{Name: "relay.listen", Status: StatusFail, Message: err.Error(), Details: map[string]any{"addr": addr}}
	}
	_ = listener.Close()
	return Check{Name: "relay.listen", Status: StatusOK, Message: "relay listen address is available", Details: map[string]any{"addr": addr}}
}

func checkLANListen(addr string) Check {
	if strings.TrimSpace(addr) == "" {
		return Check{Name: "lan.listen", Status: StatusFail, Message: "LAN listen address is empty"}
	}
	listener, err := net.Listen("tcp", addr)
	if err != nil {
		return Check{Name: "lan.listen", Status: StatusFail, Message: err.Error(), Details: map[string]any{"addr": addr}}
	}
	_ = listener.Close()
	return Check{Name: "lan.listen", Status: StatusOK, Message: "LAN listen address is available", Details: map[string]any{"addr": addr}}
}

func checkMulticast(addr string) Check {
	udpAddr, err := net.ResolveUDPAddr("udp4", addr)
	if err != nil {
		return Check{Name: "lan.multicast", Status: StatusFail, Message: err.Error(), Details: map[string]any{"addr": addr}}
	}
	if udpAddr.IP == nil || !udpAddr.IP.IsMulticast() {
		return Check{Name: "lan.multicast", Status: StatusFail, Message: "address is not multicast", Details: map[string]any{"addr": addr}}
	}
	return Check{Name: "lan.multicast", Status: StatusOK, Message: "multicast address is valid", Details: map[string]any{"addr": addr}}
}

func checkClipboardRead(provider clipboard.Provider) Check {
	text, err := provider.ReadText()
	if err != nil {
		return Check{Name: "clipboard.read", Status: StatusWarn, Message: err.Error()}
	}
	return Check{Name: "clipboard.read", Status: StatusOK, Message: "clipboard text read succeeded", Details: map[string]any{"bytes": len(text)}}
}
