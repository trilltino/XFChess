# Stage 1: Builder
# Uses the same Rust version as local development
FROM rust:1.83-slim-bookworm AS builder
WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    binaryen \
    && rm -rf /var/lib/apt/lists/*

# Install wasm-bindgen-cli handled by Trunk, but we install Trunk via cargo-binstall for speed
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall trunk -y

# Add WASM target
RUN rustup target add wasm32-unknown-unknown

# Copy manifests first for caching (optional optimization, skipping for simplicity in this specific setup due to workspace complexity)
COPY . .

# Build the Web App
# This produces the ./dist directory containing index.html, JS, and WASM
WORKDIR /app
RUN trunk build --release web/index.html --public-url /

# Stage 2: Runtime
# Uses a lightweight Nginx image to serve the static files
FROM nginx:alpine AS runtime

# Copy the build artifacts from the builder stage
COPY --from=builder /app/dist /usr/share/nginx/html

# Copy custom Nginx config if needed (using default for now, but configured for SPA fallback)
# Create a simple nginx config for SPA (Single Page App) support with gzip and caching
RUN echo 'server { \
    listen 8080; \
    server_name localhost; \
    gzip on; \
    gzip_types application/wasm application/javascript text/html text/css; \
    gzip_min_length 1000; \
    location / { \
    root /usr/share/nginx/html; \
    index index.html index.htm; \
    try_files $uri $uri/ /index.html; \
    add_header Cache-Control "public, max-age=31536000, immutable" always; \
    } \
    location /index.html { \
    root /usr/share/nginx/html; \
    add_header Cache-Control "no-cache" always; \
    } \
    }' > /etc/nginx/conf.d/default.conf

# Configure Nginx to listen on the port expected by Fly.io (8080)
EXPOSE 8080

# Start Nginx
CMD ["nginx", "-g", "daemon off;"]
