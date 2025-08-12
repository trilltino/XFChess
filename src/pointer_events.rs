
     use bevy::prelude::*;
     use bevy::picking::pointer::{PointerInteraction,PointerPress};
     use crate::pieces::PieceColor;
     use bevy::window::PrimaryWindow;
     use bevy::time::Time;

    #[derive(Resource,Debug)]
    pub struct Timer {
            pub time:f32,
    }

    impl FromWorld for Timer {
        fn from_world(world: &mut World) -> Self {
            Timer { time: 0.0 }
        }
    }
    

    pub fn cursor_position(
    q_windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    mut timer: ResMut<Timer>,) {

    timer.time += time.delta_secs(); 
    if timer.time >= 1.0 {
    match q_windows.single() {
            Ok(window) => {
                if let Some(position) = window.cursor_position() {
                println!("Cursor is inside the primary window, at {position:?}")
                } else {
                    println!("Cursor is not in the game window.");
                }
            }
            Err(_) => {
                println!("Failed to get the primary window.");
            }
        }
                timer.time = 0.0;
    }
 }


    pub fn entity_cursor(
        q_entity_query: Query<(&PointerInteraction, &PieceColor)>,
        q_pointer_press_query: Query<&PointerPress>,
        time: Res<Time>,
        mut timer: ResMut<Timer>,) {
        timer.time += time.delta_secs();   
        if timer.time >= 1.0 {
        for (pointer_interaction, piece)  in q_entity_query.iter() {
            if let Some(_hit) = pointer_interaction.get_nearest_hit() {
        println!("Cursor is over entity {piece:?},");
            for q_press in q_pointer_press_query.iter() {
            if q_press.is_any_pressed() {
            println!("Mouse clicked on piece: {:?}", piece);
        } else {
            println!("Cursor is over a piece, but nothing is clicked.");
        }
            }
        }
        timer.time= 0.0;
        }
    }
}

    pub fn update_squarematl_on<E>(
        new_material: Handle<StandardMaterial>,
        ) -> impl Fn(Trigger<E>, Query<&mut MeshMaterial3d<StandardMaterial>>) {
        move |trigger, mut query| {
            if let Ok(mut material) = query.get_mut(trigger.target()) {
                material.0 = new_material.clone();
            }
        }
    }

    pub fn revert_squarematl_on<E>(
        new_material: Handle<StandardMaterial>,
        ) -> impl Fn(Trigger<E>, Query<&mut MeshMaterial3d<StandardMaterial>>) {
        move |trigger, mut query| {
            if let Ok(mut material) = query.get_mut(trigger.target()) {
                material.0 = new_material.clone();
            }
        }
    }

pub struct PointerEventsPlugin;
        impl Plugin for PointerEventsPlugin {
            fn build(&self, app: &mut App) {
                    app.add_systems(Update, cursor_position);
                    app.add_systems(Update, entity_cursor);
                    app.init_resource::<Timer>();
            }
        }
            
                
 