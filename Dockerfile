# syntax=docker/dockerfile:1

FROM golang:1.22-alpine AS build

WORKDIR /src
COPY go.mod ./
RUN go mod download

COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -trimpath -ldflags="-s -w" -o /out/copi ./cmd/copi

FROM alpine:3.20

RUN adduser -D -H -u 10001 copi
COPY --from=build /out/copi /usr/local/bin/copi

USER copi
EXPOSE 9527

HEALTHCHECK --interval=30s --timeout=3s --retries=3 CMD wget -qO- http://127.0.0.1:9527/health >/dev/null || exit 1

ENTRYPOINT ["copi"]
CMD ["server"]
