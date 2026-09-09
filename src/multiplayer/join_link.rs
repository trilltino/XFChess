use crate::multiplayer::spectator::{parse_spectate_link, SpectateViaLinkEvent};
use crate::multiplayer::traits::{Message, MessageReader, MessageWriter};
use bevy::prelude::*;

pub fn make_join_link(game_id: &str, host_node_id_b58: &str) -> String {
    format!("xfchess://join/{}/{}", game_id, host_node_id_b58)
}

pub fn parse_join_link(url: &str) -> Option<(String, String)> {
    let path = url.strip_prefix("xfchess://join/")?;
    let mut parts = path.splitn(2, '/');
    let game_id = parts.next()?.to_string();
    let node_id = parts.next()?.to_string();
    if game_id.is_empty() || node_id.is_empty() {
        return None;
    }
    Some((game_id, node_id))
}

#[derive(Message, Debug, Clone)]
pub struct JoinViaLinkEvent {
    pub game_id: String,
    pub host_node_id: String,
}

pub struct JoinLinkPlugin;

impl Plugin for JoinLinkPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<JoinViaLinkEvent>();
        app.add_systems(Update, handle_join_via_link);
    }
}

fn handle_join_via_link(
    mut link_events: MessageReader<JoinViaLinkEvent>,
    mut connect_events: MessageWriter<crate::multiplayer::network::p2p::ConnectToPeerEvent>,
) {
    for event in link_events.read() {
        tracing::info!(
            "[join-link] Joining game {} via host {}",
            event.game_id,
            event.host_node_id
        );
        connect_events.write(crate::multiplayer::network::p2p::ConnectToPeerEvent {
            peer_node_id: event.host_node_id.clone(),
            is_host: false,
            // A join link has no VPS lobby entry behind it — this handshake is
            // the only signaling channel that will ever start the match.
            drive_game_start: true,
        });
    }
}

pub fn dispatch_deep_link(
    url: &str,
    join_events: &mut impl FnMut(JoinViaLinkEvent),
    spectate_events: &mut impl FnMut(SpectateViaLinkEvent),
) {
    if let Some((game_id, node_id)) = parse_join_link(url) {
        join_events(JoinViaLinkEvent {
            game_id,
            host_node_id: node_id,
        });
    } else if let Some(game_id) = parse_spectate_link(url) {
        spectate_events(SpectateViaLinkEvent {
            game_id,
            details: None,
            // A bare deep link carries no tournament context.
            tournament_id: None,
            playlist: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let link = make_join_link("12345678", "NodeABCxyz");
        let (gid, nid) = parse_join_link(&link).unwrap();
        assert_eq!(gid, "12345678");
        assert_eq!(nid, "NodeABCxyz");
    }
}
