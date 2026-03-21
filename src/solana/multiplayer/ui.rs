use crate::multiplayer::solana_addon::{SolanaGameSync, SolanaResult, SolanaWallet};
use crate::solana::instructions::{create_game_ix, GameType};
use bevy::tasks::IoTaskPool;
use bevy::{prelude::*, ui::Interaction};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};
use std::sync::Arc;

#[derive(Component)]
pub enum SolanaUiButton {
    ConnectWallet,
    InitializeGame,
    JoinGame,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Resource)]
pub enum SolanaUiState {
    #[default]
    Disconnected,
    Connected,
    InGame,
}

pub fn setup_solana_ui(mut commands: Commands) {
    commands.init_resource::<SolanaUiState>();
}

pub fn update_wallet_connection(
    mut wallet: ResMut<SolanaWallet>,
    mut ui_state: ResMut<SolanaUiState>,
    mut interaction_query: Query<
        (&Interaction, &SolanaUiButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, button_type) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            match button_type {
                SolanaUiButton::ConnectWallet => {
                    info!("[UI] Connecting local wallet...");
                    let keypair = Arc::new(Keypair::new());
                    wallet.pubkey = Some(keypair.pubkey());
                    wallet.keypair = Some(keypair);
                    *ui_state = SolanaUiState::Connected;
                }
                _ => {}
            }
        }
    }
}

pub fn handle_game_interactions(
    wallet: Res<SolanaWallet>,
    mut sync: ResMut<SolanaGameSync>,
    mut interaction_query: Query<
        (&Interaction, &SolanaUiButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    if wallet.pubkey.is_none() {
        return;
    }

    for (interaction, button_type) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            match button_type {
                SolanaUiButton::InitializeGame => {
                    let game_id = rand::random::<u64>();
                    info!("[UI] Initializing game {} on-chain...", game_id);

                    let rpc_url = sync.rpc_url.clone();
                    let program_id: Pubkey =
                        solana_chess_client::XFCHESS_PROGRAM_ID.parse().unwrap();
                    let player = wallet.pubkey.unwrap();
                    let tx_sender = sync.result_tx.clone().unwrap();
                    let signer_opt = wallet.keypair.clone();

                    if let Some(signer) = signer_opt {
                        IoTaskPool::get()
                            .spawn(async move {
                                let rpc = RpcClient::new(rpc_url);
                                let ix = match create_game_ix(program_id, player, game_id, 0, GameType::PvP) {
                                    Ok(ix) => ix,
                                    Err(e) => {
                                        let _ = tx_sender.send(SolanaResult::Error(e.to_string()));
                                        return;
                                    }
                                };

                                let mut tx = Transaction::new_with_payer(&[ix], Some(&player));

                                if let Ok(recent_blockhash) = rpc.get_latest_blockhash() {
                                    tx.sign(&[&signer], recent_blockhash);
                                    match rpc.send_and_confirm_transaction(&tx) {
                                        Ok(_) => {
                                            info!("Game {} initialized!", game_id);
                                            // We'd send a result here to tell Bevy to set sync.game_id
                                        }
                                        Err(e) => {
                                            let _ =
                                                tx_sender.send(SolanaResult::Error(e.to_string()));
                                        }
                                    }
                                }
                            })
                            .detach();

                        sync.game_id = Some(game_id);
                        sync.moves_submitted = 0;
                    }
                }
                _ => {}
            }
        }
    }
}

pub fn render_solana_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    wallet: Res<SolanaWallet>,
    ui_state: Res<SolanaUiState>,
) {
    // UI rendering logic...
}

// System set for Solana UI
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct SolanaUiSystemSet;
