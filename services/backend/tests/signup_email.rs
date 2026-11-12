//! Sign-up to welcome email: `POST /api/signup` queues one email job, and the job's handler
//! sends it through Resend. Resend is a local stand-in, so nothing leaves the machine.
mod common;

use anyhow::Context as _;
use axum::http::StatusCode;
use backend::signing::routes::mailer::handle_email_job;
use backend::tasks::queue::Job;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Answers every request with 200 and records each raw request.
async fn mock_resend() -> anyhow::Result<(String, Arc<Mutex<Vec<String>>>)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/emails", listener.local_addr()?);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let log = log.clone();
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let read = socket.read(&mut chunk).await.unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..read]);
                    let text = String::from_utf8_lossy(&request).into_owned();
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let length = head
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if body.len() >= length {
                            break;
                        }
                    }
                }
                if let Ok(mut seen) = log.lock() {
                    seen.push(String::from_utf8_lossy(&request).into_owned());
                }
                let body = r#"{"id":"mock"}"#;
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
            });
        }
    });
    Ok((url, seen))
}

async fn email_jobs(app: &common::TestApp) -> anyhow::Result<Vec<Job>> {
    let rows: Vec<(i64, String, String, i64)> =
        sqlx::query_as("SELECT id, kind, payload, attempts FROM jobs WHERE kind = 'email.send' ORDER BY id")
            .fetch_all(&app.state.store.pool())
            .await?;
    Ok(rows
        .into_iter()
        .map(|(id, kind, payload, attempts)| Job {
            id,
            kind,
            payload,
            attempts,
        })
        .collect())
}

#[tokio::test]
async fn a_signup_queues_one_welcome_that_resend_receives() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    let (url, seen) = mock_resend().await?;
    // Only this test touches these variables; the other test never sends.
    std::env::set_var("RESEND_API_URL", &url);
    std::env::set_var("RESEND_API_KEY", "re_test_key");
    std::env::set_var("MAIL_FROM", "XFChess <hello@example.com>");

    let (status, body) = app
        .post_json(
            "/api/signup",
            &json!({ "email": "tino@example.com", "username": "<b>Tino</b>" }),
        )
        .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["ok"], true);
    assert_eq!(body["queued"], true);

    // The same address again the same day is not queued twice.
    let (status, _) = app
        .post_json("/api/signup", &json!({ "email": "tino@example.com" }))
        .await?;
    assert_eq!(status, StatusCode::OK);
    let jobs = email_jobs(&app).await?;
    assert_eq!(jobs.len(), 1, "one welcome per address per day");

    handle_email_job(jobs[0].clone())
        .await
        .map_err(anyhow::Error::msg)
        .context("sending the welcome through the mock Resend")?;

    let requests = seen.lock().map_err(|e| anyhow::anyhow!("{e}"))?.clone();
    assert_eq!(requests.len(), 1, "exactly one email reaches Resend");
    let request = requests[0].to_ascii_lowercase();
    assert!(request.contains("authorization: bearer re_test_key"), "{request}");
    let (_, payload) = requests[0]
        .split_once("\r\n\r\n")
        .context("request has a body")?;
    let payload: serde_json::Value = serde_json::from_str(payload)?;
    assert_eq!(payload["to"], json!(["tino@example.com"]));
    assert_eq!(payload["from"], "XFChess <hello@example.com>");
    assert_eq!(payload["subject"], "Welcome to XFChess");
    let html = payload["html"].as_str().context("html body")?;
    assert!(html.contains("&lt;b&gt;Tino&lt;/b&gt;"), "the name is escaped: {html}");
    assert!(!html.contains("<b>Tino"), "no raw markup from the visitor");
    assert!(html.contains("https://xfchess.com/play"));

    // With no API key the job completes without sending anything (local development).
    std::env::remove_var("RESEND_API_KEY");
    handle_email_job(jobs[0].clone())
        .await
        .map_err(anyhow::Error::msg)
        .context("a missing key is a no-op")?;
    assert_eq!(seen.lock().map_err(|e| anyhow::anyhow!("{e}"))?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn an_invalid_address_is_refused_and_nothing_is_queued() -> anyhow::Result<()> {
    let app = common::spawn_app().await?;
    for email in ["", "nope", "a@@b.com", "no@domain"] {
        let (status, _) = app
            .post_json("/api/signup", &json!({ "email": email }))
            .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{email:?}");
    }
    assert!(email_jobs(&app).await?.is_empty());
    Ok(())
}
