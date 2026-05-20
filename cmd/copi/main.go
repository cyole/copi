package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/cyole/copi/internal/client"
	"github.com/cyole/copi/internal/clipboard"
	"github.com/cyole/copi/internal/config"
	"github.com/cyole/copi/internal/doctor"
	"github.com/cyole/copi/internal/eventlog"
	"github.com/cyole/copi/internal/lan"
	"github.com/cyole/copi/internal/server"
)

const version = "0.3.0"

func main() {
	if len(os.Args) < 2 {
		usage()
		os.Exit(2)
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	var err error
	switch os.Args[1] {
	case "relay":
		err = runRelay(ctx, os.Args[2:])
	case "client":
		err = runClient(ctx, os.Args[2:])
	case "version":
		err = runVersion(os.Args[2:])
	case "doctor":
		err = runDoctor(ctx, os.Args[2:])
	case "-h", "--help", "help":
		usage()
	default:
		fmt.Fprintf(os.Stderr, "unknown command: %s\n\n", os.Args[1])
		usage()
		os.Exit(2)
	}

	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
}

func runRelay(ctx context.Context, args []string) error {
	cfg, configPath, err := loadConfig(args)
	if err != nil {
		return err
	}
	fs := flag.NewFlagSet("relay", flag.ExitOnError)
	_ = fs.String("config", configPath, "config file path")
	addr := fs.String("addr", envOr("COPI_ADDR", cfg.Relay.Addr), "HTTP listen address")
	token := fs.String("token", envOr("COPI_TOKEN", cfg.Token), "optional shared token")
	logFormat := fs.String("log-format", envOr("COPI_LOG_FORMAT", cfg.LogFormat), "log format: text or json")
	if err := fs.Parse(args); err != nil {
		return err
	}
	logger, err := commandLogger(*logFormat)
	if err != nil {
		return err
	}

	logger.Info("relay_starting", "starting third-party HTTP relay", eventlog.Fields{
		"addr":             *addr,
		"clipboard_access": false,
		"config":           configPath,
	})
	srv := server.New(server.Options{
		Addr:   *addr,
		Token:  *token,
		Logger: logger,
	})
	return srv.Run(ctx)
}

func runClient(ctx context.Context, args []string) error {
	cfg, configPath, err := loadConfig(args)
	if err != nil {
		return err
	}
	if boolFromArgs(args, "lan", false) {
		return runClientLAN(ctx, args, cfg, configPath)
	}
	return runClientRelay(ctx, args, cfg, configPath)
}

func runClientRelay(ctx context.Context, args []string, cfg config.Config, configPath string) error {
	intervalDefault, err := durationFrom(envOr("COPI_CLIENT_INTERVAL", cfg.Client.Interval), "client interval")
	if err != nil {
		return err
	}
	waitDefault, err := durationFrom(envOr("COPI_CLIENT_LONG_POLL_WAIT", cfg.Client.LongPollWait), "client long_poll_wait")
	if err != nil {
		return err
	}

	fs := flag.NewFlagSet("client", flag.ExitOnError)
	_ = fs.String("config", configPath, "config file path")
	_ = fs.Bool("lan", false, "use LAN mode")
	relayURL := fs.String("relay", envOr("COPI_RELAY_URL", cfg.Client.RelayURL), "relay URL, for example http://192.168.1.10:9527")
	token := fs.String("token", envOr("COPI_TOKEN", cfg.Token), "optional shared token")
	name := fs.String("name", envOr("COPI_DEVICE_NAME", cfg.Device.Name), "device name")
	id := fs.String("id", envOr("COPI_DEVICE_ID", cfg.Device.ID), "device id; generated and persisted when omitted")
	interval := fs.Duration("interval", intervalDefault, "clipboard polling interval")
	wait := fs.Duration("wait", waitDefault, "relay long-poll wait time")
	logFormat := fs.String("log-format", envOr("COPI_LOG_FORMAT", cfg.LogFormat), "log format: text or json")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if strings.TrimSpace(*relayURL) == "" {
		return fmt.Errorf("client requires --relay")
	}
	logger, err := commandLogger(*logFormat)
	if err != nil {
		return err
	}

	deviceID, err := resolveDeviceID(*id)
	if err != nil {
		return err
	}

	logger.Info("client_starting", "starting client", eventlog.Fields{
		"device_id":   deviceID,
		"device_name": *name,
		"relay":       *relayURL,
		"config":      configPath,
	})
	return client.Run(ctx, client.Options{
		ServerURL:    *relayURL,
		Token:        *token,
		DeviceID:     deviceID,
		DeviceName:   *name,
		Interval:     *interval,
		LongPollWait: *wait,
		Clipboard:    clipboard.NewSystem(),
		Logger:       logger,
	})
}

func runClientLAN(ctx context.Context, args []string, cfg config.Config, configPath string) error {
	intervalDefault, err := durationFrom(envOr("COPI_LAN_INTERVAL", cfg.LAN.Interval), "lan interval")
	if err != nil {
		return err
	}

	fs := flag.NewFlagSet("client", flag.ExitOnError)
	_ = fs.String("config", configPath, "config file path")
	_ = fs.Bool("lan", false, "use LAN mode")
	listen := fs.String("listen", envOr("COPI_LAN_LISTEN", cfg.LAN.ListenAddr), "local peer HTTP listen address")
	advertise := fs.String("advertise", envOr("COPI_LAN_ADVERTISE_URL", cfg.LAN.AdvertiseURL), "HTTP URL announced to peers; auto-detected when omitted")
	multicast := fs.String("multicast", envOr("COPI_LAN_MULTICAST_ADDR", cfg.LAN.MulticastAddr), "UDP multicast address for peer discovery")
	token := fs.String("token", envOr("COPI_TOKEN", cfg.Token), "optional shared token")
	name := fs.String("name", envOr("COPI_DEVICE_NAME", cfg.Device.Name), "device name")
	id := fs.String("id", envOr("COPI_DEVICE_ID", cfg.Device.ID), "device id; generated and persisted when omitted")
	interval := fs.Duration("interval", intervalDefault, "clipboard polling interval")
	logFormat := fs.String("log-format", envOr("COPI_LOG_FORMAT", cfg.LogFormat), "log format: text or json")
	if err := fs.Parse(args); err != nil {
		return err
	}
	logger, err := commandLogger(*logFormat)
	if err != nil {
		return err
	}

	deviceID, err := resolveDeviceID(*id)
	if err != nil {
		return err
	}

	logger.Info("lan_starting", "starting LAN", eventlog.Fields{
		"device_id":      deviceID,
		"device_name":    *name,
		"listen":         *listen,
		"advertise_url":  *advertise,
		"multicast_addr": *multicast,
		"config":         configPath,
	})
	return lan.Run(ctx, lan.Options{
		ListenAddr:    *listen,
		AdvertiseURL:  *advertise,
		MulticastAddr: *multicast,
		Token:         *token,
		DeviceID:      deviceID,
		DeviceName:    *name,
		Interval:      *interval,
		Clipboard:     clipboard.NewSystem(),
		Logger:        logger,
	})
}

func resolveDeviceID(given string) (string, error) {
	if strings.TrimSpace(given) != "" {
		return given, nil
	}
	return config.DeviceID()
}

func loadConfig(args []string) (config.Config, string, error) {
	return config.Load(configPathFromArgs(args))
}

func configPathFromArgs(args []string) string {
	return valueFromArgs(args, "config", "")
}

func valueFromArgs(args []string, key, fallback string) string {
	long := "--" + key
	prefix := long + "="
	for i, arg := range args {
		if arg == long && i+1 < len(args) {
			return args[i+1]
		}
		if strings.HasPrefix(arg, prefix) {
			return strings.TrimPrefix(arg, prefix)
		}
	}
	return fallback
}

func boolFromArgs(args []string, key string, fallback bool) bool {
	long := "--" + key
	prefix := long + "="
	for _, arg := range args {
		if arg == long {
			return true
		}
		if strings.HasPrefix(arg, prefix) {
			switch strings.ToLower(strings.TrimSpace(strings.TrimPrefix(arg, prefix))) {
			case "1", "t", "true", "y", "yes", "on":
				return true
			case "0", "f", "false", "n", "no", "off":
				return false
			}
		}
	}
	return fallback
}

func durationFrom(value, label string) (time.Duration, error) {
	duration, err := time.ParseDuration(value)
	if err != nil {
		return 0, fmt.Errorf("invalid %s duration %q: %w", label, value, err)
	}
	return duration, nil
}

func commandLogger(rawFormat string) (*eventlog.Logger, error) {
	format, err := eventlog.ParseFormat(rawFormat)
	if err != nil {
		return nil, err
	}
	return eventlog.New(os.Stdout, format), nil
}

func runDoctor(ctx context.Context, args []string) error {
	cfg, configPath, err := loadConfig(args)
	if err != nil {
		return err
	}
	timeoutDefault, err := durationFrom("3s", "doctor timeout")
	if err != nil {
		return err
	}

	fs := flag.NewFlagSet("doctor", flag.ExitOnError)
	_ = fs.String("config", configPath, "config file path")
	asJSON := fs.Bool("json", false, "print machine-readable JSON")
	mode := fs.String("mode", "all", "doctor mode: all, relay, client, or lan")
	relayURL := fs.String("relay", envOr("COPI_RELAY_URL", cfg.Client.RelayURL), "relay URL to check")
	timeout := fs.Duration("timeout", timeoutDefault, "network check timeout")
	checkClipboard := fs.Bool("clipboard", false, "check clipboard read access")
	if err := fs.Parse(args); err != nil {
		return err
	}

	report := doctor.Run(ctx, doctor.Options{
		ConfigPath:     configPath,
		Config:         cfg,
		Mode:           *mode,
		ServerURL:      *relayURL,
		Timeout:        *timeout,
		CheckClipboard: *checkClipboard,
		Clipboard:      clipboard.NewSystem(),
	})

	if *asJSON {
		encoder := json.NewEncoder(os.Stdout)
		encoder.SetIndent("", "  ")
		if err := encoder.Encode(report); err != nil {
			return err
		}
	} else {
		printDoctorText(report)
	}

	if !report.OK {
		return fmt.Errorf("doctor found %d failing check(s)", report.Summary.Fail)
	}
	return nil
}

func printDoctorText(report doctor.Report) {
	fmt.Printf("copi doctor: mode=%s ok=%v\n", report.Mode, report.OK)
	for _, check := range report.Checks {
		fmt.Printf("[%s] %s: %s\n", check.Status, check.Name, check.Message)
	}
}

func runVersion(args []string) error {
	fs := flag.NewFlagSet("version", flag.ExitOnError)
	asJSON := fs.Bool("json", false, "print machine-readable JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}

	info := map[string]any{
		"name":    "copi",
		"version": version,
		"kind":    "core-cli",
		"commands": []string{
			"relay",
			"client",
			"doctor",
			"version",
		},
		"capabilities": map[string]bool{
			"text_clipboard":      true,
			"image_clipboard":     false,
			"file_clipboard":      false,
			"rich_text_clipboard": false,
			"docker_relay":        true,
			"doctor":              true,
			"json_logs":           true,
			"lan_discovery":       true,
		},
	}

	if *asJSON {
		encoder := json.NewEncoder(os.Stdout)
		encoder.SetIndent("", "  ")
		return encoder.Encode(info)
	}

	fmt.Println("copi", version)
	return nil
}

func envOr(key, fallback string) string {
	value := strings.TrimSpace(os.Getenv(key))
	if value == "" {
		return fallback
	}
	return value
}

func usage() {
	fmt.Fprintf(os.Stderr, `copi %s

Usage:
  copi relay [--addr 0.0.0.0:9527] [--token secret] [--log-format text|json]
  copi client --relay http://host:9527 [--token secret] [--log-format text|json]
  copi client --lan [--listen 0.0.0.0:9528] [--token secret] [--log-format text|json]
  copi doctor [--json] [--mode all|relay|client|lan]
  copi version [--json]

Commands:
  relay    Third-party HTTP service. It never touches the local clipboard.
  client   Device-side sync through a relay URL, or LAN sync with --lan.
  doctor   Diagnose relay, LAN, and optional clipboard access.
  version  Print version and machine-readable capabilities.

`, version)
}
