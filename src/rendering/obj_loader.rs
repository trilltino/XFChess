use bevy::{
    asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext, RenderAssetUsages},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use thiserror::Error;

#[derive(Default)]
pub struct ObjLoader;

#[derive(Error, Debug)]
pub enum ObjError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

impl AssetLoader for ObjLoader {
    type Asset = Mesh;
    type Settings = ();
    type Error = ObjError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new(); // reader.read_to_end requires mut ref to reader
                                    // Wait, reader is &mut dyn Reader.
                                    // reader.read_to_end(&mut bytes) is correct.

        reader.read_to_end(&mut bytes).await?;
        let text = String::from_utf8(bytes)?;

        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();

        // Final flattened buffers
        let mut final_positions = Vec::new();
        let mut final_normals = Vec::new();
        let mut final_uvs = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                "v" => {
                    if parts.len() >= 4 {
                        let x = parts[1].parse::<f32>().unwrap_or(0.0);
                        let y = parts[2].parse::<f32>().unwrap_or(0.0);
                        let z = parts[3].parse::<f32>().unwrap_or(0.0);
                        positions.push(Vec3::new(x, y, z));
                    }
                }
                "vn" => {
                    if parts.len() >= 4 {
                        let x = parts[1].parse::<f32>().unwrap_or(0.0);
                        let y = parts[2].parse::<f32>().unwrap_or(0.0);
                        let z = parts[3].parse::<f32>().unwrap_or(0.0);
                        normals.push(Vec3::new(x, y, z));
                    }
                }
                "vt" => {
                    if parts.len() >= 3 {
                        let u = parts[1].parse::<f32>().unwrap_or(0.0);
                        let v = parts[2].parse::<f32>().unwrap_or(0.0);
                        // Flip V coordinate usually for OBJ? Bevy/WGPU might need it.
                        // Often OBJ V is mostly 0 at bottom.
                        uvs.push(Vec2::new(u, v));
                    }
                }
                "f" => {
                    let num_verts = parts.len() - 1;
                    if num_verts < 3 {
                        continue;
                    }

                    let mut face_verts = Vec::new();
                    for i in 1..=num_verts {
                        let s = parts[i];
                        let subparts: Vec<&str> = s.split('/').collect();

                        let v_idx = subparts[0].parse::<i32>().unwrap_or(1);
                        let p_idx = if v_idx < 0 {
                            positions.len() as i32 + v_idx
                        } else {
                            v_idx - 1
                        } as usize;

                        let mut t_idx = None;
                        if subparts.len() > 1 && !subparts[1].is_empty() {
                            let idx = subparts[1].parse::<i32>().unwrap_or(1);
                            t_idx = Some(if idx < 0 {
                                uvs.len() as i32 + idx
                            } else {
                                idx - 1
                            } as usize);
                        }

                        let mut n_idx = None;
                        if subparts.len() > 2 && !subparts[2].is_empty() {
                            let idx = subparts[2].parse::<i32>().unwrap_or(1);
                            n_idx = Some(if idx < 0 {
                                normals.len() as i32 + idx
                            } else {
                                idx - 1
                            } as usize);
                        }

                        face_verts.push((p_idx, t_idx, n_idx));
                    }

                    // Triangulate fan
                    for i in 1..num_verts - 1 {
                        let idxs = [0, i, i + 1];
                        for &k in &idxs {
                            let (p, t, n) = face_verts[k];
                            if p < positions.len() {
                                final_positions.push(positions[p]);
                            } else {
                                final_positions.push(Vec3::ZERO);
                            }

                            if let Some(t_i) = t {
                                if t_i < uvs.len() {
                                    final_uvs.push(uvs[t_i]);
                                } else {
                                    final_uvs.push(Vec2::ZERO);
                                }
                            } else {
                                final_uvs.push(Vec2::ZERO);
                            }

                            if let Some(n_i) = n {
                                if n_i < normals.len() {
                                    final_normals.push(normals[n_i]);
                                } else {
                                    final_normals.push(Vec3::Y);
                                }
                            } else {
                                final_normals.push(Vec3::Y);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, final_positions);

        if !final_normals.is_empty() && final_normals.len() == mesh.count_vertices() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, final_normals);
        } else {
            mesh.compute_flat_normals();
        }

        if !final_uvs.is_empty() && final_uvs.len() == mesh.count_vertices() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, final_uvs);
        }

        Ok(mesh)
    }

    fn extensions(&self) -> &[&str] {
        &["obj"]
    }
}
