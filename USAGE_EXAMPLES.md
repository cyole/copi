# Copi Usage Examples

## Build

```bash
go build -o bin/copi ./cmd/copi
```

## Relay Mode

Run the third-party relay:

```bash
./bin/copi relay --addr 0.0.0.0:9527 --token my-secret
```

Docker:

```bash
printf "COPI_TOKEN=my-secret\n" > .env && docker compose up -d
```

Run clients on real devices:

```bash
./bin/copi client --relay http://192.168.1.10:9527 --token my-secret
```

## LAN Mode

Run this on every device in the LAN:

```bash
./bin/copi client --lan --token my-secret
```

If a machine advertises the wrong address:

```bash
./bin/copi client --lan \
  --listen 0.0.0.0:9528 \
  --advertise http://192.168.1.20:9528 \
  --token my-secret
```

## Diagnostics

```bash
./bin/copi doctor --json
./bin/copi doctor --mode client --relay http://192.168.1.10:9527 --json
./bin/copi doctor --mode lan --json
```

## JSON Logs

```bash
./bin/copi client --relay http://192.168.1.10:9527 --token my-secret --log-format json
./bin/copi client --lan --token my-secret --log-format json
./bin/copi relay --token my-secret --log-format json
```

## Manual API Check

```bash
curl http://127.0.0.1:9527/health
```
