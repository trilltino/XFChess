use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct RegisterMoneyActionReq {
    pub action_type: String,
    pub scope_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tournament_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MoneyActionRecord {
    pub id: String,
    pub action_type: String,
    pub scope_type: String,
    pub game_id: Option<i64>,
    pub tournament_id: Option<i64>,
    pub wallet: Option<String>,
    pub signature: Option<String>,
    pub status: String,
    pub reason: Option<String>,
    pub attempt_count: i64,
    pub last_error: Option<String>,
    pub next_retry_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MoneyActionList {
    pub money_actions: Vec<MoneyActionRecord>,
}

pub fn register_money_action(req: RegisterMoneyActionReq) -> Result<MoneyActionRecord, String> {
    let resp = super::client_fast()?
        .post(format!("{}/api/money-actions", super::vps_base()))
        .json(&req)
        .send()
        .map_err(|e| format!("vps register_money_action: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!(
            "vps register_money_action: HTTP {status} - {body}"
        ));
    }
    resp.json()
        .map_err(|e| format!("vps register_money_action parse: {e}"))
}

pub fn fetch_money_actions_by_scope(
    game_id: Option<i64>,
    tournament_id: Option<i64>,
    wallet: Option<&str>,
) -> Result<MoneyActionList, String> {
    let mut params = Vec::new();
    if let Some(game_id) = game_id {
        params.push(format!("game_id={game_id}"));
    }
    if let Some(tournament_id) = tournament_id {
        params.push(format!("tournament_id={tournament_id}"));
    }
    if let Some(wallet) = wallet {
        params.push(format!("wallet={wallet}"));
    }
    let query = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };

    let resp = super::client_fast()?
        .get(format!("{}/api/money-actions/by-scope{query}", super::vps_base()))
        .send()
        .map_err(|e| format!("vps fetch_money_actions_by_scope: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!(
            "vps fetch_money_actions_by_scope: HTTP {status} - {body}"
        ));
    }
    resp.json()
        .map_err(|e| format!("vps fetch_money_actions_by_scope parse: {e}"))
}
