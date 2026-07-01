//! Transactional email service.
//!
//! A small, provider-agnostic mailer. Today it talks to the Resend API
//! (`RESEND_API_KEY`); swapping to Amazon SES / SendGrid later means changing
//! only `send_email` — every template and handler stays the same.
//!
//! Email "kinds" are just template builders (see [`EmailKind`]). Add a new
//! variant + a `build()` arm to introduce a new email type.
//!
//! Env:
//! - `RESEND_API_KEY`  — required to actually send (missing = store-only, logged)
//! - `MAIL_FROM`       — from address, e.g. `XFChess <hello@xfchess.com>`
//!                       (defaults to Resend's shared test sender)
//! - `RESEND_API_URL`  — overridable for testing

use axum::{extract::State, http::StatusCode, Json, Router, routing::post};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use tracing::{info, warn, error};

// ── Requests ────────────────────────────────────────────────────────────────

/// New-account signup (fires the confirmation email).
#[derive(Deserialize, Serialize, Default)]
pub struct SignUpRequest {
    pub email: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub wallet_pubkey: Option<String>,
    #[serde(default)]
    pub referral: Option<String>,
}

/// Waitlist opt-in (fires the waitlist email).
#[derive(Deserialize, Serialize, Default)]
pub struct WaitlistRequest {
    pub email: String,
    #[serde(default)]
    pub referral: Option<String>,
}

/// Uniform response: `ok` = we recorded it, `queued` = the email job is enqueued
/// for durable delivery (sent asynchronously with retries; see `tasks::queue`).
#[derive(Serialize)]
pub struct MailResponse {
    pub ok: bool,
    pub queued: bool,
}

/// Payload for the `email.send` job kind (see [`handle_email_job`]).
#[derive(Serialize, Deserialize)]
pub struct MailJob {
    pub email: String,
    /// "confirmation" | "waitlist"
    pub template: String,
    #[serde(default)]
    pub name: Option<String>,
}

// ── Templates ───────────────────────────────────────────────────────────────

/// The set of emails we can send. Each variant knows how to render itself.
pub enum EmailKind<'a> {
    /// Account confirmation / welcome.
    Confirmation { name: &'a str },
    /// "You're on the waitlist" acknowledgement.
    Waitlist,
}

impl EmailKind<'_> {
    fn subject(&self) -> String {
        match self {
            EmailKind::Confirmation { .. } => "Welcome to XFChess".to_string(),
            EmailKind::Waitlist => "You're on the XFChess waitlist".to_string(),
        }
    }

    fn html(&self) -> String {
        match self {
            EmailKind::Confirmation { name } => shell(
                "Welcome to XFChess",
                &format!(
                    "<p>Hey {name},</p>\
                     <p>Your XFChess profile is live. You can jump straight into free games \
                     or challenge the chess computer.</p>\
                     <p>Connect a Solana wallet and complete KYC to unlock PvP wagering and \
                     Cash Tournaments.</p>\
                     <p>See you on the board,<br/>— The XFChess Team</p>",
                    name = html_escape(name),
                ),
            ),
            EmailKind::Waitlist => shell(
                "You're on the waitlist",
                "<p>Thanks for your interest in XFChess.</p>\
                 <p>You're on the waitlist — we'll email you the moment your spot opens up.</p>\
                 <p>— The XFChess Team</p>",
            ),
        }
    }
}

/// Minimal branded HTML wrapper shared by every email.
fn shell(heading: &str, body_html: &str) -> String {
    format!(
        "<!doctype html><html><body style=\"margin:0;background:#0d0d0f;padding:32px 0;\
         font-family:-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif;color:#e6e6e6;\">\
         <table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"><tr><td align=\"center\">\
         <table role=\"presentation\" width=\"520\" cellpadding=\"0\" cellspacing=\"0\" \
         style=\"background:#151519;border:1px solid #26262c;border-radius:16px;overflow:hidden;\">\
         <tr><td style=\"padding:28px 32px;border-bottom:1px solid #26262c;\">\
         <span style=\"font-size:20px;font-weight:800;color:#fff;\">XF<span style=\"color:#14f195;\">Chess</span></span>\
         </td></tr>\
         <tr><td style=\"padding:32px;\">\
         <h1 style=\"margin:0 0 16px;font-size:22px;color:#fff;\">{heading}</h1>\
         <div style=\"font-size:15px;line-height:1.7;color:#b8b8bf;\">{body}</div>\
         </td></tr>\
         <tr><td style=\"padding:20px 32px;border-top:1px solid #26262c;font-size:12px;color:#6b6b73;\">\
         XFChess — Decentralised chess on Solana.\
         </td></tr>\
         </table></td></tr></table></body></html>",
        heading = html_escape(heading),
        body = body_html,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ── Core send (the only provider-specific code) ─────────────────────────────

/// Send one email. Returns `Ok(false)` (not an error) when no API key is
/// configured, so callers can still record the signup and degrade gracefully.
pub async fn send_email(to: &str, kind: EmailKind<'_>) -> Result<bool, String> {
    let api_key = match env::var("RESEND_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            warn!("[mailer] RESEND_API_KEY not set — skipping send to {}", to);
            return Ok(false);
        }
    };

    let from = env::var("MAIL_FROM")
        .unwrap_or_else(|_| "XFChess <onboarding@resend.dev>".to_string());
    let url = env::var("RESEND_API_URL")
        .unwrap_or_else(|_| "https://api.resend.com/emails".to_string());

    let payload = json!({
        "from": from,
        "to": [to],
        "subject": kind.subject(),
        "html": kind.html(),
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if resp.status().is_success() {
        info!("[mailer] sent email to {}", to);
        Ok(true)
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(format!("provider error {status}: {body}"))
    }
}

// ── Storage (never lose a signup, even if email fails) ──────────────────────

fn append_jsonl(file: &str, value: &impl Serialize) {
    let path = PathBuf::from("data").join(file);
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("[mailer] mkdir {} failed: {}", parent.display(), e);
            return;
        }
    }
    let line = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(e) => { warn!("[mailer] serialize failed: {}", e); return; }
    };
    match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut f) => { let _ = writeln!(f, "{}", line); }
        Err(e) => warn!("[mailer] open {} failed: {}", path.display(), e),
    }
}

