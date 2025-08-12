use crate::board_utils::Square;
use crate::pieces::Piece;
use crate::pointer_events::Timer;
use bevy::input::ButtonInput;
use bevy::picking::pointer::{PointerInteraction, PointerPress};
use bevy::prelude::*;
use bevy::time::Time;

#[derive(Default, Resource)]
struct SelectedSquare {
    entity: Option<Entity>,
}

#[derive(Default)]
struct SelectedPiece {
    entity: Option<Entity>,
}

fn select_square(
    mut click_events: EventReader<Pointer<Click>>,
    mut hover_events: EventReader<Pointer<Over>>,
    squares_query: Query<(), With<Square>>,
    hover_query: Query<(), With<Square>>,
    mut selected_square: ResMut<SelectedSquare>,
    mut timer: ResMut<Timer>,
    time: Res<Time>,
) {
    timer.time += time.delta_secs();
    if timer.time >= 1.0 {
        for click in click_events.read() {
            let clicked_entity = click.target;

            for over in hover_events.read() {
                let hovered_entity = over.target;

                if hover_query.get(hovered_entity).is_ok() {
                    info!("Hovering square {}", hovered_entity.index());

                    if squares_query.get(clicked_entity).is_ok() {
                        info!("Selecting square: {}", clicked_entity.index());
                        selected_square.entity = Some(clicked_entity);
                    } else {
                        info!("Deselecting.");
                        selected_square.entity = None;
                    }
                }
            }
        }
        timer.time = 0.0;
    }
}

pub fn move_pieces(time: Res<Time>, mut query: Query<(&mut Transform, &Piece)>) {
    for (mut transform, piece) in query.iter_mut() {
        let direction = Vec3::new(piece.x as f32, 0., piece.y as f32) - transform.translation;
        if direction.length() > 0.1 {
            transform.translation += direction.normalize() * time.delta_secs();
        }
    }
}

pub struct BoardEventsPlugin;
impl Plugin for BoardEventsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedSquare>();
        app.add_systems(Update, select_square);
        app.add_systems(Update, move_pieces);
    }
}

