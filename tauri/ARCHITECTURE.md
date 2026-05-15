# XFChess Tauri Architecture

This document describes the architecture of the XFChess Tauri application, including its components, communication patterns, and design decisions.

## Overview

XFChess Tauri is a multi-window desktop application built with the Tauri framework. It combines a Rust backend with web-based frontends to provide a seamless chess platform experience with wallet integration and tournament administration capabilities.

## Core Components

### 1. Application Entry Point (`main.rs`)

The main entry point initializes the Tauri application and sets up:
- **Logging System**: Configured tracing subscriber with environment-based filtering
- **Shared State**: Global application state including wallet pubkey and pending transactions
- **Window Management**: Creates and manages multiple specialized windows
- **IPC Handlers**: Registers command handlers for frontend-backend communication

### 2. Services Layer (`services/`)

Business logic and core services are organized into:

#### Authentication Service (`services/auth.rs`)
- **AuthState**: Manages user authentication state
- **Session Management**: Handles user sessions and tokens
- **Security**: Implements secure authentication flows

#### Configuration Service (`services/config.rs`)
- **Environment Variables**: Centralized configuration management
- **Default Values**: Sensible defaults for development and production
- **Runtime Configuration**: Dynamic configuration updates

#### IPC Service (`services/ipc.rs`)
- **Command Handlers**: Implements all IPC commands
- **Window Commands**: Tournament admin window management
- **System Commands**: Notifications, clipboard, URL opening
- **Type Safety**: Strongly typed command parameters and responses

### 3. Type System (`types/`)

Strongly typed definitions for all application data:

#### Authentication Types (`types/auth.rs`)
- User and session structures
- Token and credential types
- Permission definitions

#### Configuration Types (`types/config.rs`)
- Configuration structures
- Environment variable mappings
- Validation schemas

#### IPC Types (`types/ipc.rs`)
- Command and response types
- Window operation enums
- Event message structures

### 4. Utility Layer (`utils/`)

Shared utilities and helper functions:

#### Cryptographic Utilities (`utils/crypto.rs`)
- Hashing and encryption functions
- Key derivation and validation
- Secure random number generation

#### Logging Utilities (`utils/logging.rs`)
- Structured logging setup
- Event-specific logging functions
- Performance and security event tracking

#### Validation Utilities (`utils/validation.rs`)
- Input validation functions
- Sanitization routines
- Security checks

### 5. Window Management (`windows/`)

Specialized window managers for different application windows:

#### Tournament Admin Window (`windows/tournament_admin.rs`)
- **Window Builder**: Configures tournament admin window properties
- **Window Operations**: Show, hide, resize, position controls
- **Event Handling**: Window lifecycle management

#### Wallet Window (`windows/wallet.rs`)
- **Browser Integration**: Opens wallet popup in default browser
- **Extension Communication**: Facilitates communication with wallet extensions
- **Security**: Isolated wallet operations

#### Popup Window (`windows/popup.rs`)
- **Utility Popups**: Generic popup window management
- **Modal Support**: Modal dialog functionality
- **Event Handling**: Popup lifecycle management

## Communication Patterns

### 1. IPC (Inter-Process Communication)

The application uses Tauri's IPC system for frontend-backend communication:

#### Command Pattern
```rust
#[tauri::command]
pub fn command_name(param: Type, app: AppHandle) -> Result<ResponseType, Error> {
    // Command implementation
}
```

#### Event Emission
```rust
app.emit("event-name", &payload)?;
```

#### Event Listening
```rust
window.listen("event-name", |event| {
    // Handle event
});
```

### 2. Window-to-Window Communication

Windows communicate through:
- **Shared State**: Global state managed by the main application
- **Event System**: Tauri's event system for cross-window messaging
- **Direct References**: Window handles for direct manipulation

### 3. External Service Communication

#### Backend API
- **HTTP Client**: Axum-based HTTP client for backend communication
- **Authentication**: Secure API key management
- **Error Handling**: Robust error handling and retry logic

#### Wallet Extensions
- **Browser Bridge**: Communication with browser-based wallet extensions
- **Protocol Handlers**: Custom URL scheme handling
- **Security**: Isolated communication channels

## Security Architecture

### 1. Sandboxing

