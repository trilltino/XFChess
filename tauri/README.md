# XFChess Tauri Application

A modern, cross-platform desktop application for XFChess built with Tauri. This application serves as a wrapper for the XFChess chess platform, providing wallet adapter functionality and native game launcher capabilities.

## Overview

XFChess Tauri is a polyglot desktop application that combines:
- **Wallet Integration**: Seamless integration with Phantom, Solflare, and other Solana wallet extensions
- **Tournament Administration**: Native window for managing chess tournaments
- **Game Launcher**: Native integration for launching chess games
- **Multi-Window Architecture**: Separate windows for main app, wallet popup, and tournament admin

## Features

### Core Features
- Cross-platform support (Windows, macOS, Linux)
- Native window management with multiple specialized windows
- Secure wallet integration via browser extensions
- Tournament administration interface
- Real-time IPC communication between windows

### Technical Features
- **Rust Backend**: High-performance, memory-safe backend
- **Web Frontend**: Modern web technologies for UI
- **IPC Communication**: Efficient inter-process communication
- **Deep Link Support**: Custom URL scheme `xfchess://`
- **Auto-Updates**: Built-in update mechanism
- **Security**: Content Security Policy and sandboxed webviews

## Architecture

### Module Structure
```
src/
├── main.rs              # Application entry point
├── services/            # Business logic and services
│   ├── mod.rs
│   ├── auth.rs         # Authentication state management
│   ├── config.rs       # Configuration and environment
│   └── ipc.rs         # IPC command handlers
├── types/              # Type definitions
│   ├── mod.rs
│   ├── auth.rs         # Authentication types
│   ├── config.rs       # Configuration types
│   └── ipc.rs         # IPC message types
├── utils/              # Utility functions
│   ├── mod.rs
│   ├── crypto.rs       # Cryptographic utilities
│   ├── logging.rs      # Logging configuration
│   └── validation.rs  # Input validation
└── windows/            # Window management
    ├── mod.rs
    ├── tournament_admin.rs
    ├── wallet.rs
    └── popup.rs
```

### Window Architecture
- **Main Window**: Primary application interface (400x660)
- **Wallet Popup**: Browser-based wallet integration (420x500)
- **Tournament Admin**: Tournament management interface (1200x800)

## Development

### Prerequisites
- Rust 1.77.2 or higher
- Node.js and npm/pnpm
- Platform-specific build tools

### Setup
1. Clone the repository
2. Install Rust dependencies: `cargo build`
3. Install frontend dependencies:
   ```bash
   cd wallet-ui && npm install
   cd ../tournament-admin && npm install
   cd ../wallet-popup && npm install
   ```

### Development Commands
```bash
# Start development mode
cargo tauri dev

# Start with specific features
cargo tauri dev --features all
cargo tauri dev --features wallet,tournament-admin

# Build for release
cargo tauri build

# Format code
cargo fmt

# Check for issues
cargo clippy

# Run tests
cargo test
```

### Feature Flags
- `dev`: Development-specific features
- `wallet`: Wallet integration functionality
- `tournament-admin`: Tournament administration interface
- `all`: Enable all features

## Configuration

### Environment Variables
- `XFCHESS_WALLET_PORT`: Port for wallet popup (default: 7454)
- `BACKEND_URL`: Backend API URL (default: http://127.0.0.1:8090)
- `SIGNING_SERVICE_URL`: Alternative backend URL
- `ADMIN_API_KEY`: API key for admin operations
- `RUST_LOG`: Logging level (default: info)
- `NODE_ENV`: Environment mode (development/production)

### Configuration Files
- `tauri.conf.json`: Main Tauri configuration
- `rustfmt.toml`: Code formatting rules
- `.editorconfig`: Editor configuration

## Security

### Content Security Policy
The application uses a strict Content Security Policy to prevent XSS attacks:
- Default source: `'self'` only
- Connect sources: Local backend and API endpoints
- Script sources: `'self'` and inline scripts (required for frameworks)

### Permissions
The application requests minimal permissions:
- Window management (show, hide, resize, etc.)
- File system access (app config and data directories)
- Clipboard access
- URL opening
- Global shortcuts

## Building

### Release Build
```bash
# Optimized release build
cargo tauri build

# With specific features
cargo tauri build --features all
```

### Distribution
Built artifacts are placed in `src-tauri/target/release/bundle/` with platform-specific installers.

## Testing

### Unit Tests
```bash
# Run all tests
cargo test

# Run specific module tests
cargo test services::config
cargo test utils::crypto
```

### Integration Tests
```bash
# Run integration tests
cargo test --test integration
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests and linting: `cargo test && cargo clippy`
5. Submit a pull request

### Code Style
- Use `cargo fmt` for formatting
- Follow Rust naming conventions
- Add documentation for public APIs
- Write tests for new functionality

## Troubleshooting

### Common Issues
- **Build fails**: Check Rust version and dependencies
- **Window not showing**: Verify feature flags are enabled
- **Wallet not connecting**: Check browser extension is installed
- **IPC commands failing**: Ensure windows are properly initialized

### Debug Mode
Enable debug mode for detailed logging:
```bash
RUST_LOG=debug cargo tauri dev
```

## License

This project is licensed under MIT OR Apache-2.0. See LICENSE files for details.

## Support

For issues and support:
- GitHub Issues: [Repository Issues]
- Documentation: [Project Documentation]
- Community: [Discord/Forum Links]
