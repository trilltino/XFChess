use super::{native_network::NativeNetwork, subscription::SubscriptionStreamHandle};
use super::{HeartbeatConfig, MessageParser, RetryConfig, RetryDecision, RetryState};
use crate::{BraidError, BraidRequest, Result, Update};
use async_channel::Sender;
use futures::StreamExt;
use std::{io, time::Duration};

pub(crate) struct SubscriptionFailure {
    pub error: BraidError,
    pub retry_after: Option<Duration>,
}

impl From<BraidError> for SubscriptionFailure {
    fn from(error: BraidError) -> Self {
        Self {
            error,
            retry_after: None,
        }
    }
}

fn retry_decision(retry: &mut RetryState, failure: &SubscriptionFailure) -> RetryDecision {
    match &failure.error {
        BraidError::InvalidSubscriptionStatus(status) => {
            retry.should_retry_status(*status, failure.retry_after)
        }
        BraidError::Http(_)
        | BraidError::Io(_)
        | BraidError::Timeout
        | BraidError::SubscriptionClosed => retry.should_retry_error(false),
        _ => RetryDecision::DontRetry,
    }
}

async fn connect(
    network: &NativeNetwork,
    url: &str,
    request: &BraidRequest,
    retry: &mut RetryState,
) -> Result<reqwest::Response> {
    loop {
        match network.open_subscription(url, request).await {
            Ok(response) => return Ok(response),
            Err(failure) => match retry_decision(retry, &failure) {
                RetryDecision::Retry(delay) => tokio::time::sleep(delay).await,
                RetryDecision::DontRetry => return Err(failure.error),
            },
        }
    }
}

fn heartbeat(response: &reqwest::Response) -> Option<HeartbeatConfig> {
    response
        .headers()
        .get("heartbeats")
        .and_then(|v| v.to_str().ok())
        .and_then(HeartbeatConfig::from_header)
}

pub(crate) async fn subscribe(
    network: NativeNetwork,
    url: &str,
    request: BraidRequest,
) -> Result<SubscriptionStreamHandle> {
    let mut retry = RetryState::new(request.retry.clone().unwrap_or_else(RetryConfig::no_retry));
    let response = connect(&network, url, &request, &mut retry).await?;
    let heartbeat = heartbeat(&response);
    let (tx, updates) = async_channel::bounded(100);
    let (ack_tx, ack_rx) = async_channel::bounded(1);
    let manual_ack = request.manual_ack;
    tokio::spawn(run(
        network,
        url.to_owned(),
        request,
        response,
        tx,
        retry,
        manual_ack.then_some(ack_rx),
    ));
    Ok(SubscriptionStreamHandle {
        updates,
        heartbeat,
        acknowledgments: manual_ack.then_some(ack_tx),
    })
}

async fn run(
    network: NativeNetwork,
    url: String,
    mut request: BraidRequest,
    mut response: reqwest::Response,
    tx: Sender<Result<Update>>,
    mut retry: RetryState,
    acknowledgments: Option<async_channel::Receiver<(Vec<crate::Version>, bool)>>,
) {
    let resource = reqwest::Url::parse(&url)
        .expect("URL validated during handshake")
        .path()
        .to_owned();
    loop {
        let result = tokio::select! {
            _ = tx.closed() => return,
            result = forward(response, &resource, &tx, &mut request, &mut retry, acknowledgments.as_ref()) => result,
        };
        let failure =
            SubscriptionFailure::from(result.err().unwrap_or(BraidError::SubscriptionClosed));
        match retry_decision(&mut retry, &failure) {
            RetryDecision::DontRetry => {
                if !matches!(failure.error, BraidError::SubscriptionClosed)
                    || request.retry.is_some()
                {
                    let _ = tx.send(Err(failure.error)).await;
                }
                return;
            }
            RetryDecision::Retry(delay) => {
                tokio::select! { _ = tx.closed() => return, _ = tokio::time::sleep(delay) => {} }
            }
        }
        let connected = tokio::select! {
            _ = tx.closed() => return,
            result = connect(&network, &url, &request, &mut retry) => result,
        };
        match connected {
            Ok(connected) => response = connected,
            Err(error) => {
                let _ = tx.send(Err(error)).await;
                return;
            }
        }
    }
}

async fn forward(
    response: reqwest::Response,
    resource: &str,
    tx: &Sender<Result<Update>>,
    request: &mut BraidRequest,
    retry: &mut RetryState,
    acknowledgments: Option<&async_channel::Receiver<(Vec<crate::Version>, bool)>>,
) -> Result<()> {
    let heartbeat = heartbeat(&response);
    let mut stream = response.bytes_stream();
    let mut parser = MessageParser::for_subscription();
    loop {
        let chunk = match heartbeat {
            Some(config) => tokio::time::timeout(config.timeout, stream.next())
                .await
                .map_err(|_| BraidError::Timeout)?,
            None => stream.next().await,
        };
        let Some(chunk) = chunk else {
            parser
                .finish()
                .map_err(|error| io::Error::new(io::ErrorKind::UnexpectedEof, error))?;
            return Ok(());
        };
        let chunk = chunk.map_err(|e| BraidError::Http(e.to_string()))?;
        for message in parser.feed(&chunk)? {
            let mut update = super::utils::message_to_update(message)?;
            update.url.get_or_insert_with(|| resource.to_owned());
            let version = update.version.clone();
            let parents = update.parents.clone();
            tx.send(Ok(update)).await.map_err(|_| BraidError::Aborted)?;
            if let Some(acknowledgments) = acknowledgments {
                loop {
                    let (accepted_version, accepted) = acknowledgments
                        .recv()
                        .await
                        .map_err(|_| BraidError::Aborted)?;
                    if accepted_version != version {
                        continue;
                    }
                    if !accepted {
                        return Err(BraidError::Aborted);
                    }
                    break;
                }
            }
            if !version.is_empty() {
                let frontier = request.parents.get_or_insert_with(Vec::new);
                if parents.is_empty() {
                    frontier.clear();
                } else {
                    frontier.retain(|head| !parents.contains(head));
                }
                for head in version {
                    if !frontier.contains(&head) {
                        frontier.push(head);
                    }
                }
            }
            retry.reset();
        }
    }
}
