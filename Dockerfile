FROM rust:1.86-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    build-essential \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /blazebee

COPY . .

ARG CARGO_FEATURES=default
ARG TARGETARCH=amd64
ARG CONFIG_FILE=config.example.toml

RUN cargo build --release --features "${CARGO_FEATURES}" \
    && mv target/release/blazebee /blazebee/blazebee

FROM debian:12-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /blazebee/blazebee /usr/local/bin/blazebee

ARG CONFIG_FILE=config.example.toml
COPY ${CONFIG_FILE} /etc/blazebee/config.toml

RUN mkdir -p /etc/blazebee

RUN chmod +x /usr/local/bin/blazebee

ENTRYPOINT ["/usr/local/bin/blazebee"]