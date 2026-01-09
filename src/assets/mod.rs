//! Asset management module
//!
//! Handles preloading and tracking of all game assets including:
//! - Chess piece GLTF models
//! - Board materials and textures
//! - UI fonts and images
//! - Sound effects (future)
//!
//! Provides a centralized resource for asset handles and loading progress.

use bevy::asset::AssetLoadFailedEvent;
use bevy::ecs::message::MessageReader;
use bevy::gltf::Gltf;
use bevy::prelude::*;

/// Resource storing handles to all preloaded assets
///
/// This ensures assets stay loaded in memory and provides
/// quick access without repeated AssetServer queries.
#[derive(Resource, Default)]
pub struct GameAssets {
    /// Chess pieces GLTF file
    pub pieces_gltf: Handle<Gltf>,

    /// Individual piece mesh handles (loaded from GLTF)
    pub piece_meshes: PieceMeshes,

    /// Whether all assets have finished loading
    pub loaded: bool,

    /// Whether asset loading has been started
    pub loading_started: bool,

    /// Whether asset loading has failed
    pub failed: bool,

    /// Error message if asset loading failed
    pub error_message: Option<String>,
}

/// Handles to individual piece meshes
#[derive(Default)]
pub struct PieceMeshes {
    pub king: Option<Handle<Mesh>>,
    pub queen: Option<Handle<Mesh>>,
    pub rook: Option<Handle<Mesh>>,
    pub bishop: Option<Handle<Mesh>>,
    pub knight: Option<Handle<Mesh>>,
    pub pawn: Option<Handle<Mesh>>,
}

/// Resource tracking asset loading progress (0.0 to 1.0)
#[derive(Resource, Default)]
pub struct LoadingProgress {
    /// Current loading progress (0.0 = none, 1.0 = complete)
    pub progress: f32,

    /// Total number of assets to load
    pub total_assets: usize,

    /// Number of assets loaded so far
    pub loaded_assets: usize,

    /// Whether loading is complete
    pub complete: bool,

    /// Whether loading has failed
    pub failed: bool,

    /// Error message if loading failed
    pub error_message: Option<String>,
}

impl LoadingProgress {
    pub fn new(total_assets: usize) -> Self {
        Self {
            progress: 0.0,
            total_assets,
            loaded_assets: 0,
            complete: false,
            failed: false,
            error_message: None,
        }
    }

    pub fn increment(&mut self) {
        self.loaded_assets += 1;
        self.update_progress();
    }

    fn update_progress(&mut self) {
        if self.total_assets > 0 {
            self.progress = self.loaded_assets as f32 / self.total_assets as f32;
            self.complete = self.loaded_assets >= self.total_assets;
        }
    }

    pub fn percentage(&self) -> u32 {
        (self.progress * 100.0) as u32
    }
}

/// System to initiate asset loading
///
/// Called when entering MainMenu state. Starts loading all
/// assets and initializes the progress tracker.
pub fn start_asset_loading(
    mut game_assets: ResMut<GameAssets>,
    mut progress: ResMut<LoadingProgress>,
    asset_server: Res<AssetServer>,
) {
    // Only start loading if not already loaded or started
    if game_assets.loaded || game_assets.loading_started {
        return;
    }

    // Load the main chess pieces GLTF
    let pieces_gltf = asset_server.load::<Gltf>("models/chess_kit/pieces.glb");

    // Update GameAssets resource
    game_assets.pieces_gltf = pieces_gltf;
    game_assets.loaded = false;
    game_assets.loading_started = true;

    // Initialize loading progress (1 asset: the GLTF file)
    // We'll track the GLTF as a single asset for simplicity
    *progress = LoadingProgress::new(1);
}

/// Resource to track asset loading start time for timeout detection
/// Note: This struct is kept for backward compatibility but the actual
/// timeout tracking now uses Bevy's Time resource for WASM compatibility.
#[derive(Resource, Default)]
pub struct AssetLoadingTimer {
    /// Start time in Bevy elapsed seconds (WASM-compatible)
    start_elapsed_secs: Option<f32>,
}

impl AssetLoadingTimer {
    pub fn start_with_time(&mut self, elapsed_secs: f32) {
        self.start_elapsed_secs = Some(elapsed_secs);
    }

    pub fn elapsed_secs_with_time(&self, current_elapsed_secs: f32) -> Option<f32> {
        self.start_elapsed_secs
            .map(|start| current_elapsed_secs - start)
    }
}

