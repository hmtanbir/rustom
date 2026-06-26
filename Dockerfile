# ---------------------------------------------------
# Stage 1: Chef Base (use -dev variant which includes apk)
# ---------------------------------------------------
FROM dhi.io/rust:1-alpine-dev AS chef
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static ca-certificates
# Create a non-root user/group in the build environment
RUN addgroup -S appgroup && adduser -S appuser -G appgroup
# Make cargo more resilient to slow networks during Docker builds
ENV CARGO_HTTP_TIMEOUT=120
ENV CARGO_NET_RETRY=5
ENV CARGO_HTTP_LOW_SPEED_LIMIT=5
# Install cargo-chef by downloading the precompiled binary
ARG TARGETPLATFORM
RUN mkdir -p /usr/local/bin && \
    case "${TARGETPLATFORM}" in \
      "linux/amd64") ARCH="x86_64-unknown-linux-musl" ;; \
      "linux/arm64") ARCH="aarch64-unknown-linux-musl" ;; \
      *) ARCH="x86_64-unknown-linux-musl" ;; \
    esac && \
    wget -qO- "https://github.com/LukeMathWalker/cargo-chef/releases/download/v0.1.68/cargo-chef-${ARCH}.tar.gz" | tar xz -C /usr/local/bin
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

# Build dependencies - utilizing cache mounts for registry and git
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json

# Copy actual code and build
COPY . .
ENV SQLX_OFFLINE=true
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --bin rustom

# ---------------------------------------------------
# Stage 4: Runtime (hardened image, no package manager)
# ---------------------------------------------------
FROM dhi.io/alpine-base:3.24 AS runtime

# Metadata labels
LABEL maintainer="Hasan Mohammad Tanbir <tanbir2043@gmail.com>"
LABEL org.opencontainers.image.source="https://github.com/hmtanbir/rustom"
LABEL org.opencontainers.image.description="Rustom API Boilerplate"

WORKDIR /app

# Copy user/group definitions and SSL certificates from the build stage
COPY --from=chef /etc/passwd /etc/passwd
COPY --from=chef /etc/group /etc/group
COPY --from=chef /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

# Copy build artifact and set ownership
COPY --from=builder --chown=appuser:appgroup /app/target/release/rustom /app/rustom

# Tell Docker to run the container as the non-root user
USER appuser

EXPOSE 3000
ENV TZ=Etc/UTC

# Optional: Add a healthcheck (assuming you have a /health route)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://127.0.0.1:3000/health || exit 1

CMD ["/app/rustom"]

