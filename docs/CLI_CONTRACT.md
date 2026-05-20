# CLI Contract

Native clients should treat `copi` as the shared sync engine and communicate with it through stable CLI commands, config files, environment variables, and JSON log events.

## Configuration

The default config path is printed by:

```bash
copi config path
```

Create a starter config:

```bash
copi config init
```

Show the resolved config, including defaults:

```bash
copi config show
```

Use a custom config file:

```bash
copi client --config ./copi.json
```

Precedence is:

1. CLI flags
2. Environment variables
3. Config file
4. Built-in defaults

Example config:

```json
{
  "token": "replace-this",
  "log_format": "json",
  "device": {
    "name": "MacBook"
  },
  "relay": {
    "addr": "0.0.0.0:9527"
  },
  "client": {
    "server_url": "http://relay.example.com:9527",
    "interval": "500ms",
    "long_poll_wait": "30s"
  },
  "lan": {
    "listen_addr": "0.0.0.0:9528",
    "advertise_url": "",
    "multicast_addr": "239.255.27.42:9529",
    "interval": "500ms"
  }
}
```

## JSON Logs

Long-running commands support:

```bash
copi client --log-format json
copi lan --log-format json
copi relay --log-format json
```

JSON logs are newline-delimited. Each line is one event with at least:

```json
{
  "time": "2026-05-20T00:00:00Z",
  "level": "info",
  "type": "started",
  "message": "client sync started"
}
```

Important event types:

- `started`: sync loop has started
- `relay_starting`: relay command is starting
- `relay_started`: relay HTTP listener is ready
- `client_starting`: client command is starting
- `lan_starting`: LAN command is starting
- `lan_peer_listening`: LAN peer HTTP listener is ready
- `peer_discovered`: LAN peer discovered
- `clipboard_published`: local clipboard was published
- `clipboard_applied`: remote clipboard was applied locally
- `clipboard_broadcast`: LAN clipboard broadcast attempted
- `clipboard_read_failed`: local clipboard read failed
- `clipboard_write_failed`: local clipboard write failed
- `publish_failed`: client failed to publish to relay
- `poll_failed`: client failed to poll relay
- `peer_sync_failed`: LAN peer sync failed

Clipboard content is never written to logs. Events include metadata such as byte counts, sequence numbers, device IDs, and peer URLs.

## Capability Detection

Native shells should call:

```bash
copi status --json
```

This reports CLI version, supported commands, modes, and capabilities.
