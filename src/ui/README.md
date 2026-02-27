# UI Module

## Purpose

The UI module implements all user interface components for XFChess using Bevy's EGUI integration. It provides menus, panels, HUD elements, and interactive controls that allow players to navigate the game, configure settings, and interact with blockchain features.

## Impact on Game

This module provides:
- **Authentication UI**: Wallet connection and session management
- **Game HUD**: In-game information display (turn, captured pieces, move history)
- **Multiplayer Menu**: Lobby creation, game joining, and connection status
- **Promotion Dialog**: Piece selection when pawn reaches the end
- **Solana Panel**: Blockchain integration interface (when Solana feature enabled)
- **Consistent Styling**: Unified color scheme, typography, and component design

## Architecture/Key Components

### Main UI Plugins

| Plugin | File | Purpose |
|--------|------|---------|
| [`UiPlugin`](mod.rs:18) | `mod.rs` | Entry point, registers all UI plugins |
| [`AuthUiPlugin`](auth.rs) | `auth.rs` | Wallet connection and authentication |
| [`GameUiPlugin`](game_ui.rs) | `game_ui.rs` | In-game HUD and information panels |
| [`MultiplayerMenuPlugin`](multiplayer_menu.rs) | `multiplayer_menu.rs` | Multiplayer lobby interface |
| [`PromotionUiPlugin`](promotion_ui.rs) | `promotion_ui.rs` | Pawn promotion selection dialog |
| [`SolanaPanelPlugin`](solana_panel.rs) | `solana_panel.rs` | Blockchain status and controls |

### Styling System

| Module | Purpose |
|--------|---------|
| [`styles/colors.rs`](styles/colors.rs) | Color palette and theme colors |
| [`styles/typography.rs`](styles/typography.rs) | Font sizes, text styles |
| [`styles/components.rs`](styles/components.rs) | Reusable styled components |

### Key UI Components

- **Auth Panel**: Wallet connection status, session key display
- **Game Info**: Current turn, move history, captured pieces
- **Connection Status**: Multiplayer connection indicators
- **Settings Controls**: Sliders, toggles, dropdowns
- **Action Buttons**: Styled buttons with hover effects

## Usage

### Creating a UI Panel

```rust
fn my_ui_panel(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<UiState>,
) {
    egui::Window::new("My Panel")
        .show(contexts.ctx_mut(), |ui| {
            ui.label("Welcome to XFChess!");
            
            if ui.button("Click Me").clicked() {
                ui_state.clicked = true;
            }
        });
}
```

### Using Styled Components

```rust
use crate::ui::styles::components::StyledButton;
use crate::ui::styles::colors::PRIMARY;

fn styled_ui(mut contexts: EguiContexts) {
    egui::CentralPanel::default().show(contexts.ctx_mut(), |ui| {
        ui.visuals_mut().override_text_color = Some(PRIMARY);
        
        if StyledButton::new("Connect Wallet").show(ui) {
            // Handle connection
        }
    });
}
```

### Conditional UI (Feature Gates)

```rust
#[cfg(feature = "solana")]
fn solana_panel(
    mut contexts: EguiContexts,
    solana: Res<SolanaClient>,
) {
    egui::Window::new("Solana")
        .show(contexts.ctx_mut(), |ui| {
            ui.label(format!("Wallet: {}", solana.wallet_address()));
            ui.label(format!("Balance: {} SOL", solana.balance()));
        });
}
```

### System Parameters Helper

```rust
use crate::ui::system_params::UiParams;

fn ui_system(
    params: UiParams,
) {
    // Access all common UI resources at once
    let UiParams { 
        contexts, 
        settings, 
        game_state 
    } = params;
    
    // Use contexts for EGUI rendering
}
```

## Dependencies

- [`bevy_egui`](https://docs.rs/bevy_egui) - EGUI integration for Bevy
- [`egui`](https://docs.rs/egui) - Immediate mode GUI library
- [`core`](../core/README.md) - Access to game state and settings

## Related Modules

- [`core`](../core/README.md) - Settings and state access
- [`states`](../states/README.md) - State-specific UI screens
- [`solana`](../solana/README.md) - Blockchain data for UI
- [`multiplayer`](../multiplayer/README.md) - Connection status

## UI Design Principles

1. **Consistency**: All UI uses the style system for uniform appearance
2. **Responsiveness**: UI adapts to window resizing
3. **Accessibility**: Clear labels, sufficient contrast
4. **Performance**: Minimal per-frame allocations
