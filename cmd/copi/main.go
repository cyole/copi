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
	case "server", "relay":
		err = runRelay(ctx, os.Args[2:])
	case "client":
		err = runClient(ctx, os.Args[2:])
	case "lan":
		err = runLAN(ctx, os.Args[2:])
	case "version":
		fmt.Println("copi", version)
	case "status":
		err = runStatus(os.Args[2:])
	case "config":
		err = runConfig(os.Args[2:])
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
	fs := flag.NewFlagSet("server", flag.ExitOnError)
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
	serverURL := fs.String("server", envOr("COPI_SERVER_URL", cfg.Client.ServerURL), "server URL, for example http://192.168.1.10:9527")
	token := fs.String("token", envOr("COPI_TOKEN", cfg.Token), "optional shared token")
	name := fs.String("name", envOr("COPI_DEVICE_NAME", cfg.Device.Name), "device name")
	id := fs.String("id", envOr("COPI_DEVICE_ID", cfg.Device.ID), "device id; generated and persisted when omitted")
	interval := fs.Duration("interval", intervalDefault, "clipboard polling interval")
	wait := fs.Duration("wait", waitDefault, "server long-poll wait time")
	logFormat := fs.String("log-format", envOr("COPI_LOG_FORMAT", cfg.LogFormat), "log format: text or json")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if strings.TrimSpace(*serverURL) == "" {
		return fmt.Errorf("client mode requires --server")
	}
	logger, err := commandLogger(*logFormat)
	if err != nil {
		return err
	}

	deviceID, err := resolveDeviceID(*id)
	if err != nil {
		return err
	}

	logger.Info("client_starting", "starting client mode", eventlog.Fields{
		"device_id":   deviceID,
		"device_name": *name,
		"server":      *serverURL,
		"config":      configPath,
	})
	return client.Run(ctx, client.Options{
		ServerURL:    *serverURL,
		Token:        *token,
		DeviceID:     deviceID,
		DeviceName:   *name,
		Interval:     *interval,
		LongPollWait: *wait,
		Clipboard:    clipboard.NewSystem(),
		Logger:       logger,
	})
}

