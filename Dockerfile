FROM --platform=$TARGETPLATFORM rust:1.86-alpine AS builder

RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    pkgconf \
    gcc \
    make \
    binutils

WORKDIR /usr/src/blazebee
COPY . .

ARG TARGETPLATFORM
ARG CARGO_FEATURES=standard

RUN case "$TARGETPLATFORM" in \
    "linux/amd64")  TARGET="x86_64-unknown-linux-musl" ;; \
    "linux/arm64")  TARGET="aarch64-unknown-linux-musl" ;; \
    *) exit 1 ;; \
    esac \
    && rustup target add "$TARGET" \
    && cargo build --release --target "$TARGET" --features "$CARGO_FEATURES" \
    && strip "target/$TARGET/release/blazebee" \
    && mv "target/$TARGET/release/blazebee" /blazebee

FROM scratch
COPY --from=builder /blazebee /blazebee
COPY --from=builder /etc/ssl/cert.pem /etc/ssl/cert.pem
ENTRYPOINT ["/blazebee"]
