use super::*;

pub fn spawn_swiss_subscription(
    tournament_id: u64,
    round_tx: mpsc::UnboundedSender<SwissRoundStartedEvent>,
    result_tx: mpsc::UnboundedSender<SwissResultRecordedEvent>,
    standings_tx: mpsc::UnboundedSender<SwissStandingsUpdatedEvent>,
    bracket_fired_tx: crossbeam_channel::Sender<BracketFiredEvent>,
) {
    crate::core::net_task::spawn_detached("tournament", move || {
        futures_lite::future::block_on(async move {
            use braid_chess::{MatchResult, SwissMessage};
            use braid_iroh::BraidIrohNode;
            use braid_iroh::DiscoveryConfig;

            let vps_url = crate::multiplayer::network::vps::vps_base();

            // Get Iroh node ID from VPS
            let client = reqwest::Client::new();
            let node_id_res = client.get(format!("{}/iroh/node-id", vps_url)).send().await;

            let _node_id = match node_id_res {
                Ok(resp) if resp.status().is_success() => match resp.text().await {
                    Ok(text) => text,
                    Err(e) => {
                        error!("Failed to read Iroh node ID response: {}", e);
                        return;
                    }
                },
                Ok(resp) => {
                    error!(
                        "Failed to get Iroh node ID from VPS: HTTP {}",
                        resp.status()
                    );
                    return;
                }
                Err(e) => {
                    error!("Failed to get Iroh node ID from VPS: {}", e);
                    return;
                }
            };

            use braid_iroh::BraidIrohConfig;
            let node = match BraidIrohNode::spawn(BraidIrohConfig {
                discovery: DiscoveryConfig::Real,
                secret_key: None,
                proxy_config: None,
                data_dir: None,
            })
            .await
            {
                Ok(node) => node,
                Err(e) => {
                    error!("Failed to spawn Iroh node: {}", e);
                    return;
                }
            };

            let topic = format!("/swiss/{}", tournament_id);
            let mut rx = match node.subscribe(&topic, vec![]).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to subscribe to Swiss topic {}: {}", topic, e);
                    return;
                }
            };

            info!("[Swiss] Subscribed to topic {}", topic);

            use futures::StreamExt;
            use iroh_gossip::api::Event as GossipEvent;

            while let Some(Ok(event)) = rx.next().await {
                let payload = match event {
                    GossipEvent::Received(message) => message.content,
                    _ => continue,
                };
                // This topic includes every tournament resource; from_update ignores unhandled roster/meta updates.
                let Ok(update) = serde_json::from_slice::<braid_http::Update>(&payload) else {
                    continue;
                };
                let Some(swiss_msg) = SwissMessage::from_update(&update) else {
                    continue;
                };
                match swiss_msg {
                    SwissMessage::RoundStarted {
                        round, pairings, ..
                    } => {
                        let _ = round_tx.send(SwissRoundStartedEvent {
                            tournament_id,
                            round,
                            pairings: pairings
                                .iter()
                                .map(|p| (p.white.clone(), p.black.clone()))
                                .collect(),
                        });
                    }
                    SwissMessage::ResultRecorded {
                        round,
                        board,
                        result,
                        ..
                    } => {
                        // ResultRecorded carries the outcome; resolve player IDs from the round’s pairing.
                        let (white, black) = match &result {
                            MatchResult::Win { winner } => (winner.clone(), String::new()),
                            MatchResult::Draw => (String::new(), String::new()),
                        };
                        let _ = result_tx.send(SwissResultRecordedEvent {
                            tournament_id,
                            round,
                            board,
                            white,
                            black,
                            result: result.to_string(),
                        });
                    }
                    SwissMessage::StandingsUpdated { standings, .. } => {
                        let _ = standings_tx.send(SwissStandingsUpdatedEvent {
                            tournament_id,
                            standings: standings
                                .iter()
                                .map(|s| (s.player_id.clone(), s.score, s.rank))
                                .collect(),
                        });
                    }
                    SwissMessage::BracketFired {
                        player_count,
                        started_at,
                        ..
                    } => {
                        let _ = bracket_fired_tx.send(BracketFiredEvent {
                            tournament_id,
                            player_count,
                            started_at,
                        });
                    }
                }
            }
        })
    });
}
