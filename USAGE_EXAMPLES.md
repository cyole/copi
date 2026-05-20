# Copi Usage Examples

## Local Build

```bash
go build -o bin/copi ./cmd/copi
```

Check the CLI core from a native shell:

```bash
./bin/copi status --json
```

Create and inspect a config file:

```bash
./bin/copi config init
./bin/copi config show
./bin/copi config set client.server_url http://192.168.1.10:9527
./bin/copi config get client.server_url
```

Run diagnostics:

```bash
./bin/copi doctor --json
./bin/copi doctor --mode client --server http://192.168.1.10:9527 --json
```

Run with JSON logs for a native shell:

```bash
./bin/copi client --server http://192.168.1.10:9527 --token my-secret --log-format json
```

## Server Mode

A third-party machine runs the HTTP relay. This machine does not read or write its own clipboard:

```bash
./bin/copi server --addr 0.0.0.0:9527 --token my-secret
```

Docker one-command deployment:

```bash
printf "COPI_TOKEN=my-secret\n" > .env && docker compose up -d
```

A real user device connects as a client:

```bash
./bin/copi client --server http://192.168.1.10:9527 --token my-secret
```

Another user device connects the same way:

```bash
./bin/copi client --server http://192.168.1.10:9527 --token my-secret
```

Now text copied on one client is published to the third-party relay and applied on the other clients.

## LAN Mode

Run this on every device in the same LAN:

```bash
./bin/copi lan --token my-secret
```

If a machine has multiple network cards and advertises the wrong address:

```bash
./bin/copi lan \
  --listen 0.0.0.0:9528 \
  --advertise http://192.168.1.20:9528 \
  --token my-secret
```

## Health Check

```bash
curl http://127.0.0.1:9527/health
```

## Publish Clipboard Content Manually

```bash
curl -X POST http://127.0.0.1:9527/v1/clipboard \
  -H 'Authorization: Bearer my-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "id": "manual-1",
    "device_id": "manual",
    "device_name": "manual",
    "timestamp": "2026-05-20T00:00:00Z",
    "payload": {
      "type": "text",
      "mime": "text/plain; charset=utf-8",
      "text": "hello from copi"
    }
  }'
```

## Poll Latest Content

```bash
curl 'http://127.0.0.1:9527/v1/clipboard?since=0&wait=1s' \
  -H 'Authorization: Bearer my-secret'
```
