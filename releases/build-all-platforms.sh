#!/bin/bash

# XFChess Cross-Platform Build Script
# Builds for Windows, Linux, and macOS

set -e

echo "🏗️  XFChess Cross-Platform Build Script"
echo "======================================"

# Configuration
VERSION="0.1.0"
PROJECT_ROOT="$(cd .. && pwd)"
RELEASE_DIR="$PWD"

# Create release directories
mkdir -p "$RELEASE_DIR/dist/windows"
mkdir -p "$RELEASE_DIR/dist/linux"
mkdir -p "$RELEASE_DIR/dist/macos"

echo "📋 Build Configuration:"
echo "   Version: $VERSION"
echo "   Project Root: $PROJECT_ROOT"
echo "   Release Dir: $RELEASE_DIR"
echo ""

# Function to build for a specific target
build_target() {
    local target=$1
    local output_dir=$2
    local exe_name=$3
    
    echo "🔨 Building for $target..."
    
    cd "$PROJECT_ROOT"
    
    # Build for the target
    if command -v cross >/dev/null 2>&1; then
        cross build --release --target "$target" --bin xfchess
    else
        rustup target add "$target"
        cargo build --release --target "$target" --bin xfchess
    fi
    
    # Copy executable and assets
    local exe_path="target/$target/release/xfchess"
    if [[ "$target" == *"windows"* ]]; then
        exe_path="target/$target/release/xfchess.exe"
    fi
    
    if [[ "$target" == *"windows"* ]]; then
        cp "$exe_path" "$output_dir/XFChess-Iroh.exe"
    else
        cp "$exe_path" "$output_dir/xfchess"
        chmod +x "$output_dir/xfchess"
    fi
    
    # Copy assets
    cp -r "$PROJECT_ROOT/assets" "$output_dir/"
    
    # Copy documentation
    cp "$RELEASE_DIR/README.md" "$output_dir/"
    
    # Create platform-specific launcher
    if [[ "$target" == *"windows"* ]]; then
        cp "$RELEASE_DIR/Start-XFChess.bat" "$output_dir/"
    else
        cat > "$output_dir/start-xfchess.sh" << 'EOF'
#!/bin/bash
echo "🎮 XFChess - Iroh Networking Version"
echo ""
echo "Choose game mode:"
echo "1. Single Player (vs AI)"
echo "2. Multiplayer - Host Game"
echo "3. Multiplayer - Join Game"
echo "4. Load Session File"
echo ""
read -p "Enter your choice (1-4): " choice

case $choice in
    1)
        echo "🤖 Starting single player game..."
        ./xfchess play --player-color white
        ;;
    2)
        echo "🏠 Starting multiplayer host..."
        echo "📝 Share your node ID with your opponent when it appears."
        ./xfchess play --player-color white --p2p-port 5001
        ;;
    3)
        read -p "🔗 Enter host's node ID: " nodeid
        echo "🔌 Connecting to host..."
        ./xfchess play --player-color black --bootstrap-node "$nodeid"
        ;;
    4)
        read -p "📁 Enter session file path: " sessionfile
        echo "📂 Loading session..."
        ./xfchess --session-config "$sessionfile"
        ;;
    *)
        echo "❌ Invalid choice. Please run again."
        exit 1
        ;;
esac
EOF
        chmod +x "$output_dir/start-xfchess.sh"
    fi
    
    echo "✅ Build completed for $target"
}

# Build for each platform
echo "🚀 Starting builds..."

# Windows (x64)
build_target "x86_64-pc-windows-gnu" "$RELEASE_DIR/dist/windows" "XFChess-Iroh.exe"

# Linux (x64)
build_target "x86_64-unknown-linux-gnu" "$RELEASE_DIR/dist/linux" "xfchess"

# macOS (x64)
build_target "x86_64-apple-darwin" "$RELEASE_DIR/dist/macos" "xfchess"

# macOS (ARM64) - if supported
if rustup target list --installed | grep -q "aarch64-apple-darwin"; then
    build_target "aarch64-apple-darwin" "$RELEASE_DIR/dist/macos-arm64" "xfchess"
fi

echo ""
echo "📦 Creating distribution archives..."

# Create archives
cd "$RELEASE_DIR/dist"

# Windows
cd windows
zip -r "../XFChess-Iroh-Windows-x64-$VERSION.zip" .
cd ..

# Linux
cd linux
tar -czf "../XFChess-Iroh-Linux-x64-$VERSION.tar.gz" .
cd ..

# macOS
cd macos
tar -czf "../XFChess-Iroh-macOS-x64-$VERSION.tar.gz" .
cd ..

# macOS ARM64 (if exists)
if [ -d "macos-arm64" ]; then
    cd macos-arm64
    tar -czf "../XFChess-Iroh-macOS-ARM64-$VERSION.tar.gz" .
    cd ..
fi

echo ""
echo "✅ Build completed successfully!"
echo ""
echo "📦 Distribution files created:"
ls -la *.zip *.tar.gz 2>/dev/null || echo "No archives found"
echo ""
echo "🚀 Ready for GitHub release deployment!"
