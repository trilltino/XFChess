//! Keep startup covered until the menu scene has reached the GPU and its
//! shader pipelines have finished compiling.

use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

use bevy::pbr::PreparedMaterial;
use bevy::prelude::*;
use bevy::render::{
    erased_render_asset::ErasedRenderAssets,
    mesh::RenderMesh,
    render_asset::RenderAssets,
    render_resource::{CachedPipelineState, PipelineCache},
    Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
};
use bevy::shader::ShaderCacheError;

use crate::assets::LoadingProgress;
use crate::states::main_menu::menu::MenuBg;

const WAITING: u8 = 0;
const READY: u8 = 1;
const FAILED: u8 = 2;

/// Shared by the main and render worlds, including pipelined rendering.
#[derive(Resource, Clone, Default)]
pub struct StartupRenderStatus(Arc<AtomicU8>);

impl StartupRenderStatus {
    pub fn is_ready(&self) -> bool {
        self.0.load(Ordering::Acquire) == READY
    }
}

#[derive(Resource, Default)]
struct StartupScene {
    assets_loaded: bool,
    meshes: Vec<AssetId<Mesh>>,
    materials: Vec<AssetId<StandardMaterial>>,
}

fn pipeline_ready(state: &CachedPipelineState) -> Result<bool, &ShaderCacheError> {
    match state {
        CachedPipelineState::Ok(_) => Ok(true),
        // Bevy retries these states as shader assets and imports arrive. They
        // are part of normal startup, even though the cache calls them Err.
        CachedPipelineState::Err(
            ShaderCacheError::ShaderNotLoaded(_) | ShaderCacheError::ShaderImportNotYetAvailable,
        ) => Ok(false),
        CachedPipelineState::Err(error) => Err(error),
        _ => Ok(false),
    }
}

pub(super) fn register(app: &mut App) {
    let status = StartupRenderStatus::default();
    app.insert_resource(status.clone()).add_systems(
        Update,
        report_render_failure.in_set(crate::core::schedule::ApplySettings),
    );
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .insert_resource(status)
            .init_resource::<StartupScene>()
            .add_systems(ExtractSchedule, extract_startup_scene)
            .add_systems(Render, check_startup_render.in_set(RenderSystems::Cleanup));
    } else {
        // Headless apps have no GPU work to wait for.
        status.0.store(READY, Ordering::Release);
    }
}

fn extract_startup_scene(
    status: Res<StartupRenderStatus>,
    progress: Extract<Res<LoadingProgress>>,
    objects: Extract<Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>), With<MenuBg>>>,
    mut scene: ResMut<StartupScene>,
) {
    if status.0.load(Ordering::Acquire) != WAITING {
        return;
    }
    scene.assets_loaded = progress.complete;
    scene.meshes.clear();
    scene.materials.clear();
    for (mesh, material) in &objects {
        scene.meshes.push(mesh.0.id());
        scene.materials.push(material.0.id());
    }
}

fn check_startup_render(
    status: Res<StartupRenderStatus>,
    scene: Res<StartupScene>,
    meshes: Res<RenderAssets<RenderMesh>>,
    materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    pipelines: Res<PipelineCache>,
    mut stable_frames: Local<u8>,
    mut previous_pipeline_count: Local<usize>,
) {
    if status.0.load(Ordering::Acquire) != WAITING {
        return;
    }

    let mut pipeline_count = 0;
    let mut pipelines_ready = true;
    for pipeline in pipelines.pipelines() {
        pipeline_count += 1;
        match pipeline_ready(&pipeline.state) {
            Ok(ready) => pipelines_ready &= ready,
            Err(error) => {
                error!("[STARTUP] Render pipeline failed: {error}");
                status.0.store(FAILED, Ordering::Release);
                return;
            }
        }
    }

    // All 64 squares and 32 pieces must have GPU meshes and materials. Check
    // after rendering, so an empty pipeline cache cannot reveal a blank board.
    let ready = scene.assets_loaded
        && scene.meshes.len() == 96
        && scene.meshes.iter().all(|id| meshes.get(*id).is_some())
        && scene
            .materials
            .iter()
            .all(|id| materials.get(*id).is_some())
        && pipeline_count > 0
        && pipelines_ready
        && pipelines.waiting_pipelines().next().is_none();

    // Require consecutive completed render frames with no new pipelines;
    // follow-up passes may queue work after the first mesh is prepared.
    *stable_frames = if ready && pipeline_count == *previous_pipeline_count {
        stable_frames.saturating_add(1)
    } else {
        0
    };
    *previous_pipeline_count = pipeline_count;
    if *stable_frames >= 3 {
        info!("[STARTUP] Menu meshes, materials and render pipelines ready");
        status.0.store(READY, Ordering::Release);
    }
}

fn report_render_failure(status: Res<StartupRenderStatus>, mut progress: ResMut<LoadingProgress>) {
    if status.0.load(Ordering::Acquire) == FAILED && !progress.failed {
        progress.complete = false;
        progress.failed = true;
        progress.error_message =
            Some("Could not prepare the menu graphics. See the log for details.".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::shader::Shader;

    #[test]
    fn shader_assets_still_loading_keep_startup_waiting_instead_of_failing() {
        for state in [
            CachedPipelineState::Queued,
            CachedPipelineState::Err(ShaderCacheError::ShaderNotLoaded(
                Handle::<Shader>::default().id(),
            )),
            CachedPipelineState::Err(ShaderCacheError::ShaderImportNotYetAvailable),
        ] {
            assert!(matches!(pipeline_ready(&state), Ok(false)));
        }
    }

    #[test]
    fn invalid_shader_still_reports_a_failure() {
        for error in [
            ShaderCacheError::ProcessShaderError("invalid shader".into()),
            ShaderCacheError::CreateShaderModule("unsupported shader".into()),
        ] {
            assert!(pipeline_ready(&CachedPipelineState::Err(error)).is_err());
        }
    }
}
