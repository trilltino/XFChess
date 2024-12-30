use bevy::prelude::*;
use bevy::picking::pointer::{PointerLocation, PointerMap};
use crate::board::{Square, SquareMaterials, SelectedSquare};


    pub fn on_hover_color_board (   
        _hover: Trigger<Pointer<Over>>,
        pointers:  Query<&PointerLocation>,
        pointer_map: Res<PointerMap>,
        selected_square: Res<SelectedSquare>,
        materials: Res<SquareMaterials>,
        mut query: Query<(Entity, &Square, &mut Handle<StandardMaterial>)>,

    ) {
        let hovered_square = 




 
   





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
}