fn valid_email(email: &str) -> bool {
    let e = email.trim();
    e.len() >= 3 && e.len() <= 254 && e.contains('@') && e.split('@').count() == 2
        && e.split('@').nth(1).map(|d| d.contains('.')).unwrap_or(false)
}

// ── Durable delivery (job queue) ────────────────────────────────────────────

/// Enqueue an email for durable delivery. Deduped per (template, email, day) so a
/// double-submit can't double-send, but a genuine re-signup weeks later still works.
async fn enqueue_email(
    pool: &sqlx::SqlitePool,
    template: &str,
    email: &str,
    name: Option<String>,
) -> bool {
    let job = MailJob { email: email.to_string(), template: template.to_string(), name };
    let day = chrono::Utc::now().format("%Y-%m-%d");
    let dedupe = format!("email:{template}:{email}:{day}");
    match crate::tasks::queue::enqueue(pool, "email.send", &job, Some(&dedupe)).await {
        Ok(Some(_)) => true,
        Ok(None) => {
            info!("[mailer] duplicate {template} for {email} today — not re-queued");
            true // already queued/sent today; from the caller's view this is success
        }
        Err(e) => {
            error!("[mailer] enqueue failed: {}", e);
            false
        }
    }
}

/// Job handler for `email.send` — registered on the queue worker at startup.
/// At-least-once: rendering + Resend send are idempotent enough (worst case one
/// duplicate email on a crash between send and mark_done).
pub async fn handle_email_job(job: crate::tasks::queue::Job) -> Result<(), String> {
    let mail: MailJob = job.parse().map_err(|e| format!("bad payload: {e}"))?;
    let kind = match mail.template.as_str() {
        "confirmation" => {
            EmailKind::Confirmation { name: mail.name.as_deref().unwrap_or("Player") }
        }
        "waitlist" => EmailKind::Waitlist,
        other => return Err(format!("unknown email template '{other}'")),
    };
    // Ok(false) = no API key configured: logged inside send_email; treat as done so
    // dev environments don't fill the DLQ. Provider errors → Err → bounded retries.
    send_email(&mail.email, kind).await.map(|_| ())
}

// ── HTTP handlers ───────────────────────────────────────────────────────────

/// `POST /api/signup` — record subscriber + queue confirmation email.
pub async fn send_confirmation(
    State(app): State<crate::signing::AppState>,
    Json(req): Json<SignUpRequest>,
) -> Result<Json<MailResponse>, StatusCode> {
    if !valid_email(&req.email) {
        return Err(StatusCode::BAD_REQUEST);
    }
    append_jsonl("subscribers.jsonl", &req);

    let name = req
        .username
        .clone()
        .unwrap_or_else(|| req.email.split('@').next().unwrap_or("Player").to_string());

    let queued = enqueue_email(&app.store.pool(), "confirmation", &req.email, Some(name)).await;
    Ok(Json(MailResponse { ok: true, queued }))
}

/// `POST /api/waitlist` — record waitlist opt-in + queue acknowledgement email.
pub async fn join_waitlist(
    State(app): State<crate::signing::AppState>,
    Json(req): Json<WaitlistRequest>,
) -> Result<Json<MailResponse>, StatusCode> {
    if !valid_email(&req.email) {
        return Err(StatusCode::BAD_REQUEST);
    }
    append_jsonl("waitlist.jsonl", &req);

    let queued = enqueue_email(&app.store.pool(), "waitlist", &req.email, None).await;
    Ok(Json(MailResponse { ok: true, queued }))
}

/// Router for mailer endpoints (mounted under `/api`).
pub fn mailer_routes() -> Router<crate::signing::AppState> {
    Router::new()
        .route("/signup", post(send_confirmation))
        .route("/waitlist", post(join_waitlist))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_email_accepts_normal() {
        assert!(valid_email("a@b.com"));
        assert!(valid_email("user.name@domain.co.uk"));
        assert!(valid_email("user+tag@example.com"));
    }

    #[test]
    fn valid_email_rejects_bad() {
        assert!(!valid_email(""));
        assert!(!valid_email("nope"));
        assert!(!valid_email("no@domain"));
        assert!(!valid_email("a@@b.com"));
    }

    #[test]
    fn templates_render_non_empty() {
        assert!(EmailKind::Confirmation { name: "Ada" }.html().contains("Ada"));
        assert!(!EmailKind::Waitlist.html().is_empty());
        assert!(EmailKind::Waitlist.subject().contains("waitlist"));
    }

    #[test]
    fn html_escape_neutralises_tags() {
        assert_eq!(html_escape("<b>&"), "&lt;b&gt;&amp;");
    }

    #[test]
    fn routes_build() {
        let _ = mailer_routes();
    }
}
