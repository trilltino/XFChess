pub mod state;
pub mod systems;
pub mod plugin;
pub mod profile_check;
pub mod rpc;

pub use state::{SolanaIntegrationState, ProfileStatus, BalanceRefreshTimer, DEVNET_RPC_URL, MAGICBLOCK_EU_DEVNET};
pub use plugin::SolanaIntegrationPlugin;
pub use profile_check::{check_profile_on_connect, handle_profile_check_tasks, auto_init_profile};
pub use rpc::{initiate_game_on_chain, join_game_on_chain, prepare_final_game_state};
