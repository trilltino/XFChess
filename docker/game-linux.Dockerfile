# syntax=docker/dockerfile:1.7

# Build from the repo root with docker build -f docker/game-linux.Dockerfile .
# The binary is /app/xfchess inside the image.

FROM rust:1.96-slim-bookworm AS builder

WORKDIR /app

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libasound2-dev \
    libudev-dev \
    libx11-dev \
    libxi-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxinerama-dev \
    libwayland-dev \
    libxkbcommon-dev \
    libgl1-mesa-dev \
    libssl-dev \
    perl \
    make

COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY assets ./assets
COPY crates ./crates
COPY programs ./programs
COPY tauri ./tauri
COPY backend ./backend

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin xfchess \
    && cp target/release/xfchess /app/xfchess-out \
    && cp -r target/release/assets /app/assets-out

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    xvfb \
    libasound2 \
    libudev1 \
    libx11-6 \
    libxi6 \
    libxcursor1 \
    libxrandr2 \
    libxinerama1 \
    libwayland-client0 \
    libxkbcommon0 \
    libxkbcommon-x11-0 \
    mesa-vulkan-drivers \
    libgl1-mesa-dri

COPY --from=builder /app/xfchess-out /app/xfchess
COPY --from=builder /app/assets-out /app/assets

CMD ["/app/xfchess"]
