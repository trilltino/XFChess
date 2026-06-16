//! Viewport-pinned in-game HUD.
//!
//! This is a heads-up overlay that stays pinned to a screen corner regardless of
//! where the board, camera or surrounding UI sits. It shows the side to move and
//! move number, anchored to the top-right of the viewport while a game is in
//! progress.
//!
//! ## `FixedNode` and the 0.19 release line
//!
//! Bevy 0.19 introduces [`FixedNode`], a marker that positions a UI node relative
//! to the **camera viewport** rather than its parent element (ignoring the
//! parent's layout, clipping and transform). That is the purpose-built primitive
//! for HUD overlays nested inside other UI.
//!
//! `FixedNode` landed *after* `0.19.0-rc.3`, the release this project currently
//! builds against, so it is gated behind the `bevy_fixed_node` cargo feature.
//! Because this HUD is a **root** node (it has no UI parent), an absolute-
//! positioned [`Node`] is already anchored to the viewport, so the default build
//! is visually identical. Enable `bevy_fixed_node` once a Bevy release that
//! exports `FixedNode` is pinned to also tag the node explicitly (and to make it
//! robust if the HUD is ever re-parented under another UI tree).
//!
//! [`FixedNode`]: https://docs.rs/bevy_ui/latest/bevy_ui/struct.FixedNode.html

use crate::core::{DespawnOnExit, GameState};
use crate::game::components::piece_types::PieceColor;
use crate::game::resources::turn::current::CurrentTurn;
use bevy::prelude::*;

/// Marker for the HUD's dynamic text node, so the update system can find it.
#[derive(Component)]
struct TurnHudText;

/// Plugin that owns the fixed (viewport-anchored) in-game HUD.
pub struct FixedHudPlugin;

impl Plugin for FixedHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn_turn_hud)
            .add_systems(
                Update,
                update_turn_hud.run_if(in_state(GameState::InGame)),
            );
    }
}

/// Spawn the pinned HUD container and its text child on entering a game.
fn spawn_turn_hud(mut commands: Commands, current_turn: Res<CurrentTurn>) {
    let mut container = commands.spawn((
        // Absolute offsets on a root node are measured from the viewport edges,
        // so this stays pinned to the top-right corner.
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            right: px(16),
            padding: UiRect::axes(px(12), px(8)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        DespawnOnExit(GameState::InGame),
        Name::new("Fixed Turn HUD"),
    ));

    // When a Bevy release that exports `FixedNode` is pinned, tag the node so it
    // is anchored to the viewport even if re-parented under another UI tree.
    #[cfg(feature = "bevy_fixed_node")]
    container.insert(bevy::ui::FixedNode);

    container.with_children(|parent| {
        parent.spawn((
            Text::new(hud_label(&current_turn)),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(Color::WHITE),
            TurnHudText,
            Name::new("Fixed Turn HUD Text"),
        ));
    });
}

/// Refresh the HUD text whenever the turn changes.
fn update_turn_hud(
    current_turn: Res<CurrentTurn>,
    mut text_query: Query<&mut Text, With<TurnHudText>>,
) {
    if !current_turn.is_changed() {
        return;
    }
    for mut text in text_query.iter_mut() {
        *text = Text::new(hud_label(&current_turn));
    }
}

fn hud_label(current_turn: &CurrentTurn) -> String {
    let side = match current_turn.color {
        PieceColor::White => "White",
        PieceColor::Black => "Black",
    };
    format!("Move {} — {} to play", current_turn.move_number, side)
}
