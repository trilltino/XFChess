#!/bin/bash

# XFChess Docker Run Script
# Easy launcher for XFChess in Docker

set -e

IMAGE_NAME="xfchess-iroh:local"
CONTAINER_NAME="xfchess-iroh"

# Create necessary directories
mkdir -p sessions logs

echo "🎮 XFChess Docker Launcher"
echo "=========================="
echo ""
echo "Choose game mode:"
echo "1. Single Player (vs AI)"
echo "2. Multiplayer - Host Game"  
echo "3. Multiplayer - Join Game"
echo "4. Load Session File"
echo "5. Debug Mode"
echo ""

read -p "Enter your choice (1-5): " choice

case $choice in
    1)
        echo "🤖 Starting single player game..."
        docker run -it --rm \
            --name $CONTAINER_NAME \
            -v "$(pwd)/sessions:/home/xfchess/sessions" \
            -v "$(pwd)/logs:/home/xfchess/logs" \
            -p 5001:5001 \
            $IMAGE_NAME \
            play --player-color white
        ;;
    2)
        echo "🏠 Starting multiplayer host..."
        echo "📝 Share your node ID with your opponent when it appears."
        docker run -it --rm \
            --name $CONTAINER_NAME \
            -v "$(pwd)/sessions:/home/xfchess/sessions" \
            -v "$(pwd)/logs:/home/xfchess/logs" \
            -p 5001:5001 \
            $IMAGE_NAME \
            play --player-color white --p2p-port 5001
        ;;
    3)
        read -p "🔗 Enter host's node ID: " nodeid
        echo "🔌 Connecting to host..."
        docker run -it --rm \
            --name $CONTAINER_NAME \
            -v "$(pwd)/sessions:/home/xfchess/sessions" \
            -v "$(pwd)/logs:/home/xfchess/logs" \
            -p 5001:5001 \
            $IMAGE_NAME \
            play --player-color black --bootstrap-node "$nodeid"
        ;;
    4)
        read -p "📁 Enter session file path (relative to sessions/): " sessionfile
        echo "📂 Loading session..."
        docker run -it --rm \
            --name $CONTAINER_NAME \
            -v "$(pwd)/sessions:/home/xfchess/sessions" \
            -v "$(pwd)/logs:/home/xfchess/logs" \
            -p 5001:5001 \
            $IMAGE_NAME \
            --session-config "/home/xfchess/sessions/$sessionfile"
        ;;
    5)
        echo "🐛 Starting debug mode..."
        docker run -it --rm \
            --name $CONTAINER_NAME \
            -v "$(pwd)/sessions:/home/xfchess/sessions" \
            -v "$(pwd)/logs:/home/xfchess/logs" \
            -p 5001:5001 \
            $IMAGE_NAME \
            debug
        ;;
    *)
        echo "❌ Invalid choice. Please run again."
        exit 1
        ;;
esac

echo ""
echo "🎮 Game session ended!"
echo "📁 Session files saved in: $(pwd)/sessions/"
echo "📋 Logs available in: $(pwd)/logs/"
