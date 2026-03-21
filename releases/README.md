# XFChess - Iroh Networking Version

## Overview
XFChess with Iroh P2P networking for decentralized multiplayer chess. This version does not include Solana wallet functionality - it's purely focused on peer-to-peer chess gaming.

## Features
- **3D Chess Game**: Full 3D chess board and pieces with Bevy engine
- **Iroh P2P Networking**: Decentralized peer-to-peer multiplayer
- **Stockfish AI**: Built-in chess AI for single-player
- **Session Management**: JSON-based session configuration
- **Transaction Debugging**: Built-in debugging tools for network transactions
- **Cross-Platform Support**: Native executable + Docker containers

## Installation Options

### Option 1: Native Executable (Recommended)
Download `XFChess-Iroh.exe` and run directly on Windows.

### Option 2: Docker (Universal)
Run on any system with Docker installed - perfect for Linux, macOS, or containerized deployments.

## Docker Setup

### Prerequisites
- Docker Desktop (Windows/macOS) or Docker Engine (Linux)
- Docker Buildx (for multi-platform builds)

### Quick Start with Docker
```bash
# Clone and build
git clone https://github.com/trilltino/XFChess.git
cd XFChess/releases
docker build -t xfchess-iroh:local .

# Run the interactive launcher
./docker-run.sh    # Linux/macOS
docker-run.bat     # Windows
```

### Manual Docker Commands
```bash
# Single Player
docker run -it --rm -p 5001:5001 xfchess-iroh:local play --player-color white

# Multiplayer Host
docker run -it --rm -p 5001:5001 xfchess-iroh:local play --player-color white --p2p-port 5001

# Multiplayer Join
docker run -it --rm -p 5001:5001 xfchess-iroh:local play --player-color black --bootstrap-node <NODE_ID>
```

### Docker Compose
```bash
cd releases
docker-compose up
```

### Building Multi-Platform Images
```bash
cd releases
chmod +x build-docker.sh
./build-docker.sh
```

## Quick Start

### Single Player
**Native:**
```bash
XFChess-Iroh.exe play --player-color white
```

**Docker:**
```bash
docker run -it --rm -p 5001:5001 xfchess-iroh:local play --player-color white
```

### Multiplayer (Player 1 - Host)
**Native:**
```bash
XFChess-Iroh.exe play --player-color white --p2p-port 5001
```

**Docker:**
```bash
docker run -it --rm -p 5001:5001 xfchess-iroh:local play --player-color white --p2p-port 5001
```
This will start a game and display your node ID. Share this node ID with Player 2.

### Multiplayer (Player 2 - Join)
**Native:**
```bash
XFChess-Iroh.exe play --player-color black --bootstrap-node <PLAYER_1_NODE_ID>
```

**Docker:**
```bash
docker run -it --rm -p 5001:5001 xfchess-iroh:local play --player-color black --bootstrap-node <PLAYER_1_NODE_ID>
```
Replace `<PLAYER_1_NODE_ID>` with the node ID provided by Player 1.

## Command Line Options

### Basic Options
- `--player-color <COLOR>`: Choose your color (white/black)
- `--p2p-port <PORT>`: P2P network port (default: 5001)
- `--bootstrap-node <NODE_ID>`: Connect to another player's node
- `--game-id <ID>`: Specific game ID
- `--debug`: Enable transaction debugging

### Session Configuration
- `--session-config <FILE>`: Load game from JSON session file
- `--session-key <KEY>`: Session signing key (base58)
- `--session-pubkey <PUBKEY>`: Session public key

### Debug Options
- `--log-file <FILE>`: Debug log file (default: rollup_debug.log)
- `--no-pretty-print`: Disable pretty printing in debug output

## Session Files

XFChess can save and load game sessions as JSON files:

```json
{
  "game_id": "12345",
  "player_color": "white",
  "p2p_port": 5001,
  "session_key": "base58_encoded_key",
  "session_pubkey": "base58_encoded_pubkey"
}
```

### Creating a Session
Start a game and it will automatically generate a session file in the current directory (or `sessions/` folder for Docker).

### Loading a Session
**Native:**
```bash
XFChess-Iroh.exe --session-config session.json
```

**Docker:**
```bash
docker run -it --rm -v "$(pwd)/sessions:/home/xfchess/sessions" xfchess-iroh:local --session-config /home/xfchess/sessions/session.json
```

## Assets
The `assets/` directory contains:
- Chess 3D models and textures
- Board themes
- Sound effects
- Stockfish engine executable

## Requirements

### Native Version
- Windows 10/11
- Graphics card with OpenGL 3.3+ support
- Network connection for multiplayer

### Docker Version
- Docker Desktop or Docker Engine
- Any supported platform (Windows, macOS, Linux)
- Network connection for multiplayer

## Network Requirements
- Port 5001 (or custom port) must be open for hosting games
- No central servers required - pure P2P networking
- Docker automatically handles port forwarding

## Troubleshooting

### Native Issues
- Check firewall settings for the P2P port
- Ensure graphics drivers are up to date
- Try reducing graphics settings if available

### Docker Issues
- Ensure Docker is running and accessible
- Check if port 5001 is already in use: `netstat -an | grep 5001`
- For graphics issues on Linux, ensure X11 forwarding is set up

### Connection Issues
- Check firewall settings for the P2P port
- Ensure both players have network connectivity
- Verify the node ID is copied correctly

### Performance Issues
- Close other applications to improve performance
- Ensure graphics drivers are up to date
- For Docker, allocate more resources if needed

### Debug Mode
Enable debug mode to see detailed network transactions:

**Native:**
```bash
XFChess-Iroh.exe --debug play --player-color white
```

**Docker:**
```bash
docker run -it --rm -v "$(pwd)/logs:/home/xfchess/logs" xfchess-iroh:local --debug play --player-color white
```

## Platform Support

| Platform | Native | Docker | Notes |
|----------|---------|---------|-------|
| Windows 10/11 | ✅ | ✅ | Both versions fully supported |
| macOS | ❌ | ✅ | Docker recommended for macOS |
| Linux | ❌ | ✅ | Docker recommended for Linux |
| ARM64 | ❌ | ✅ | Docker supports ARM64 via buildx |

## License
MIT/Apache-2.0 - See repository for full license details.

## Support
For issues and support:
- GitHub Issues: https://github.com/trilltino/XFChess/issues
- Documentation: Check the main repository README
