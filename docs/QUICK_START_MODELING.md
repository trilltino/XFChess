# Quick Start: Adding Chess Piece Models

## TL;DR - Three Ways to Add Models

### 1. Use Existing GLTF Models (Easiest)

```rust
// In src/rendering/pieces.rs - update PieceMeshes
let piece_meshes = PieceMeshes {
    king: asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0"),
    king_cross: asset_server.load("models/chess_kit/pieces.glb#Mesh1/Primitive0"),
    // Add your new model paths here
};
```

**Steps**:
1. Download or create `.glb` files
2. Place in `assets/models/chess_kit/`
3. Update mesh paths in code
4. Run game

### 2. Create Simple Primitive Pieces (Fast Prototype)

```rust
use bevy::prelude::*;

fn create_pawn_from_primitives(
    commands: &mut Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    position: (u8, u8),
    color: PieceColor,
) {
    let base = meshes.add(Cylinder::new(0.3, 0.1));
    let body = meshes.add(Cylinder::new(0.2, 0.3));
    let head = meshes.add(Sphere::new(0.15));
    
    let material = materials.add(StandardMaterial {
        base_color: if color == PieceColor::White { Color::WHITE } else { Color::BLACK },
        ..default()
    });
    
    commands.spawn((
        Transform::from_translation(Vec3::new(position.0 as f32, 0.0, position.1 as f32)),
        Piece { color, piece_type: PieceType::Pawn, x: position.0, y: position.1 },
        HasMoved::default(),
        // ... other components
    )).with_children(|parent| {
        // Base
        parent.spawn((
            Mesh3d(base),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::new(0.0, 0.05, 0.0)),
        ));
        // Body
        parent.spawn((
            Mesh3d(body),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::new(0.0, 0.25, 0.0)),
        ));
        // Head
        parent.spawn((
            Mesh3d(head),
            MeshMaterial3d(material),
            Transform::from_translation(Vec3::new(0.0, 0.55, 0.0)),
        ));
    });
}
```

### 3. Create Manual Mesh (Programmatic)

```rust
use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;

fn create_simple_pawn_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            // Bottom square (4 vertices)
            [-0.3, 0.0, -0.3], [0.3, 0.0, -0.3], 
            [0.3, 0.0, 0.3], [-0.3, 0.0, 0.3],
            // Top square (4 vertices)
            [-0.2, 0.6, -0.2], [0.2, 0.6, -0.2],
            [0.2, 0.6, 0.2], [-0.2, 0.6, 0.2],
        ],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![
            [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], 
            [0.0, -1.0, 0.0], [0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0], [0.0, 1.0, 0.0],
        ],
    )
    .with_inserted_indices(Indices::U32(vec![
        // Bottom face
        0, 1, 2, 0, 2, 3,
        // Top face
        4, 6, 5, 4, 7, 6,
        // Side faces
        0, 4, 1, 1, 4, 5,
        1, 5, 2, 2, 5, 6,
        2, 6, 3, 3, 6, 7,
        3, 7, 0, 0, 7, 4,
    ]))
}

// Use it:
let pawn_mesh = meshes.add(create_simple_pawn_mesh());
commands.spawn((
    Mesh3d(pawn_mesh),
    MeshMaterial3d(material),
    // ... other components
));
```

## File Structure

```
assets/
└── models/
    └── chess_kit/
        ├── pieces.glb          # Your GLTF model file
        └── pieces_texture.png  # Optional texture
```

## Blender Quick Export

1. **Create/Open Model**: Model your chess pieces in Blender
2. **Scale**: Ensure pieces are ~1 unit tall
3. **Export**: File → Export → glTF 2.0 (.glb)
4. **Settings**:
   - Format: `glTF Binary (.glb)`
   - Include: `Selected Objects` or `All`
   - Transform: `+Y Up`
   - Geometry: `Apply Modifiers`
5. **Save**: Place in `assets/models/chess_kit/`

## Testing Checklist

- [ ] Model file exists in `assets/models/chess_kit/`
- [ ] Mesh path in code matches file name
- [ ] Mesh index is correct (`#Mesh0/Primitive0`)
- [ ] Pieces appear at correct positions
- [ ] White/black materials apply correctly
- [ ] Pieces scale appropriately (not too big/small)
- [ ] No console errors on load

## Common Issues & Fixes

**"Asset not found" error**:
```rust
// Wrong: Missing #Mesh0/Primitive0
asset_server.load("models/chess_kit/pieces.glb")

// Correct: Include mesh path
asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0")
```

**Pieces too big/small**:
```rust
// Adjust scale in piece_transform()
fn piece_transform(offset: Vec3) -> Transform {
    let mut t = Transform::from_translation(offset);
    t.scale = Vec3::splat(0.2); // Change 0.2 to adjust size
    t
}
```

**Pieces appear black**:
- Check that lights are spawned in scene
- Verify material has base_color set
- Check normals face outward in Blender

**Model loads but doesn't render**:
- Ensure `Mesh3d` and `MeshMaterial3d` components are added
- Check `Transform` component exists
- Verify camera can see the pieces

## Next Steps

1. **Start with primitives** to test the system
2. **Download a free GLTF model** to see how it looks
3. **Create your own in Blender** for custom pieces
4. **Add textures** for enhanced appearance
5. **Optimize** by combining meshes or reducing polygons

See `docs/GRAPHICS_MODELING_GUIDE.md` for complete documentation.

