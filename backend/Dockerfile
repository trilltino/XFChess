# XFChess Backend Dockerfile
# Multi-stage build for optimized image

FROM rust:1.75-slim as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY backend ./backend
COPY crates ./crates
COPY programs ./programs
COPY xfchessdotcom ./xfchessdotcom
COPY tauri ./tauri

RUN cargo build --release --bin signing-server

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/signing-server /app/signing-server

RUN useradd -r -u 1000 xfchess
RUN chown -R xfchess:xfchess /app
USER xfchess

# Expose port
EXPOSE 8090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8090/health || exit 1

# Run the application
CMD ["/app/signing-server"]
