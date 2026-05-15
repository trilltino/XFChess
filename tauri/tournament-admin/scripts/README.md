# Admin Panel Launch Scripts

This directory contains scripts to easily launch the XFChess Tournament Admin panel.

## Usage

### Option 1: NPM Script (Recommended)
```bash
npm start
```

### Option 2: Direct Node Script
```bash
node scripts/launch-admin.js
```

### Option 3: Windows Batch Script
```bash
scripts\launch-admin.bat
```

## What the scripts do

- **launch-admin.js**: Cross-platform Node.js script that starts the dev server and handles graceful shutdown
- **launch-admin.bat**: Windows-specific batch script for quick launching

Both scripts will:
1. Change to the correct directory
2. Start the Vite development server
3. Open the admin panel at http://localhost:5173
4. Handle Ctrl+C gracefully to shut down the server

## Accessing the Admin Panel

Once launched, open your browser and navigate to:
- Local: http://localhost:5173/
- The admin panel will prompt for a backend URL and admin token
