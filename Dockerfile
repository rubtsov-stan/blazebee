FROM --platform=$BUILDPLATFORM rust:1.86-alpine AS builder

RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    pkgconf \
    gcc \
    make \
    cmake \
    binutils \
    curl \
    unzip

RUN curl -L https://ziglang.org/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz -o zig.tar.xz \
    && tar -xJf zig.tar.xz -C /usr/local \
    && mv /usr/local/zig-linux-x86_64-0.13.0 /usr/local/zig \
    && ln -s /usr/local/zig/zig /usr/local/bin/zig \
    && rm zig.tar.xz

RUN cargo install cargo-zigbuild

WORKDIR /usr/src/blazebee

COPY . .

ARG TARGETPLATFORM
ARG CARGO_FEATURES=standard

RUN case "$TARGETPLATFORM" in \
    "linux/amd64")  TARGET="x86_64-unknown-linux-musl" ;; \
    "linux/arm64")  TARGET="aarch64-unknown-linux-musl" ;; \
    *) echo "Unsupported platform: $TARGETPLATFORM" && exit 1 ;; \
    esac \
    && rustup target add "$TARGET" \
    && cargo zigbuild --release --target "$TARGET" --features "${CARGO_FEATURES}" \
    && zig objcopy --strip-all "target/$TARGET/release/blazebee" "target/$TARGET/release/blazebee.stripped" \
    && mv "target/$TARGET/release/blazebee.stripped" /blazebee

FROM scratch

COPY --from=builder /blazebee /blazebee
COPY --from=builder /etc/ssl/cert.pem /etc/ssl/cert.pem

ENTRYPOINT ["/blazebee"]
CMD ["--config", "/etc/blazebee/config.toml"]