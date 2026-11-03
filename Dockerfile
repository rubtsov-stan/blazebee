FROM --platform=$TARGETPLATFORM rust:1.86-alpine AS builder

# gcc/make: C sources of zstd-sys and ring (rustls) are compiled from source.
RUN apk add --no-cache musl-dev gcc make

WORKDIR /usr/src/blazebee
COPY . .

ARG TARGETPLATFORM
# One of: minimal, standard, large (or an explicit list of collector-* features).
ARG CARGO_FEATURES=standard

RUN case "$TARGETPLATFORM" in \
        "linux/amd64") TARGET="x86_64-unknown-linux-musl" ;; \
        "linux/arm64") TARGET="aarch64-unknown-linux-musl" ;; \
        *) echo "unsupported platform: $TARGETPLATFORM" >&2; exit 1 ;; \
    esac \
    && rustup target add "$TARGET" \
    && cargo build --release --locked --target "$TARGET" \
        --no-default-features --features "$CARGO_FEATURES" \
    && mv "target/$TARGET/release/blazebee" /blazebee

FROM scratch
COPY --from=builder /blazebee /blazebee
COPY --from=builder /etc/ssl/cert.pem /etc/ssl/cert.pem
# Used by the system trust store when [transport.tls] has no ca_cert_path.
ENV SSL_CERT_FILE=/etc/ssl/cert.pem
ENTRYPOINT ["/blazebee"]
