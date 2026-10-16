use anyhow::{Context, Result};
use std::{collections::BTreeMap, time::Duration};
use tokio::{sync::watch, task::JoinSet};

use super::super::scenario::{Scenario, Subscription};
use super::{
    template::{text, url},
    StreamObservation, Variables,
};

async fn subscribe(
    client: reqwest::Client,
    endpoint: String,
    headers: BTreeMap<String, String>,
    mut stop: watch::Receiver<bool>,
    timeout_ms: u64,
    ready: tokio::sync::oneshot::Sender<()>,
) -> StreamObservation {
    let mut result = StreamObservation::default();
    let mut request = client.get(endpoint).header("Subscribe", "true");
    for (key, value) in headers {
        request = request.header(key, value);
    }
    let response = tokio::time::timeout(Duration::from_millis(timeout_ms), request.send()).await;
    let mut response = match response {
        Ok(Ok(response)) if response.status().is_success() => response,
        _ => {
            result.errors += 1;
            let _ = ready.send(());
            return result;
        }
    };
    result.connections += 1;
    let _ = ready.send(());
    loop {
        if *stop.borrow() {
            break;
        }
        tokio::select! {
            _ = stop.changed() => break,
            chunk = response.chunk() => match chunk {
                Ok(Some(bytes)) => { result.received_bytes += bytes.len() as u64; result.received_chunks += 1; },
                _ => { result.errors += 1; break; }
            }
        }
    }
    result
}

pub(super) fn merge_streams(target: &mut StreamObservation, value: StreamObservation) {
    target.connections += value.connections;
    target.errors += value.errors;
    target.received_bytes += value.received_bytes;
    target.received_chunks += value.received_chunks;
}

pub(super) async fn start_streams(
    client: &reqwest::Client,
    scenario: &Scenario,
    variables: &Variables,
    specs: &[Subscription],
    stop: &watch::Receiver<bool>,
    jobs: &mut JoinSet<StreamObservation>,
) -> Result<()> {
    for spec in specs {
        for _ in 0..spec.count {
            let endpoint = url(&scenario.backend_url, &spec.path, variables)?;
            let headers = spec
                .headers
                .iter()
                .map(|(k, v)| Ok((k.clone(), text(v, variables)?)))
                .collect::<Result<_>>()?;
            let (ready, receive) = tokio::sync::oneshot::channel();
            jobs.spawn(subscribe(
                client.clone(),
                endpoint,
                headers,
                stop.clone(),
                scenario.timeout_ms,
                ready,
            ));
            receive
                .await
                .context("stream task stopped before connecting")?;
        }
    }
    Ok(())
}
