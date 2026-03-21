#!/bin/bash

# XFChess Docker Build Script
# Builds cross-platform Docker images for XFChess Iroh

set -e

echo "🏗️  Building XFChess Docker image..."

# Build arguments
IMAGE_NAME="xfchess-iroh"
VERSION="0.1.0"
PLATFORMS="linux/amd64,linux/arm64"

# Check if buildx is available
if ! docker buildx version >/dev/null 2>&1; then
    echo "❌ Docker buildx not found. Please install Docker Buildx."
    exit 1
fi

# Create and use buildx builder
echo "🔧 Setting up buildx builder..."
docker buildx create --use --name xfchess-builder 2>/dev/null || true

# Build for multiple platforms
echo "🚀 Building for platforms: $PLATFORMS"
docker buildx build \
    --platform $PLATFORMS \
    --tag $IMAGE_NAME:$VERSION \
    --tag $IMAGE_NAME:latest \
    --push \
    ..

# Also build local version
echo "🏠 Building local version..."
docker buildx build \
    --platform linux/amd64 \
    --tag $IMAGE_NAME:local \
    --load \
    ..

echo "✅ Docker build completed!"
echo ""
echo "📦 Available images:"
echo "  $IMAGE_NAME:$VERSION (multi-arch, pushed to registry)"
echo "  $IMAGE_NAME:latest (multi-arch, pushed to registry)"  
echo "  $IMAGE_NAME:local (amd64 only, local)"
echo ""
echo "🐳 To run locally:"
echo "  docker run -it --rm -p 5001:5001 $IMAGE_NAME:local"
echo ""
echo "🔗 To use with docker-compose:"
echo "  cd releases && docker-compose up"
