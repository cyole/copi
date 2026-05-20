package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/cyole/copi/internal/client"
	"github.com/cyole/copi/internal/clipboard"
	"github.com/cyole/copi/internal/config"
	"github.com/cyole/copi/internal/lan"
	"github.com/cyole/copi/internal/server"
)

const version = "0.3.0"

func main() {
	if len(os.Args) < 2 {
		usage()
		os.Exit(2)
	}

	logger := log.New(os.Stdout, "", log.LstdFlags)
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	var err error
	switch os.Args[1] {
	case "server", "relay":
		err = runRelay(ctx, os.Args[2:], logger)
	case "client":
		err = runClient(ctx, os.Args[2:], logger)
	case "lan":
		err = runLAN(ctx, os.Args[2:], logger)
	case "version":
		fmt.Println("copi", version)
	case "status":
		err = runStatus(os.Args[2:])
	case "-h", "--help", "help":
		usage()
	default:
		fmt.Fprintf(os.Stderr, "unknown command: %s\n\n", os.Args[1])
		usage()
		os.Exit(2)
	}

	if err != nil {
		logger.Printf("error: %v", err)
		os.Exit(1)
	}
}

func runRelay(ctx context.Context, args []string, logger *log.Logger) error {
	fs := flag.NewFlagSet("server", flag.ExitOnError)
	addr := fs.String("addr", envOr("COPI_ADDR", "0.0.0.0:9527"), "HTTP listen address")
	token := fs.String("token", os.Getenv("COPI_TOKEN"), "optional shared token")
	if err := fs.Parse(args); err != nil {
		return err
	}

	logger.Printf("starting third-party HTTP relay on %s; this process does not read or write any clipboard", *addr)
	srv := server.New(server.Options{
		Addr:   *addr,
		Token:  *token,
		Logger: logger,
	})
	return srv.Run(ctx)
}

func runClient(ctx context.Context, args []string, logger *log.Logger) error {
	fs := flag.NewFlagSet("client", flag.ExitOnError)
	serverURL := fs.String("server", os.Getenv("COPI_SERVER_URL"), "server URL, for example http://192.168.1.10:9527")
	token := fs.String("token", os.Getenv("COPI_TOKEN"), "optional shared token")
	name := fs.String("name", config.DefaultDeviceName(), "device name")
	id := fs.String("id", "", "device id; generated and persisted when omitted")
	interval := fs.Duration("interval", 500*time.Millisecond, "clipboard polling interval")
	wait := fs.Duration("wait", 30*time.Second, "server long-poll wait time")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if strings.TrimSpace(*serverURL) == "" {
		return fmt.Errorf("client mode requires --server")
	}

	deviceID, err := resolveDeviceID(*id)
	if err != nil {
		return err
	}

	logger.Printf("starting client mode as %s (%s)", *name, deviceID)
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

func runLAN(ctx context.Context, args []string, logger *log.Logger) error {
	fs := flag.NewFlagSet("lan", flag.ExitOnError)
	listen := fs.String("listen", envOr("COPI_LAN_LISTEN", "0.0.0.0:9528"), "local peer HTTP listen address")
	advertise := fs.String("advertise", os.Getenv("COPI_LAN_ADVERTISE_URL"), "HTTP URL announced to peers; auto-detected when omitted")
	multicast := fs.String("multicast", envOr("COPI_LAN_MULTICAST_ADDR", lan.DefaultMulticastAddress), "UDP multicast address for peer discovery")
	token := fs.String("token", os.Getenv("COPI_TOKEN"), "optional shared token")
	name := fs.String("name", config.DefaultDeviceName(), "device name")
	id := fs.String("id", "", "device id; generated and persisted when omitted")
	interval := fs.Duration("interval", 500*time.Millisecond, "clipboard polling interval")
	if err := fs.Parse(args); err != nil {
		return err
	}

	deviceID, err := resolveDeviceID(*id)
	if err != nil {
		return err
	}

	logger.Printf("starting LAN mode as %s (%s)", *name, deviceID)
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
	fmt.Println("commands: relay, server, client, lan, status, version")
	fmt.Println("capabilities: text_clipboard, docker_relay, lan_discovery")
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
  copi server [--addr 0.0.0.0:9527] [--token secret]
  copi relay [--addr 0.0.0.0:9527] [--token secret]
  copi client --server http://host:9527 [--token secret]
  copi lan [--listen 0.0.0.0:9528] [--token secret]
  copi status [--json]
  copi version

Modes:
  server   Third-party HTTP relay. It never touches the local clipboard.
  relay    Alias for server.
  client   Device-side clipboard client. Fill in the relay URL and it can sync.
  lan      Zero-config LAN mode. Peers discover each other and sync clipboard text directly.
  status   Machine-readable capabilities for native app shells.

`, version)
}
