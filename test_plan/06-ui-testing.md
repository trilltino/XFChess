# UI Testing Guide

Testing bevy_egui UI systems requires special considerations since EGUI runs in a separate pass.

## Challenges

1. **EGUI Context**: Requires `EguiContext` resource
2. **Immediate Mode**: UI is redrawn every frame
3. **Side Effects**: UI actions trigger state changes

## Testing Approach

### Test State Changes, Not Rendering

Instead of testing EGUI rendering, test the state changes triggered by UI:

```rust
// src/ui/auth.rs
#[derive(Resource, Default)]
pub struct AuthState {
    pub username: String,
    pub password: String,
    pub is_authenticated: bool,
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_default() {
        let state = AuthState::default();
        assert!(state.username.is_empty());
        assert!(!state.is_authenticated);
    }

    #[test]
    fn test_validate_credentials() {
        let mut state = AuthState::default();
        state.username = "user".to_string();
        state.password = "pass".to_string();
        
        // Test validation logic
        let is_valid = !state.username.is_empty() && state.password.len() >= 4;
        assert!(is_valid);
    }
}
```

### Testing UI Resources

```rust
// src/ui/chat.rs
#[derive(Resource, Default)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub input_text: String,
    pub is_visible: bool,
}

impl ChatState {
    pub fn add_message(&mut self, sender: &str, content: &str) {
        self.messages.push(ChatMessage {
            sender: sender.to_string(),
            content: content.to_string(),
            timestamp: Instant::now(),
        });
    }
    
    pub fn clear_input(&mut self) {
        self.input_text.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_message() {
        let mut chat = ChatState::default();
        chat.add_message("Player1", "Hello!");
        
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].content, "Hello!");
    }

    #[test]
    fn test_clear_input() {
        let mut chat = ChatState::default();
        chat.input_text = "draft message".to_string();
        chat.clear_input();
        
        assert!(chat.input_text.is_empty());
    }
}
```

### Testing Menu State Transitions

```rust
// src/states/main_menu.rs
#[derive(Resource, Default)]
pub struct MainMenuState {
    pub selected_option: MenuOption,
    pub is_transitioning: bool,
}

#[derive(Default, PartialEq, Clone, Copy)]
pub enum MenuOption {
    #[default]
    Play,
    Settings,
    Multiplayer,
    Quit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_navigation() {
        let mut menu = MainMenuState::default();
        assert_eq!(menu.selected_option, MenuOption::Play);
        
        menu.selected_option = MenuOption::Settings;
        assert_eq!(menu.selected_option, MenuOption::Settings);
    }
}
```

## Testing UI Systems with Mock Context

For systems that must interact with EGUI:

```rust
use bevy::prelude::*;
use bevy_egui::{EguiContext, EguiPlugin};

fn create_egui_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(EguiPlugin);
    
    // EGUI needs a window entity
    app.world_mut().spawn(Window::default());
    
    app
}

#[test]
fn test_ui_system_with_egui() {
    let mut app = create_egui_test_app();
    app.init_resource::<ChatState>();
    
    // Add the UI system
    app.add_systems(Update, chat_system);
    
    // This will run EGUI context
    app.update();
    
    // Verify state after UI frame
    let chat = app.world().resource::<ChatState>();
    assert!(!chat.is_visible); // Default state
}
```

## Testing Promotion UI

```rust
// src/ui/promotion_ui.rs
#[derive(Resource, Default)]
pub struct PromotionState {
    pub pending: bool,
    pub position: Option<(u8, u8)>,
    pub selected_piece: Option<PieceType>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promotion_selection() {
        let mut state = PromotionState {
            pending: true,
            position: Some((0, 7)),
            selected_piece: None,
        };
        
        // Simulate queen selection
        state.selected_piece = Some(PieceType::Queen);
        state.pending = false;
        
        assert_eq!(state.selected_piece, Some(PieceType::Queen));
        assert!(!state.pending);
    }
}
```

## Testing Game Status UI

```rust
// src/ui/game_ui.rs
use xfchess::game::resources::{CurrentTurn, GameTimer};

#[test]
fn test_format_timer() {
    let timer = GameTimer {
        white_time: Duration::from_secs(125), // 2:05
        black_time: Duration::from_secs(59),  // 0:59
    };
    
    assert_eq!(format_time(timer.white_time), "2:05");
    assert_eq!(format_time(timer.black_time), "0:59");
}

fn format_time(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
```

## Snapshot Testing (Future)

For visual regression testing, consider:

```rust
// Using insta for snapshot tests
#[test]
fn test_menu_layout() {
    let layout = render_menu_to_string();
    insta::assert_snapshot!(layout);
}
```

## Running UI Tests

```bash
# Run UI-related tests
cargo test ui

# Run with EGUI (may need display)
cargo test --features egui-tests
```
