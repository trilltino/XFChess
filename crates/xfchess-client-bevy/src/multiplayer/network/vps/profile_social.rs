//! A request snapshot owns its profile credentials. Switching profiles cannot
//! change the authority of an already queued operation.
use super::client::{client_fast, vps_base};
use super::social::{
    BlockedPlayer, Contact, FriendRequest, LobbyInvite, Presence, SocialPollResponse,
};
use crate::multiplayer::network::profile_identity::ProfileIdentity;
use reqwest::{
    blocking::{Client, RequestBuilder},
    Method,
};

#[derive(Clone)]
pub struct ProfileSocialClient {
    http: Client,
    base: String,
    profile: ProfileIdentity,
    transport_auth: Option<String>,
    wallet: Option<String>,
}

impl ProfileSocialClient {
    pub fn new(profile: ProfileIdentity) -> Result<Self, String> {
        let transport_auth = super::auth_headers()
            .into_iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("x-node-auth"))
            .map(|(_, value)| value);
        Ok(Self {
            http: client_fast()?,
            base: vps_base(),
            profile,
            transport_auth,
            wallet: None,
        })
    }

    /// The connected wallet, named in requests. The backend honours it only when the bearer
    /// token proves it, so friendships form between verified wallets only.
    pub fn with_wallet(mut self, wallet: Option<String>) -> Self {
        self.wallet = wallet;
        self
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.request_query(method, path, &[])
    }

    fn request_query(&self, method: Method, path: &str, query: &[(&str, &str)]) -> RequestBuilder {
        let url = format!("{}{path}", self.base);
        let builder = match reqwest::Url::parse(&url) {
            Ok(mut parsed) => {
                if !query.is_empty() {
                    parsed.query_pairs_mut().extend_pairs(query.iter().copied());
                }
                self.http.request(method, parsed)
            }
            Err(_) => self.http.request(method, url),
        };
        builder.header("X-Node-Auth", self.profile.auth_token())
    }

    fn json<T: serde::de::DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, String> {
        let response = request.send().map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response
                .json::<serde_json::Value>()
                .ok()
                .and_then(|v| v["error"].as_str().map(str::to_string))
                .unwrap_or_default();
            return Err(format!("Friends: {status} {detail}"));
        }
        response.json().map_err(|e| e.to_string())
    }

    fn empty(&self, request: RequestBuilder) -> Result<(), String> {
        let response = request.send().map_err(|e| e.to_string())?;
        if response.status().is_success() {
            return Ok(());
        }
        let status = response.status();
        let detail = response
            .json::<serde_json::Value>()
            .ok()
            .and_then(|v| v["error"].as_str().map(str::to_string))
            .unwrap_or_default();
        Err(format!("Friends: {status} {detail}"))
    }

    pub fn contacts(&self) -> Result<Vec<Contact>, String> {
        self.json(self.request_query(Method::GET, "/friends", &[("node_id", &self.profile.id)]))
    }

    pub fn requests(&self) -> Result<Vec<FriendRequest>, String> {
        // Requests addressed to this wallet are visible only when the wallet is named; the
        // backend honours it only if the bearer token proves it.
        let mut query = vec![("node_id", self.profile.id.as_str())];
        if let Some(wallet) = self.wallet.as_deref() {
            query.push(("pubkey", wallet));
        }
        self.json(self.request_query(Method::GET, "/friends/requests", &query))
    }

    pub fn outgoing(&self) -> Result<Vec<FriendRequest>, String> {
        self.json(self.request_query(
            Method::GET,
            "/friends/requests",
            &[("node_id", self.profile.id.as_str()), ("outgoing", "true")],
        ))
    }

    pub fn cancel(&self, id: &str) -> Result<(), String> {
        self.empty(self.request_query(
            Method::DELETE,
            &format!("/friends/requests/{id}"),
            &[("node_id", &self.profile.id)],
        ))
    }

    pub fn send(&self, to: &str) -> Result<(), String> {
        self.json::<FriendRequest>(self.request(Method::POST, "/friends/requests").json(&serde_json::json!({
            "from_node_id": self.profile.id, "from_pubkey": self.wallet, "from_display": self.profile.display, "to_node_id": to,
        }))).map(|_| ())
    }

    pub fn send_username(&self, username: &str) -> Result<(), String> {
        self.json::<FriendRequest>(self.request(Method::POST, "/friends/requests").json(&serde_json::json!({
            "from_node_id": self.profile.id, "from_pubkey": self.wallet, "from_display": self.profile.display, "to_username": username,
        }))).map(|_| ())
    }

    pub fn respond(&self, id: &str, accept: bool) -> Result<(), String> {
        self.empty(
            self.request(Method::PUT, &format!("/friends/requests/{id}"))
                .json(&serde_json::json!({
                    "responder_node_id": self.profile.id, "responder_display": self.profile.display,
                    "action": if accept { "accept" } else { "reject" },
                })),
        )
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        self.empty(self.request_query(
            Method::DELETE,
            &format!("/friends/{id}"),
            &[("node_id", &self.profile.id)],
        ))
    }

    pub fn blocks(&self) -> Result<Vec<BlockedPlayer>, String> {
        self.json(self.request_query(
            Method::GET,
            "/friends/blocks",
            &[("node_id", &self.profile.id)],
        ))
    }

    pub fn block(&self, target: &str) -> Result<(), String> {
        self.empty(
            self.request(Method::POST, "/friends/blocks")
                .json(&serde_json::json!({
                    "blocker_node_id": self.profile.id, "blocked_node_id": target,
                })),
        )
    }

    pub fn unblock(&self, target: &str) -> Result<(), String> {
        self.empty(self.request_query(
            Method::DELETE,
            &format!("/friends/blocks/{target}"),
            &[("node_id", &self.profile.id)],
        ))
    }

    pub fn heartbeat(
        &self,
        status: &str,
        game_id: Option<&str>,
        wallet: Option<&str>,
    ) -> Result<(), String> {
        let transport = self
            .transport_auth
            .as_deref()
            .ok_or("The network is not ready")?;
        self.json::<serde_json::Value>(
            self.request(Method::PUT, "/presence")
                .header("X-Transport-Auth", transport)
                .json(&Presence {
                    node_id: self.profile.id.clone(),
                    pubkey: wallet.map(str::to_string),
                    display_name: self.profile.display.clone(),
                    status: status.into(),
                    game_id: game_id.map(str::to_string),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                    transport_node_id: None,
                }),
        )
        .map(|_| ())
    }

    pub fn invites(&self) -> Result<Vec<LobbyInvite>, String> {
        self.json::<SocialPollResponse>(self.request_query(
            Method::GET,
            "/social/poll",
            &[("node_id", &self.profile.id)],
        ))
        .map(|response| response.invites)
    }

    pub fn dismiss_invite(&self, id: &str) -> Result<(), String> {
        self.empty(self.request_query(
            Method::DELETE,
            &format!("/friends/invites/{id}"),
            &[("node_id", &self.profile.id)],
        ))
    }

    pub fn invite(&self, game: &str, target: &str, direct: bool) -> Result<(), String> {
        let transport = self
            .transport_auth
            .as_deref()
            .ok_or("The network connection is not ready")?;
        self.empty(self.request(Method::POST, "/friends/invite").header("X-Transport-Auth", transport).json(&serde_json::json!({
            "game_id": game, "from_node_id": self.profile.id, "from_display": self.profile.display, "to_node_id": target, "direct": direct,
        })))
    }
}
