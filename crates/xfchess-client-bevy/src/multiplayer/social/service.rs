use super::*;
use crate::core::net_task::{Pending, Poll};
use crate::multiplayer::network::{
    profile_identity::ProfileIdentity,
    vps::{
        profile_social::ProfileSocialClient, LobbyInvite, SocialBlocked, SocialContact,
        SocialFriendRequest,
    },
};
use std::collections::VecDeque;

#[derive(Clone)]
pub enum FriendAction {
    Send(String),
    SendName(String),
    Respond(String, bool),
    Cancel(String),
    Remove(String),
    Invite {
        game: String,
        target: String,
        direct: bool,
    },
    DismissInvite(String),
    Block(String),
    Unblock(String),
}

#[derive(Default)]
struct Snapshot {
    contacts: Vec<SocialContact>,
    requests: Vec<SocialFriendRequest>,
    outgoing: Vec<SocialFriendRequest>,
    invites: Vec<LobbyInvite>,
    blocked: Vec<SocialBlocked>,
}

#[derive(Resource, Default)]
pub struct FriendsState {
    pub contacts: Vec<SocialContact>,
    pub pending_requests: Vec<SocialFriendRequest>,
    pub outgoing_requests: Vec<SocialFriendRequest>,
    pub pending_invites: Vec<LobbyInvite>,
    pub blocked: Vec<SocialBlocked>,
    pub our_node_id: Option<String>,
    pub our_pubkey: Option<String>,
    pub our_display: String,
    pub profile: Option<ProfileIdentity>,
    pub loading: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub last_sync: Option<Instant>,
    actions: VecDeque<FriendAction>,
    fetch: Option<Pending<Result<Snapshot, String>>>,
    mutation: Option<Pending<Result<String, String>>>,
    dirty: bool,
}

impl FriendsState {
    pub fn enqueue(&mut self, action: FriendAction) {
        if self.profile.is_none() {
            self.error = Some("Select a local profile first".into());
            return;
        }
        // One explicit write at a time; prevent duplicate clicks and unbounded queues.
        if self.busy || !self.actions.is_empty() {
            return;
        }
        self.actions.push_back(action);
        self.busy = true;
        self.error = None;
        self.notice = None;
    }
    pub fn refresh(&mut self) {
        self.dirty = true;
    }
    pub fn client(&self) -> Result<ProfileSocialClient, String> {
        ProfileSocialClient::new(self.profile.clone().ok_or("Select a local profile first")?)
            .map(|client| client.with_wallet(self.our_pubkey.clone()))
    }
    pub fn badge(&self) -> usize {
        self.pending_requests.len() + self.pending_invites.len()
    }
}

pub(super) fn sync_identity(
    identity: Res<crate::states::main_menu::PlayerIdentity>,
    network: Option<Res<crate::multiplayer::OnlineNetworkState>>,
    mut friends: ResMut<FriendsState>,
    mut ui: ResMut<super::ui::FriendsUiState>,
    mut previous: Local<Option<(Option<String>, bool)>>,
) {
    let node = network.and_then(|state| {
        state
            .node_id
            .map(|id| bs58::encode(id.as_bytes()).into_string())
    });
    if friends.our_node_id != node {
        friends.our_node_id = node;
    }
    if friends.our_pubkey != identity.pubkey_str {
        friends.our_pubkey = identity.pubkey_str.clone();
    }
    if friends.our_display != identity.display_name() {
        friends.our_display = identity.display_name().into();
    }
    let fingerprint = (identity.username.clone(), identity.is_guest);
    if previous.as_ref() == Some(&fingerprint) {
        return;
    }
    *previous = Some(fingerprint);
    match crate::multiplayer::network::identity::active_social_identity() {
        Ok(profile) => {
            if friends.profile.as_ref().map(|p| &p.id) != profile.as_ref().map(|p| &p.id) {
                if let Ok(previous) = friends.client() {
                    crate::core::net_task::spawn_detached("profile-offline", move || {
                        let _ = previous.heartbeat("offline", None, None);
                    });
                }
                // Drop old receivers and view state. Their work retains the old profile's
                // credentials, and late results can never populate the new profile.
                let transport = friends.our_node_id.clone();
                let wallet = friends.our_pubkey.clone();
                let display = friends.our_display.clone();
                *friends = FriendsState {
                    profile,
                    our_node_id: transport,
                    our_pubkey: wallet,
                    our_display: display,
                    dirty: true,
                    ..default()
                };
                ui.reset_profile();
            } else {
                friends.profile = profile;
            }
        }
        Err(error) => {
            friends.profile = None;
            friends.contacts.clear();
            friends.pending_requests.clear();
            friends.outgoing_requests.clear();
            friends.pending_invites.clear();
            friends.blocked.clear();
            friends.fetch = None;
            friends.mutation = None;
            friends.busy = false;
            friends.loading = false;
            friends.error = Some(error);
        }
    }
}

