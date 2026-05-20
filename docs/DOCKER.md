# Docker Deployment

The Docker image is for the third-party HTTP relay used by server mode. It does not read or write the host clipboard.

## One-Command Compose Deployment

From the repository root:

```bash
printf "COPI_TOKEN=%s\n" "$(openssl rand -hex 16)" > .env && docker compose up -d
```

The relay will listen on port `9527`.

Check it:

```bash
curl http://127.0.0.1:9527/health
```

Then point clients at:

```bash
copi client --server http://YOUR_SERVER_IP:9527 --token "$(grep '^COPI_TOKEN=' .env | cut -d= -f2-)"
```

## Compose With A Fixed Token

Create a `.env` file:

```env
COPI_PORT=9527
COPI_TOKEN=replace-this-with-a-long-random-token
```

Start or update:

```bash
docker compose up -d --build
```

Stop:

```bash
docker compose down
```

## Plain Docker

Build:

```bash
docker build -t copi:local .
```

Run:

```bash
docker run -d \
  --name copi-relay \
  --restart unless-stopped \
  -p 9527:9527 \
  -e COPI_TOKEN=replace-this-with-a-long-random-token \
  copi:local
```

## Environment Variables

- `COPI_ADDR`: relay listen address inside the container. Default: `0.0.0.0:9527`.
- `COPI_TOKEN`: shared client token. Set this in production.
- `COPI_PORT`: host port used by `compose.yaml`. Default: `9527`.
