use bevy::prelude::*;
use bevy::picking::pointer::{PointerMap, PointerId};
use bevy::picking::backend::HitData;
use bevy::picking::pointer::{PointerLocation, PointerInput, PointerButton, PressDirection};
use bevy::ecs::system::SystemParam;
use std::time::Instant;
use std::collections::HashMap;



        #[derive(Debug, Clone, Copy, Reflect)]
        enum PointerAction {
            Pressed {
                direction: PressDirection,
                button: PointerButton,
            },
        } 
    

        #[derive(Event, Debug, Clone, Copy, PartialEq, Eq)]
        pub struct MouseButtonInput {
            pub button: MouseButton,
            pub state: ButtonState,
            pub window: Entity,
        }



        #[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
        pub enum MouseButton {
            Left,
            Right,
        }

        #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
        pub enum ButtonState {
            Pressed,
            Released,
        }

    
        #[derive(Debug, Default, Clone, Component, Reflect, PartialEq, Eq)]
        pub struct PointerPress {
            primary: bool,
            secondary: bool,
            middle: bool,
        }

         
        #[derive(Clone, PartialEq, Debug, Reflect)]
        pub struct Down {
            pub button: PointerButton,
            pub hit: HitData,
        }


        #[derive(Clone, PartialEq, Debug, Reflect)]
        pub struct Over {
            pub hit: HitData,
        }

            
        #[derive(Debug, Deref, DerefMut, Default, Resource)]
        pub struct HoverMap(pub HashMap<PointerId, HashMap<Entity, HitData>>);


        impl HoverMap {
            pub fn get_entity(&self, _location: impl Fn(PointerId) -> Option<Vec2>) -> Option<Entity> {
                None 
            }
        }

        #[derive(Debug, Deref, DerefMut, Default, Resource)]
        pub struct PreviousHoverMap(pub HashMap<PointerId, HashMap<Entity, HitData>>);

        #[derive(SystemParam)]
        pub struct PickingEventWriters <'w> {
            hover_events: EventWriter<'w, Pointer<Over>>,
            down_events: EventWriter<'w, Pointer<Down>>,
            click_events: EventWriter<'w, Pointer<Click>>,
        }

        #[allow(clippy::too_many_arguments)]
        pub fn pointer_event_system(
            mut input_events: EventReader<PointerInput>,
            pointers: Query<&PointerLocation>,
            pointer_map: Res<PointerMap>,
            hover_map: Res<HoverMap>,
            previous_hover_map: Res<PreviousHoverMap>,
            mut pointer_state: ResMut<PointerState>,
            mut commands: Commands,
            mut event_writers: PickingEventWriters,
            mut pointer_events: EventWriter<PointerInput>,
        ) {

            let now = Instant::now();
            let pointer_location =| pointer_id: PointerId |{
                pointer_map
                .get_entity(pointer_id)
                .and_then(|entity| pointers.get(entity).ok())
                .and_then(|pointer| pointer.location.clone())
            }; 

            for (pointer_id, hovered_entity, hit) in previous_hover_map
            .iter()
            .flat_map(|(id,hashmap)| hashmap.iter().map(|data| (*id, *data.0, data.1.clone())))
            {
                if !hover_map
                .get(&pointer_id)
                .iter()
                .any(|e| e.contains_key(&hovered_entity))
                {
                    let Some(location) = pointer_location(pointer_id) else {
                        debug! (
                            "Unable to get pointer location for pointer_id {:?} while not over entity",
                            pointer_id
                        );
                        continue;
                    };
                }
            }

            for PointerInput {
                pointer_id,
                location,
                action,
            } 
            
            in input_events.read().cloned() {
                match action {
                    PointerAction::Pressed { direction, button} => {
                        let state = pointer_state.get_mut(pointer_id, button);

                        match direction {
                            PressDirection::Down => {
                                for (hovered_entity, hit) in hover_map
                                .get(&pointer_id)
                                .iter()
                                .flat_map(|h| h.iter().map(|(entity, data)| (*entity, data.clone())))
                            {
                                let down_event = Pointer::new(
                                    hovered_entity,
                                    pointer_id,
                                    location.clone(),
                                    Down {
                                        button,
                                        hit: hit.clone(),
                                    },
                                );
                                commands.trigger_targets(down_event.clone(), hovered_entity);
                                event_writers.down_events.send(down_event);
                                // Also insert the press into the state
                                state
                                    .pressing
                                    .insert(hovered_entity, (location.clone(), now, hit));
                            }
                        }
                       
                        PressDirection::Up => {
                            for (hovered_entity, hit) in previous_hover_map
                                .get(&pointer_id)
                                .iter()
                                .flat_map(|h| h.iter().map(|(entity, data)| (*entity, data.clone())))
                            {
                                if let Some((_, press_instant, _)) = state.pressing.get(&hovered_entity)
                                {
                                    let click_event = Pointer::new(
                                        hovered_entity,
                                        pointer_id,
                                        location.clone(),
                                        Click {
                                            button,
                                            hit: hit.clone(),
                                            duration: now - *press_instant,
                                        },
                                    );
                                    commands.trigger_targets(click_event.clone(), hovered_entity);
                                    event_writers.click_events.send(click_event);
                                }                      
                            }
                        }
                    }
                }
            }
        }
    } 

    // Get pointer location
    // get pointer entity
    