/// System to check asset loading status
///
/// Polls the AssetServer to determine when assets are fully loaded.
/// Updates LoadingProgress resource accordingly.
/// Also checks for timeout and load state to detect failures.
pub fn check_asset_loading(
    mut game_assets: ResMut<GameAssets>,
    mut progress: ResMut<LoadingProgress>,
    asset_server: Res<AssetServer>,
    gltf_assets: Res<Assets<Gltf>>,
    time: Res<Time>,
    mut loading_start_time: Local<Option<f32>>,
) {
    // Don't check if already loaded or failed
    if game_assets.loaded || game_assets.failed {
        return;
    }

    // Don't check if loading hasn't started yet
    if !game_assets.loading_started {
        return;
    }

    let elapsed_secs = time.elapsed_secs();

    // Initialize timer on first check
    if loading_start_time.is_none() {
        *loading_start_time = Some(elapsed_secs);
    }

    // Check for timeout (30 seconds)
    const TIMEOUT_SECS: f32 = 30.0;
    if let Some(start_time) = *loading_start_time {
        if elapsed_secs - start_time > TIMEOUT_SECS {
            let error_msg = format!("Asset loading timeout after {} seconds", TIMEOUT_SECS);
            error!("[ASSETS] {}", error_msg);

            game_assets.failed = true;
            game_assets.error_message = Some(error_msg.clone());
            progress.failed = true;
            progress.error_message = Some(error_msg);
            return;
        }
    }

    // Check asset load state using AssetServer
    let load_state = asset_server.load_state(&game_assets.pieces_gltf);
    match load_state {
        bevy::asset::LoadState::Loaded => {
            // Asset is loaded - verify it exists in Assets collection
            if gltf_assets.get(&game_assets.pieces_gltf).is_some() {
                if !game_assets.loaded {
                    game_assets.loaded = true;
                    progress.increment();

                    // Note: Individual mesh extraction from GLTF will happen
                    // in the piece spawning system (rendering/pieces.rs)
                }
            }
        }
        bevy::asset::LoadState::Failed(_) => {
            // Asset loading failed
            let error_msg = format!("Failed to load asset: models/chess_kit/pieces.glb");
            error!("[ASSETS] {}", error_msg);

            game_assets.failed = true;
            game_assets.error_message = Some(error_msg.clone());
            progress.failed = true;
            progress.error_message = Some(error_msg);
        }
        bevy::asset::LoadState::NotLoaded | bevy::asset::LoadState::Loading => {
            // Still loading - this is normal, just continue waiting
        }
    }
}

/// System to handle asset loading failures via events (backup method)
///
/// Listens to AssetLoadFailedEvent as a backup to load state checking.
/// This provides additional error information if available.
pub fn handle_asset_loading_errors(
    mut game_assets: ResMut<GameAssets>,
    mut progress: ResMut<LoadingProgress>,
    mut failed_events: MessageReader<AssetLoadFailedEvent<Gltf>>,
) {
    // Only process if not already marked as failed (to avoid duplicate errors)
    if game_assets.failed {
        return;
    }

    for event in failed_events.read() {
        // Check if this is our pieces GLTF asset
        if event.id == game_assets.pieces_gltf.id() {
            let error_msg = format!("Failed to load asset: {}", event.path.path().display());
            error!("[ASSETS] Asset loading failed (via event): {}", error_msg);
            error!("[ASSETS] Error details: {:?}", event.error);

            game_assets.failed = true;
            game_assets.error_message = Some(error_msg.clone());
            progress.failed = true;
            progress.error_message = Some(error_msg);

            warn!("[ASSETS] Asset loading failed. Game may not function correctly without assets.");
        }
    }
}

/// System to handle generic asset loading failures (for untyped assets)
///
/// Listens to UntypedAssetLoadFailedEvent as a backup method to catch failures.
pub fn handle_untyped_asset_loading_errors(
    mut game_assets: ResMut<GameAssets>,
    mut progress: ResMut<LoadingProgress>,
    mut failed_events: MessageReader<bevy::asset::UntypedAssetLoadFailedEvent>,
) {
    // Only process if not already marked as failed (to avoid duplicate errors)
    if game_assets.failed {
        return;
    }

    for event in failed_events.read() {
        // Check if this is our pieces GLTF asset by comparing paths
        // Note: We can't directly compare handles, so we check if the path matches
        // Convert path to string for comparison
        let path_str = event.path.path().to_string_lossy().to_lowercase();
        if path_str.contains("pieces.glb") || path_str.contains("chess_kit") {
            let error_msg = format!(
                "Failed to load asset {}: {:?}",
                event.path.path().display(),
                event.error
            );
            error!(
                "[ASSETS] Asset loading failed (via untyped event): {}",
                error_msg
            );

            game_assets.failed = true;
            game_assets.error_message = Some(error_msg.clone());
            progress.failed = true;
            progress.error_message = Some(error_msg);

            warn!("[ASSETS] Asset loading failed. Game may not function correctly without assets.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_progress_new() {
        let progress = LoadingProgress::new(10);
        assert_eq!(progress.total_assets, 10);
        assert_eq!(progress.loaded_assets, 0);
        assert_eq!(progress.progress, 0.0);
        assert!(!progress.complete);
    }

    #[test]
    fn test_loading_progress_increment() {
        let mut progress = LoadingProgress::new(4);

        progress.increment();
        assert_eq!(progress.loaded_assets, 1);
        assert_eq!(progress.progress, 0.25);
        assert!(!progress.complete);

        progress.increment();
        assert_eq!(progress.loaded_assets, 2);
        assert_eq!(progress.progress, 0.5);
        assert!(!progress.complete);

        progress.increment();
        progress.increment();
        assert_eq!(progress.loaded_assets, 4);
        assert_eq!(progress.progress, 1.0);
        assert!(progress.complete);
    }

    #[test]
    fn test_loading_progress_percentage() {
        let mut progress = LoadingProgress::new(2);
        assert_eq!(progress.percentage(), 0);

        progress.increment();
        assert_eq!(progress.percentage(), 50);

        progress.increment();
        assert_eq!(progress.percentage(), 100);
    }
}