func runLAN(ctx context.Context, args []string) error {
	cfg, configPath, err := loadConfig(args)
	if err != nil {
		return err
	}
	intervalDefault, err := durationFrom(envOr("COPI_LAN_INTERVAL", cfg.LAN.Interval), "lan interval")
	if err != nil {
		return err
	}

	fs := flag.NewFlagSet("lan", flag.ExitOnError)
	_ = fs.String("config", configPath, "config file path")
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

	logger.Info("lan_starting", "starting LAN mode", eventlog.Fields{
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
	for i, arg := range args {
		if arg == "--config" && i+1 < len(args) {
			return args[i+1]
		}
		if strings.HasPrefix(arg, "--config=") {
			return strings.TrimPrefix(arg, "--config=")
		}
	}
	return ""
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

func runConfig(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("config requires a subcommand: path, init, show, get, or set")
	}

	switch args[0] {
	case "path":
		fs := flag.NewFlagSet("config path", flag.ExitOnError)
		configPath := fs.String("config", "", "config file path")
		if err := fs.Parse(args[1:]); err != nil {
			return err
		}
		_, resolved, err := config.Load(*configPath)
		if err != nil {
			return err
		}
		fmt.Println(resolved)
		return nil
	case "init":
		fs := flag.NewFlagSet("config init", flag.ExitOnError)
		configPath := fs.String("config", "", "config file path")
		force := fs.Bool("force", false, "overwrite existing config")
		if err := fs.Parse(args[1:]); err != nil {
			return err
		}
		resolved, err := config.Init(*configPath, *force)
		if err != nil {
			return err
		}
		fmt.Println(resolved)
		return nil
	case "show":
		fs := flag.NewFlagSet("config show", flag.ExitOnError)
		configPath := fs.String("config", "", "config file path")
		if err := fs.Parse(args[1:]); err != nil {
			return err
		}
		cfg, resolved, err := config.Load(*configPath)
		if err != nil {
			return err
		}
		output := struct {
			Path   string        `json:"path"`
			Config config.Config `json:"config"`
		}{
			Path:   resolved,
			Config: cfg,
		}
		encoder := json.NewEncoder(os.Stdout)
		encoder.SetIndent("", "  ")
		return encoder.Encode(output)
	case "get":
		fs := flag.NewFlagSet("config get", flag.ExitOnError)
		configPath := fs.String("config", "", "config file path")
		if err := fs.Parse(args[1:]); err != nil {
			return err
		}
		if fs.NArg() != 1 {
			return fmt.Errorf("config get requires exactly one key")
		}
		cfg, _, err := config.Load(*configPath)
		if err != nil {
			return err
		}
		value, err := config.Get(cfg, fs.Arg(0))
		if err != nil {
			return err
		}
		fmt.Println(value)
		return nil
	case "set":
		fs := flag.NewFlagSet("config set", flag.ExitOnError)
		configPath := fs.String("config", "", "config file path")
		if err := fs.Parse(args[1:]); err != nil {
			return err
		}
		if fs.NArg() != 2 {
			return fmt.Errorf("config set requires a key and value")
		}
		cfg, _, err := config.Load(*configPath)
		if err != nil {
			return err
		}
		if err := config.Set(&cfg, fs.Arg(0), fs.Arg(1)); err != nil {
			return err
		}
		resolved, err := config.Save(*configPath, cfg)
		if err != nil {
			return err
		}
		fmt.Println(resolved)
		return nil
	default:
		return fmt.Errorf("unknown config subcommand: %s", args[0])
	}
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
	serverURL := fs.String("server", envOr("COPI_SERVER_URL", cfg.Client.ServerURL), "relay URL to check")
	timeout := fs.Duration("timeout", timeoutDefault, "network check timeout")
	checkClipboard := fs.Bool("clipboard", false, "check clipboard read access")
	if err := fs.Parse(args); err != nil {
		return err
	}

	report := doctor.Run(ctx, doctor.Options{
		ConfigPath:     configPath,
		Config:         cfg,
		Mode:           *mode,
		ServerURL:      *serverURL,
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

func runStatus(args []string) error {
	fs := flag.NewFlagSet("status", flag.ExitOnError)
	asJSON := fs.Bool("json", false, "print machine-readable JSON")
	if err := fs.Parse(args); err != nil {
		return err
	}

	status := map[string]any{
		"name":    "copi",
		"version": version,
		"kind":    "core-cli",
		"commands": []string{
			"relay",
			"server",
			"client",
			"lan",
			"config",
			"doctor",
			"status",
			"version",
		},
		"modes": []string{
			"third-party-http-relay",
			"server-mode-client",
			"lan-peer",
		},
		"capabilities": map[string]bool{
			"text_clipboard":      true,
			"image_clipboard":     false,
			"file_clipboard":      false,
			"rich_text_clipboard": false,
			"docker_relay":        true,
			"file_config":         true,
			"doctor":              true,
			"json_logs":           true,
			"lan_discovery":       true,
		},
	}

	if *asJSON {
		encoder := json.NewEncoder(os.Stdout)
		encoder.SetIndent("", "  ")
		return encoder.Encode(status)
	}

	fmt.Println("copi", version)
	fmt.Println("kind: core-cli")
	fmt.Println("commands: relay, server, client, lan, config, doctor, status, version")
	fmt.Println("capabilities: text_clipboard, docker_relay, doctor, file_config, json_logs, lan_discovery")
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
  copi server [--addr 0.0.0.0:9527] [--token secret] [--log-format text|json]
  copi relay [--addr 0.0.0.0:9527] [--token secret] [--log-format text|json]
  copi client --server http://host:9527 [--token secret] [--log-format text|json]
  copi lan [--listen 0.0.0.0:9528] [--token secret] [--log-format text|json]
  copi config path|init|show|get|set
  copi doctor [--json] [--mode all|relay|client|lan]
  copi status [--json]
  copi version

Modes:
  server   Third-party HTTP relay. It never touches the local clipboard.
  relay    Alias for server.
  client   Device-side clipboard client. Fill in the relay URL and it can sync.
  lan      Zero-config LAN mode. Peers discover each other and sync clipboard text directly.
  config   Manage the CLI config file used by native shells.
  doctor   Diagnose config, relay, LAN, and optional clipboard access.
  status   Machine-readable capabilities for native app shells.

`, version)
}