- **Webview Sandboxing**: Each window runs in an isolated webview
- **File System Access**: Limited to specific directories
- **Network Access**: Controlled through Content Security Policy

### 2. Content Security Policy

Strict CSP configuration:
```json
{
  "default-src": "'self'",
  "connect-src": "'self' ws: http://localhost:8090 https://api.xfchess.com",
  "script-src": "'self' 'unsafe-inline'",
  "style-src": "'self' 'unsafe-inline'",
  "img-src": "'self' data: https:",
  "font-src": "'self' data:",
  "object-src": "'none'",
  "media-src": "'self'",
  "frame-src": "'none'"
}
```

### 3. Permission System

Minimal permissions requested:
- **Window Management**: Basic window operations
- **File System**: App config and data directories only
- **System**: Clipboard, URL opening, notifications
- **Network**: Specific backend endpoints

## State Management

### 1. Global State

Shared application state managed by Tauri:
```rust
app.manage(wallet_pubkey);
app.manage(pending_tx);
app.manage(auth_state);
```

### 2. Window State

Each window maintains its own state:
- **Window Properties**: Size, position, visibility
- **UI State**: Component-specific state
- **Session State**: User session data

### 3. Persistent State

Configuration and user data persisted to:
- **App Config Directory**: Application configuration
- **App Data Directory**: User data and cache
- **System Registry**: Windows-specific settings

## Performance Optimizations

### 1. Build Optimizations

Release profile optimizations:
```toml
[profile.release]
panic = "abort"
codegen-units = 1
lto = true
incremental = false
opt-level = "s"
strip = true
```

### 2. Runtime Optimizations

- **Async Operations**: Non-blocking I/O throughout
- **Memory Management**: Efficient memory usage patterns
- **Resource Management**: Proper cleanup and disposal

### 3. Asset Optimization

- **Asset Compression**: Compressed assets in release builds
- **Lazy Loading**: Assets loaded on demand
- **Caching**: Intelligent asset caching

## Error Handling

### 1. Error Types

Custom error types for different domains:
- **AppError**: General application errors
- **IpcError**: IPC communication errors
- **WindowError**: Window management errors
- **ConfigError**: Configuration errors

### 2. Error Propagation

Consistent error handling patterns:
```rust
fn operation() -> Result<Success, AppError> {
    // Implementation
}
```

### 3. User Feedback

- **Error Messages**: User-friendly error descriptions
- **Notifications**: System notifications for important events
- **Logging**: Detailed error logging for debugging

## Testing Strategy

### 1. Unit Tests

- **Service Tests**: Test business logic in isolation
- **Utility Tests**: Test helper functions
- **Type Tests**: Test serialization/deserialization

### 2. Integration Tests

- **IPC Tests**: Test command handlers
- **Window Tests**: Test window management
- **API Tests**: Test backend integration

### 3. End-to-End Tests

- **User Workflows**: Test complete user journeys
- **Cross-Platform**: Test on different platforms
- **Performance**: Test performance characteristics

## Development Workflow

### 1. Development Mode

```bash
cargo tauri dev --features all
```

Features:
- **Hot Reload**: Automatic frontend reload
- **DevTools**: Browser developer tools
- **Debug Logging**: Verbose logging output

### 2. Testing

```bash
cargo test                    # Unit tests
cargo test --test integration # Integration tests
```

### 3. Building

```bash
cargo tauri build --features all  # Release build
```

## Future Enhancements

### 1. Plugin Architecture

- **Modular Plugins**: Pluggable feature modules
- **Plugin Registry**: Dynamic plugin loading
- **Plugin API**: Standardized plugin interface

### 2. Advanced Security

- **Code Signing**: Platform-specific code signing
- **Auto-Updates**: Secure update mechanism
- **Sandboxing**: Enhanced isolation

### 3. Performance

- **WebGPU**: Hardware acceleration
- **Web Workers**: Background processing
- **Streaming**: Real-time data streaming

## Conclusion

The XFChess Tauri architecture is designed for:
- **Security**: Isolated execution and minimal permissions
- **Performance**: Optimized builds and efficient runtime
- **Maintainability**: Modular design and clear separation of concerns
- **Extensibility**: Plugin architecture and feature flags
- **User Experience**: Responsive interface and smooth interactions

This architecture provides a solid foundation for a modern desktop chess application with wallet integration and tournament management capabilities.
