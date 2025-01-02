use bevy::{color::palettes::tailwind::*, prelude::*};
use bevy::picking::pointer::{PointerId, PointerLocation, PointerMap, PointerInteraction};
use bevy::picking::focus::{HoverMap, PreviousHoverMap};


        pub fn pointer_event_system(
            pointers: Query<&PointerLocation>,
            pointer_map: Res<PointerMap>,
            hover_map: Res<HoverMap>,
            previous_hover_map: Res<PreviousHoverMap>,
            mut commands: Commands,
        ) {

            let pointer_location =| pointer_id: PointerId |{
                pointer_map
                .get_entity(pointer_id)
                .and_then(|entity| pointers.get(entity).ok())
                .and_then(|pointer| pointer.location.clone())
            }; 
        }


        
             
 

    
    
    // Get pointer location
    // get pointer entity
    



    pub fn update_entitymatl_on<E>(
        new_material: Handle<StandardMaterial>,
         ) -> impl Fn(Trigger<E>, Query<&mut MeshMaterial3d<StandardMaterial>>) {
         move |trigger, mut query| {
             if let Ok(mut material) = query.get_mut(trigger.entity()) {
                 material.0 = new_material.clone();
             }
         }
     }



    pub fn update_squarematl_on<E>(
        new_material: Handle<StandardMaterial>,
         ) -> impl Fn(Trigger<E>, Query<&mut MeshMaterial3d<StandardMaterial>>) {
         move |trigger, mut query| {
             if let Ok(mut material) = query.get_mut(trigger.entity()) {
                 material.0 = new_material.clone();
             }
         }
     }

     
    pub fn revert_squarematl_on<Out>(
        new_material: Handle<StandardMaterial>,
         ) -> impl Fn(Trigger<Out>, Query<&mut MeshMaterial3d<StandardMaterial>>) {
         move |trigger, mut query| {
             if let Ok(mut material) = query.get_mut(trigger.entity()) {
                 material.0 = new_material.clone();
             }
         }
     }
 
         pub fn draw_mesh_intersections(pointers: Query<&PointerInteraction>, mut gizmos: Gizmos) {
             for (point, normal) in pointers
             .iter()
             .filter_map(|interaction| interaction.get_nearest_hit())
             .filter_map(|(_entity, hit)| hit.position.zip(hit.normal))
             {
                 gizmos.sphere(point, 0.05, RED_500);
                 gizmos.arrow(point, point + normal.normalize() * 0.5, PINK_100);
             }
         }

         