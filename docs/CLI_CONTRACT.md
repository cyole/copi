# CLI Contract

Native clients should treat `copi` as the shared sync engine. The shell-facing interface is intentionally small.

## Commands

```text
copi relay
copi client --relay <url>
copi client --lan
copi config
copi config set <key> <value>
copi doctor
copi version
```

## Configuration

Show resolved config:

```bash
copi config
```

Use a custom config file:

```bash
copi config --config ./copi.json
copi client --config ./copi.json --relay http://127.0.0.1:9527
```

Set values:

```bash
copi config set client.relay_url http://127.0.0.1:9527
copi config set token replace-this
copi config set log_format json
```

Precedence is:

1. CLI flags
2. Environment variables
3. Config file
4. Built-in defaults

Supported config keys:

- `token`
- `log_format`
- `device.id`
- `device.name`
- `relay.addr`
- `client.relay_url`
- `client.interval`
- `client.long_poll_wait`
- `lan.listen_addr`
- `lan.advertise_url`
- `lan.multicast_addr`
- `lan.interval`

## Doctor

```bash
copi doctor --json
copi doctor --mode client --relay http://127.0.0.1:9527 --json
copi doctor --mode lan --json
copi doctor --mode relay --json
copi doctor --clipboard --json
```

`doctor` checks config parsing, token presence, log format, durations, relay reachability, LAN listen address availability, multicast address validity, and optional clipboard read access.

## JSON Logs

Long-running commands support:

```bash
copi relay --log-format json
copi client --relay http://127.0.0.1:9527 --log-format json
copi client --lan --log-format json
```

Each line is one JSON event:

```json
{
  "time": "2026-05-20T00:00:00Z",
  "level": "info",
  "type": "started",
  "message": "client started"
}
```

Important event types:

- `started`
- `relay_starting`
- `relay_started`
- `client_starting`
- `lan_starting`
- `lan_peer_listening`
- `peer_discovered`
- `clipboard_published`
- `clipboard_applied`
- `clipboard_broadcast`
- `clipboard_read_failed`
- `clipboard_write_failed`
- `publish_failed`
- `poll_failed`
- `peer_sync_failed`

Clipboard content is never written to logs.

## Capabilities

```bash
copi version --json
```
