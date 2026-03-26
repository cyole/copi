# Stage 1: Build
FROM rust:1.88-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxcb1-dev \
    cmake \
    gcc \
    g++ \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libxcb-shape0 \
    libxcb-xfixes0 \
    libxcb1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/copi /usr/local/bin/copi

EXPOSE 9527

ENTRYPOINT ["copi"]
CMD ["server", "--relay-only", "--addr", "0.0.0.0:9527", "--tls-auto-cert"]
