# XFChess Iroh - Multi-stage Docker build for cross-platform compatibility
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libx11-dev \
    libgl1-mesa-dev \
    libvulkan-dev \
    libvulkan-tools \
    vulkan-tools \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY crates ./crates
COPY assets ./assets

# Build the release version
RUN cargo build --release --bin xfchess

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libx11-6 \
    libgl1-mesa-glx \
    libvulkan1 \
    vulkan-tools \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 xfchess

# Set working directory
WORKDIR /home/xfchess

# Copy the built executable
COPY --from=builder /app/target/release/xfchess /usr/local/bin/xfchess

# Copy assets
COPY --from=builder /app/assets /home/xfchess/assets

# Change ownership
RUN chown -R xfchess:xfchess /home/xfchess

# Switch to non-root user
USER xfchess

# Expose P2P port
EXPOSE 5001

# Add helpful labels
LABEL maintainer="trilltino"
LABEL description="XFChess with Iroh P2P networking"
LABEL version="0.1.0"

# Default command
CMD ["xfchess", "--help"]