fn fetch(client: ProfileSocialClient) -> Result<Snapshot, String> {
    // Preserve the last complete snapshot on any failure; a server outage is
    // never rendered as a successful empty friends list.
    Ok(Snapshot {
        contacts: client.contacts()?,
        requests: client.requests()?,
        outgoing: client.outgoing()?,
        invites: client.invites()?,
        blocked: client.blocks()?,
    })
}

pub(super) fn update_friends(mut state: ResMut<FriendsState>) {
    if let Some(job) = state.mutation.as_ref() {
        match job.poll() {
            Poll::Running => {}
            result => {
                state.mutation = None;
                state.busy = false;
                match result {
                    Poll::Ready(Ok(message)) => {
                        state.notice = Some(message);
                        state.error = None;
                    }
                    Poll::Ready(Err(error)) => state.error = Some(error),
                    _ => state.error = Some("The friends operation stopped. Please retry.".into()),
                }
                // A pre-mutation fetch must not overwrite this action's next snapshot.
                state.fetch = None;
                state.loading = false;
                state.dirty = true;
            }
        }
    }
    if let Some(job) = state.fetch.as_ref() {
        match job.poll() {
            Poll::Running => {}
            result => {
                state.fetch = None;
                state.loading = false;
                state.last_sync = Some(Instant::now());
                match result {
                    Poll::Ready(Ok(snapshot)) => {
                        state.contacts = snapshot.contacts;
                        state.pending_requests = snapshot.requests;
                        state.outgoing_requests = snapshot.outgoing;
                        state.pending_invites = snapshot.invites;
                        state.blocked = snapshot.blocked;
                        state.error = None;
                    }
                    Poll::Ready(Err(error)) => state.error = Some(error),
                    _ => state.error = Some("Friends sync stopped. Please refresh.".into()),
                }
            }
        }
    }
    if state.profile.is_none() {
        return;
    }
    if state.mutation.is_none() {
        if let Some(action) = state.actions.pop_front() {
            match state.client() {
                Ok(client) => {
                    state.mutation = Some(Pending::spawn("friend-action", move || {
                        let message = match action {
                            FriendAction::Send(target) => {
                                client.send(&target)?;
                                "Friend request sent"
                            }
                            FriendAction::SendName(username) => {
                                client.send_username(&username)?;
                                "Friend request sent"
                            }
                            FriendAction::Respond(id, accept) => {
                                client.respond(&id, accept)?;
                                if accept {
                                    "Friend added"
                                } else {
                                    "Request declined"
                                }
                            }
                            FriendAction::Cancel(id) => {
                                client.cancel(&id)?;
                                "Request cancelled"
                            }
                            FriendAction::Remove(target) => {
                                client.remove(&target)?;
                                "Friend removed"
                            }
                            FriendAction::Invite {
                                game,
                                target,
                                direct,
                            } => {
                                client.invite(&game, &target, direct)?;
                                "Invitation sent"
                            }
                            FriendAction::DismissInvite(id) => {
                                client.dismiss_invite(&id)?;
                                "Invitation dismissed"
                            }
                            FriendAction::Block(target) => {
                                client.block(&target)?;
                                "Player blocked"
                            }
                            FriendAction::Unblock(target) => {
                                client.unblock(&target)?;
                                "Player unblocked"
                            }
                        };
                        Ok(message.into())
                    }))
                }
                Err(error) => {
                    state.busy = false;
                    state.error = Some(error);
                }
            }
        }
    }
    if state.fetch.is_none()
        && state.mutation.is_none()
        && (state.dirty
            || state
                .last_sync
                .is_none_or(|last| last.elapsed().as_secs() >= 10))
    {
        state.dirty = false;
        match state.client() {
            Ok(client) => {
                state.loading = true;
                state.fetch = Some(Pending::spawn("friends-sync", move || fetch(client)));
            }
            Err(error) => {
                state.error = Some(error);
                state.last_sync = Some(Instant::now());
            }
        }
    }
}
