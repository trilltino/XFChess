# Graphics Modeling Guide for Chess Pieces in Bevy

This guide explains how to create and integrate 3D models for chess pieces in your Bevy chess game.

## Overview

Your current setup uses GLTF models loaded from `assets/models/chess_kit/pieces.glb`. This guide covers:
1. **Creating Models**: Using 3D modeling software or procedural generation
2. **Loading Models**: How Bevy loads and uses 3D assets
3. **Integrating Models**: Connecting models to your piece spawning system

## Three Approaches to Chess Piece Models

### Approach 1: Use Existing GLTF Models (Current Approach)

**Best for**: Quick setup, professional-looking pieces

Your current system loads individual meshes from a GLTF file:
```rust
let king_mesh = asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0");
```

**Where to find models**:
- [Sketchfab](https://sketchfab.com) - Search "chess pieces gltf"
- [Poly Haven](https://polyhaven.com/models) - Free 3D models
- [TurboSquid](https://www.turbosquid.com) - Paid models
- [OpenGameArt](https://opengameart.org) - Free game assets

**Model Requirements**:
- Format: `.glb` (binary GLTF) or `.gltf` (text GLTF)
- Scale: Pieces should fit within ~1x1x1 units (you scale to 0.2 in code)
- Organization: Each piece type should be a separate mesh (or use scene graphs)

### Approach 2: Create Manual Meshes (Programmatic)

**Best for**: Simple geometric pieces, learning Bevy mesh API

**Reference**: `reference/bevy/examples/3d/generate_custom_mesh.rs`

Create meshes programmatically using Bevy's `Mesh` API:

```rust
use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;

fn create_pawn_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        // Define vertices: [x, y, z] coordinates
        vec![
            // Base (bottom)
            [-0.3, 0.0, -0.3], [0.3, 0.0, -0.3], [0.3, 0.0, 0.3], [-0.3, 0.0, 0.3],
            // Top of base
            [-0.25, 0.2, -0.25], [0.25, 0.2, -0.25], [0.25, 0.2, 0.25], [-0.25, 0.2, 0.25],
            // Body cylinder top
            [-0.15, 0.5, -0.15], [0.15, 0.5, -0.15], [0.15, 0.5, 0.15], [-0.15, 0.5, 0.15],
            // Head sphere (simplified as cube for now)
            [-0.1, 0.7, -0.1], [0.1, 0.7, -0.1], [0.1, 0.7, 0.1], [-0.1, 0.7, 0.1],
        ],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        // Normals for lighting (simplified)
        vec![
            [0.0, -1.0, 0.0]; 8, // Bottom faces
            [0.0, 1.0, 0.0]; 8,  // Top faces
        ],
    )
    .with_inserted_indices(
        Indices::U32(vec![
            // Bottom faces
            0, 1, 2, 0, 2, 3,
            // Side faces
            0, 4, 1, 1, 4, 5,
            1, 5, 2, 2, 5, 6,
            // ... (add all triangle indices)
        ])
    )
}
```

**Pros**:
- No external files needed
- Full control over geometry
- Good for simple shapes (pawns, rooks)

**Cons**:
- Complex shapes (knights, bishops) are difficult
- No textures/materials in mesh definition
- More code to maintain

### Approach 3: Use Primitive Shapes (Quick Prototype)

**Best for**: Rapid prototyping, simple pieces

**Reference**: `reference/bevy/examples/3d/3d_shapes.rs`

Use Bevy's built-in primitive shapes:

```rust
use bevy::prelude::*;

fn spawn_pawn_primitive(
    commands: &mut Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    position: (u8, u8),
) {
    // Combine multiple primitives to make a pawn
    let base = meshes.add(Cylinder::new(0.3, 0.1));
    let body = meshes.add(Cylinder::new(0.2, 0.3));
    let head = meshes.add(Sphere::new(0.15));
    
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });
    
    commands.spawn((
        Transform::from_translation(Vec3::new(position.0 as f32, 0.0, position.1 as f32)),
        // Spawn base
        Mesh3d(base),
        MeshMaterial3d(material.clone()),
        // Use children for multi-part pieces
    )).with_children(|parent| {
        // Body on top of base
        parent.spawn((
            Mesh3d(body),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::new(0.0, 0.2, 0.0)),
        ));
        // Head on top of body
        parent.spawn((
            Mesh3d(head),
            MeshMaterial3d(material),
            Transform::from_translation(Vec3::new(0.0, 0.5, 0.0)),
        ));
    });
}
```

**Available Primitives**:
- `Cuboid` - Box/rectangular prism
- `Sphere` - Ball
- `Cylinder` - Tube
- `Cone` - Cone
- `Torus` - Donut
- `Capsule3d` - Pill shape
- `Tetrahedron` - 4-sided pyramid

**Pros**:
- Very fast to implement
- No external assets
- Good for testing

**Cons**:
- Limited to simple shapes
- Doesn't look like real chess pieces
- Difficult to make complex pieces (knights)

## Recommended Workflow

### Step 1: Choose Your Modeling Tool

**Free Options**:
1. **Blender** (Recommended) - Full-featured, exports GLTF
   - Download: https://www.blender.org
   - Tutorial: Search "Blender chess pieces tutorial"
   - Export: File → Export → glTF 2.0 (.glb)

2. **Tinkercad** - Web-based, simple
   - Website: https://www.tinkercad.com
   - Good for: Simple geometric pieces
   - Export: Export as .obj, convert to .glb

3. **Procedural Generation** - Code-based
   - Use Bevy primitives or mesh API
   - Good for: Prototyping, simple pieces

**Paid Options**:
- **Maya/3ds Max** - Professional tools
- **Cinema 4D** - Motion graphics focus

### Step 2: Create Your Models

**Chess Piece Checklist**:
- [ ] All 6 piece types (King, Queen, Bishop, Knight, Rook, Pawn)
- [ ] Consistent scale across pieces
- [ ] Pieces fit within ~1 unit (height: 0.8-1.2 units)
- [ ] Base is flat (for board placement)
- [ ] Clean geometry (no duplicate vertices)
- [ ] Proper normals (for lighting)

**Blender Workflow**:
1. Create each piece as a separate object
2. Model pieces to scale (e.g., pawn height = 1 unit)
3. Apply materials (white/black will be set in Bevy)
4. Export as GLTF:
   - File → Export → glTF 2.0
   - Format: glTF Binary (.glb)
   - Include: Selected Objects (or All)
   - Transform: +Y Up
   - Geometry: Apply Modifiers

### Step 3: Organize Your GLTF File

**Option A: Single File with Multiple Meshes** (Current approach)
```
pieces.glb
├── Mesh0 (King base)
├── Mesh1 (King cross)
├── Mesh2 (Pawn)
├── Mesh3 (Knight part 1)
├── Mesh4 (Knight part 2)
├── Mesh5 (Rook)
├── Mesh6 (Bishop)
└── Mesh7 (Queen)
```

Load with: `asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0")`

**Option B: Separate Files per Piece**
```
assets/models/chess_kit/
├── king.glb
├── queen.glb
├── bishop.glb
├── knight.glb
├── rook.glb
└── pawn.glb
```

Load with: `asset_server.load("models/chess_kit/king.glb#Mesh0/Primitive0")`

**Option C: Scene Graph** (For complex pieces)
```
pieces.glb
└── Scene0
    ├── King (parent)
    │   ├── Base (mesh)
    │   └── Cross (mesh)
    ├── Queen (mesh)
    └── ...
```

Load with: `asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/chess_kit/pieces.glb"))`

### Step 4: Integrate into Your Code

**Current System** (`src/rendering/pieces.rs`):

```rust
// 1. Load meshes in create_pieces()
let piece_meshes = PieceMeshes {
    king: asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0"),
    king_cross: asset_server.load("models/chess_kit/pieces.glb#Mesh1/Primitive0"),
    // ... etc
};

// 2. Use in spawn functions
fn spawn_king(...) {
    commands.spawn(...)
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(...),
            ));
        });
}
```

**To Use New Models**:
1. Place `.glb` file in `assets/models/chess_kit/`
2. Update mesh paths in `PieceMeshes` struct
3. Adjust `piece_transform()` scale/position if needed
4. Test in game

### Step 5: Materials and Textures

**Current**: Simple color materials
```rust
let white_material = materials.add(StandardMaterial {
    base_color: Color::WHITE,
    ..default()
});
```

**Enhanced**: Add textures, normal maps, etc.
```rust
let white_material = materials.add(StandardMaterial {
    base_color: Color::WHITE,
    base_color_texture: Some(asset_server.load("textures/wood_white.png")),
    normal_map_texture: Some(asset_server.load("textures/wood_normal.png")),
    metallic: 0.1,
    perceptual_roughness: 0.7,
    ..default()
});
```

**Texture Options**:
- Diffuse/Albedo: Base color
- Normal Map: Surface detail
- Roughness Map: Shiny vs matte
- Metallic Map: Metal vs non-metal
- Emission Map: Glowing effects

## Testing Your Models

1. **Load Test**: Ensure models load without errors
2. **Scale Test**: Verify pieces are appropriately sized
3. **Position Test**: Check pieces align with board squares
4. **Material Test**: Verify white/black materials work
5. **Performance Test**: Check FPS with 32 pieces spawned

## Troubleshooting

**Model too large/small**:
- Adjust `piece_transform()` scale: `t.scale = Vec3::splat(0.2);`
- Or scale in Blender before export

**Model appears black**:
- Check normals are facing outward
- Verify lighting is set up
- Check material base_color

**Model doesn't load**:
- Verify file path is correct
- Check GLTF file is valid (test in glTF viewer)
- Ensure mesh index is correct (`#Mesh0/Primitive0`)

**Model is flipped/rotated wrong**:
- Bevy uses +Y up, ensure Blender export matches
- Adjust Transform rotation in spawn function

## Reference Examples

From `reference/bevy/examples/`:
- `3d/3d_shapes.rs` - Primitive shapes
- `3d/generate_custom_mesh.rs` - Manual mesh creation
- `asset/asset_loading.rs` - GLTF loading patterns
- `gltf/load_gltf.rs` - GLTF scene loading
- `gltf/update_gltf_scene.rs` - Manipulating GLTF scenes

## Next Steps

1. **Start Simple**: Use primitives or find free GLTF models
2. **Learn Blender**: Watch tutorials on modeling chess pieces
3. **Create Models**: Model all 6 piece types
4. **Export & Test**: Export as GLTF and test in Bevy
5. **Iterate**: Refine models based on in-game appearance
6. **Enhance**: Add textures, animations, or particle effects

## Resources

- **Bevy Mesh Documentation**: https://docs.rs/bevy_mesh
- **GLTF Specification**: https://www.khronos.org/gltf/
- **Blender GLTF Export**: https://docs.blender.org/manual/en/latest/addons/io_scene_gltf2.html
- **Bevy Examples**: `reference/bevy/examples/3d/` and `reference/bevy/examples/gltf/`

---

**Current Implementation**: Your game uses Approach 1 (GLTF models) with a single file containing multiple meshes. This is a solid approach that balances quality and simplicity.

