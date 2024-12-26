use bevy::prelude::*;
use bevy::picking::{PickingEventWriters, PointerEvent, PointerInteraction, PickingCamera, PickingPlugin, SelectionPlugin, PointerId};



pub fn pointer_events(
    mut input_events: EventReader<PointerEvent>,
    pointers: Query<&PointerInteraction>,
    hover_map: Res<HoverMap>,
    mut commands: Commands,
    mut event_writers: PickingEventWriters,
    square_materials : Handle<SquareMaterials>,
    standard_materials : Handle<StandardMaterial>,
//
  









    pub fn update_material_on<E>(
        new_material: Handle<StandardMaterial>,
    )-> impl Fn(Trigger<E>, Query<&mut MeshMaterial3d<StandardMaterial>>) {
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
