use anyhow::{ensure, Result};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use super::super::scenario::{Request, Scenario};
use super::{
    template::{render, text, url},
    Attempt, CapitalEvent, GameResult, Variables,
};
use crate::cost::measurements::MeasurementLog;

const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

pub(super) struct RequestContext<'a> {
    pub client: &'a reqwest::Client,
    pub scenario: &'a Scenario,
    pub game_index: u32,
    pub start: Instant,
}

impl RequestContext<'_> {
    pub(super) async fn send(
        &self,
        step: &Request,
        variables: &mut Variables,
        lag_ms: f64,
        output: &mut GameResult,
    ) -> Result<()> {
        let endpoint = url(&self.scenario.backend_url, &step.path, variables)?;
        let body = step
            .body
            .as_ref()
            .map(|v| render(v, variables))
            .transpose()?;
        let bytes = body
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()?
            .unwrap_or_default();
        let headers = step
            .headers
            .iter()
            .map(|(k, v)| Ok((k.clone(), text(v, variables)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        for index in 0..=step.retries {
            let began = Instant::now();
            let mut record = Attempt {
                game_index: self.game_index,
                operation: step.name.clone(),
                attempt: index,
                elapsed_ms: self.start.elapsed().as_millis() as u64,
                latency_ms: 0.0,
                schedule_lag_ms: lag_ms,
                status: None,
                error: None,
                request_body_bytes: bytes.len() as u64,
                response_body_bytes: 0,
            };
            let operation = async {
                let method = reqwest::Method::from_bytes(step.method.as_bytes())?;
                let mut request = self.client.request(method, &endpoint).header(
                    "X-Benchmark-Run",
                    variables["run_id"].as_str().unwrap_or_default(),
                );
                for (key, value) in &headers {
                    request = request.header(key, value);
                }
                if body.is_some() {
                    request = request
                        .header("Content-Type", "application/json")
                        .body(bytes.clone());
                }
                let mut response = request.send().await?;
                record.status = Some(response.status().as_u16());
                let mut received = Vec::new();
                while let Some(chunk) = response.chunk().await? {
                    record.response_body_bytes += chunk.len() as u64;
                    ensure!(
                        received.len() + chunk.len() <= MAX_RESPONSE_BYTES,
                        "response too large"
                    );
                    received.extend_from_slice(&chunk);
                }
                let parsed: Value = serde_json::from_slice(&received).unwrap_or(Value::Null);
                if let (Some(pointer), Some(layer)) = (&step.signature, step.layer) {
                    if let Some(signature) = parsed
                        .pointer(pointer)
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                    {
                        let mut log = MeasurementLog::new();
                        log.log(
                            layer,
                            "backend",
                            &step.name,
                            response.status().is_success(),
                            Some(signature.into()),
                        );
                        output.transactions.extend_from_slice(log.entries());
                    }
                }
                if !response.status().is_success() {
                    record.error = Some("http".into());
                    return Ok(false);
                }
                for (pointer, expected) in &step.expect {
                    if parsed.pointer(pointer) != Some(&render(expected, variables)?) {
                        record.error = Some("expectation".into());
                        return Ok(false);
                    }
                }
                let mut captures = BTreeMap::new();
                for (key, pointer) in &step.capture {
                    let Some(value) = parsed.pointer(pointer) else {
                        record.error = Some("capture".into());
                        return Ok(false);
                    };
                    captures.insert(key.clone(), value.clone());
                }
                if step.signature.as_ref().is_some_and(|pointer| {
                    parsed
                        .pointer(pointer)
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                }) {
                    record.error = Some("missing_signature".into());
                    return Ok(false);
                }
                variables.extend(captures);
                Ok::<_, anyhow::Error>(true)
            };
            let success = match tokio::time::timeout(
                Duration::from_millis(self.scenario.timeout_ms),
                operation,
            )
            .await
            {
                Ok(Ok(value)) => value,
                Ok(Err(_)) => {
                    record.error = Some("transport_or_invalid_response".into());
                    false
                }
                Err(_) => {
                    record.error = Some("timeout".into());
                    false
                }
            };
            record.latency_ms = began.elapsed().as_secs_f64() * 1000.0;
            let retryable =
                matches!(record.status, Some(429) | Some(500..=599)) || record.status.is_none();
            output.attempts.push(record);
            if success {
                if step.capital_delta_lamports != 0 {
                    output.capital.push(CapitalEvent {
                        elapsed_ms: self.start.elapsed().as_millis() as u64,
                        delta_lamports: step.capital_delta_lamports,
                    });
                }
                return Ok(());
            }
            if index == step.retries || !retryable {
                anyhow::bail!("operation failed: {}", step.name);
            }
            let pause = if output
                .attempts
                .last()
                .is_some_and(|r| r.status == Some(429))
            {
                10_000
            } else {
                100 * (1u64 << index)
            };
            tokio::time::sleep(Duration::from_millis(pause)).await;
        }
        unreachable!()
    }
}
