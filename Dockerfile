# ---------------------------------------------------
# Stage 1: Chef Base (use -dev variant which includes apk)
# ---------------------------------------------------
FROM dhi.io/rust:1-alpine-dev AS chef

# SPEED UP 1: Added `lld` and `clang` to leverage a much faster linker
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static ca-certificates lld clang curl

# Create a non-root user/group in the build environment
RUN addgroup -S appgroup && adduser -S appuser -G appgroup

# SPEED UP 2: Force Cargo to use the sparse registry protocol (faster dependency fetching over the network)
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# Make cargo more resilient to slow networks during Docker builds
ENV CARGO_HTTP_TIMEOUT=120
ENV CARGO_NET_RETRY=5
ENV CARGO_HTTP_LOW_SPEED_LIMIT=5
ENV CARGO_HTTP_MULTIPLEXING=false

# SPEED UP 3: Configure cargo to use the faster `lld` linker for musl targets
RUN mkdir -p /app/.cargo && \
    echo '[target.x86_64-unknown-linux-musl]' > /app/.cargo/config.toml && \
    echo 'rustflags = ["-C", "link-arg=-fuse-ld=lld"]' >> /app/.cargo/config.toml

# Install cargo-binstall to pull pre-compiled cargo-chef binary (saves 4+ minutes of compiling cargo-chef)
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | sh && \
    cargo binstall -y cargo-chef

WORKDIR /app

# ---------------------------------------------------
# Stage 2: Planner
# ---------------------------------------------------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---------------------------------------------------
# Stage 3: Builder
# ---------------------------------------------------
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - utilizing cache mounts for registry, git, and target directory
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

# Copy actual code and build
COPY . .
ENV SQLX_OFFLINE=true
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin rustom && \
    cp /app/target/release/rustom /app/rustom-bin

# ---------------------------------------------------
# Stage 4: Runtime (hardened image, no package manager)
# ---------------------------------------------------
FROM dhi.io/alpine-base:3.24 AS runtime

# Metadata labels
LABEL maintainer="Hasan Mohammad Tanbir <tanbir2043@gmail.com>"
LABEL org.opencontainers.image.source="https://github.com/hmtanbir/rustom"
LABEL org.opencontainers.image.description="rustom API Service"

WORKDIR /app

# Copy user/group definitions and SSL certificates from the build stage
COPY --from=chef /etc/passwd /etc/passwd
COPY --from=chef /etc/group /etc/group
COPY --from=chef /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

# Copy required dynamic libraries from the chef stage
COPY --from=chef /usr/lib/libssl.so.3 /usr/lib/libssl.so.3
COPY --from=chef /usr/lib/libcrypto.so.3 /usr/lib/libcrypto.so.3
COPY --from=chef /usr/lib/libgcc_s.so.1 /usr/lib/libgcc_s.so.1

# Copy build artifact and set ownership
COPY --from=builder --chown=appuser:appgroup /app/rustom-bin /app/rustom

# Tell Docker to run the container as the non-root user
USER appuser

EXPOSE 3000
ENV TZ=Etc/UTC

# Optional: Add a healthcheck (assuming you have a /health route)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://127.0.0.1:3000/health || exit 1

CMD ["/app/rustom"]
