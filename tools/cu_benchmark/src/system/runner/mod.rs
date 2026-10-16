mod game;
mod http;
mod streams;
mod template;

use anyhow::{ensure, Context, Result};
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;

use super::{
    collectors::{self, Sample},
    scenario::Scenario,
};
use crate::cost::measurements::{Measurement, MeasurementLog};
use game::game;
use streams::merge_streams;
pub use template::render;
use template::validate_environment;

type Variables = BTreeMap<String, Value>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Attempt {
    pub game_index: u32,
    pub operation: String,
    pub attempt: u8,
    pub elapsed_ms: u64,
    pub latency_ms: f64,
    pub schedule_lag_ms: f64,
    pub status: Option<u16>,
    pub error: Option<String>,
    pub request_body_bytes: u64,
    pub response_body_bytes: u64,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct StreamObservation {
    pub connections: u32,
    pub errors: u32,
    pub received_bytes: u64,
    pub received_chunks: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CapitalEvent {
    pub elapsed_ms: u64,
    pub delta_lamports: i64,
}

#[derive(Debug, Default)]
struct GameResult {
    completed: bool,
    cleanup_failed: bool,
    attempts: Vec<Attempt>,
    streams: StreamObservation,
    transactions: Vec<Measurement>,
    capital: Vec<CapitalEvent>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ObservedStage {
    pub name: String,
    pub requested_games: u32,
    pub requested_concurrency: u32,
    pub peak_concurrency: u32,
    pub completed_games: u32,
    pub cleanup_failed_games: u32,
    pub duration_ms: u64,
    pub attempts: Vec<Attempt>,
    pub streams: StreamObservation,
    pub samples: BTreeMap<String, Vec<Sample>>,
    pub baseline_samples: BTreeMap<String, Vec<Sample>>,
    pub capital: Vec<CapitalEvent>,
    pub transactions: Vec<Measurement>,
    pub warnings: Vec<String>,
}

fn append_samples(target: &mut BTreeMap<String, Vec<Sample>>, samples: BTreeMap<String, Sample>) {
    for (name, sample) in samples {
        target.entry(name).or_default().push(sample);
    }
}

async fn baseline(client: &reqwest::Client, scenario: &Scenario) -> BTreeMap<String, Vec<Sample>> {
    let mut output = BTreeMap::new();
    if scenario.telemetry.is_empty() {
        return output;
    }
    let start = Instant::now();
    append_samples(
        &mut output,
        collectors::sample(client, &scenario.telemetry, 0).await,
    );
    let deadline =
        tokio::time::Instant::from_std(start) + Duration::from_millis(scenario.baseline_ms);
    let mut timer = tokio::time::interval(Duration::from_millis(scenario.sampling_ms));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    timer.tick().await;
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => break,
            _ = timer.tick() => append_samples(&mut output, collectors::sample(client, &scenario.telemetry, start.elapsed().as_millis() as u64).await),
        }
    }
    append_samples(
        &mut output,
        collectors::sample(
            client,
            &scenario.telemetry,
            start.elapsed().as_millis() as u64,
        )
        .await,
    );
    output
}

pub async fn execute(scenario: &Scenario, allow_transactions: bool) -> Result<Vec<ObservedStage>> {
    ensure!(!scenario.sends_transactions || allow_transactions,
        "this scenario sends transactions; explicitly use --allow-transactions after reviewing its workflow");
    // Fail before starting any games when a required environment reference is missing.
    for request in scenario.requests() {
        for template in request
            .headers
            .values()
            .chain(std::iter::once(&request.path))
        {
            validate_environment(template)?;
        }
        if let Some(body) = &request.body {
            validate_environment(&body.to_string())?;
        }
    }
    for values in std::iter::once(&scenario.variables).chain(&scenario.actors) {
        for value in values.values() {
            validate_environment(&value.to_string())?;
        }
    }
    for value in scenario.workflow.moves.iter().chain(
        scenario
            .requests()
            .flat_map(|request| request.expect.values()),
    ) {
        validate_environment(&value.to_string())?;
    }
    for subscription in &scenario.workflow.spectators {
        validate_environment(&subscription.path)?;
        for header in subscription.headers.values() {
            validate_environment(header)?;
        }
    }
    for metric in &scenario.telemetry {
        if let Some(name) = &metric.bearer_env {
            ensure!(
                std::env::var(name).is_ok(),
                "missing telemetry environment variable {name}"
            );
        }
    }
    if let Some(metadata) = &scenario.transaction_metadata {
        for name in [&metadata.solana_rpc_url_env, &metadata.er_rpc_url_env] {
            super::scenario::validate_url(
                &std::env::var(name)
                    .with_context(|| format!("missing metadata endpoint {name}"))?,
            )?;
        }
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(scenario.timeout_ms))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let telemetry_client = reqwest::Client::builder()
        .timeout(Duration::from_millis(scenario.timeout_ms.min(5000)))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_nanos()
        .to_string();
    let mut stages = Vec::new();
    for stage in &scenario.stages {
        eprintln!(
            "{}: {} games, concurrency {}",
            stage.name, stage.games, stage.concurrency
        );
        let baseline_samples = baseline(&telemetry_client, scenario).await;
        let start = Instant::now();
        let mut output = ObservedStage {
            name: stage.name.clone(),
            requested_games: stage.games,
            requested_concurrency: stage.concurrency,
            peak_concurrency: 0,
            completed_games: 0,
            cleanup_failed_games: 0,
            duration_ms: 0,
            attempts: Vec::new(),
            streams: StreamObservation::default(),
            samples: BTreeMap::new(),
            baseline_samples,
            capital: Vec::new(),
            transactions: Vec::new(),
            warnings: Vec::new(),
        };
        append_samples(
            &mut output.samples,
            collectors::sample(&telemetry_client, &scenario.telemetry, 0).await,
        );
        let active = AtomicU32::new(0);
        let peak = AtomicU32::new(0);
        let (stop_sampling, receiver) = watch::channel(false);
        let metrics = scenario.telemetry.clone();
        let sampler_client = telemetry_client.clone();
        let interval_ms = scenario.sampling_ms;
        let sampler = tokio::spawn(async move {
            let mut result = BTreeMap::new();
            if metrics.is_empty() {
                return result;
            }
            let mut stop = receiver;
            let mut timer = tokio::time::interval(Duration::from_millis(interval_ms));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            timer.tick().await;
            loop {
                tokio::select! {
                    _ = stop.changed() => break,
                    _ = timer.tick() => append_samples(&mut result,collectors::sample(&sampler_client,&metrics,start.elapsed().as_millis() as u64).await),
                }
            }
            result
        });
        let jobs = stream::iter(0..stage.games)
            .map(|index| {
                let active = &active;
                let peak = &peak;
                let run_id = &run_id;
                let client = &client;
                async move {
                    if index < stage.concurrency && stage.concurrency > 1 {
                        let offset = u128::from(index) * u128::from(stage.ramp_ms)
                            / u128::from(stage.concurrency - 1);
                        tokio::time::sleep_until(
                            (start + Duration::from_millis(offset as u64)).into(),
                        )
                        .await;
                    }
                    let current = active.fetch_add(1, Ordering::Relaxed) + 1;
                    peak.fetch_max(current, Ordering::Relaxed);
                    let result = game(client, scenario, stage, index, run_id, start).await;
                    active.fetch_sub(1, Ordering::Relaxed);
                    result
                }
            })
            .buffer_unordered(stage.concurrency as usize);
        tokio::pin!(jobs);
        while let Some(result) = jobs.next().await {
            output.completed_games += u32::from(result.completed);
            output.cleanup_failed_games += u32::from(result.cleanup_failed);
            output.attempts.extend(result.attempts);
            merge_streams(&mut output.streams, result.streams);
            output.transactions.extend(result.transactions);
            output.capital.extend(result.capital);
        }
        tokio::time::sleep(Duration::from_millis(scenario.drain_ms)).await;
        let _ = stop_sampling.send(true);
        for (name, samples) in sampler.await.context("telemetry sampler failed")? {
            output.samples.entry(name).or_default().extend(samples);
        }
        append_samples(
            &mut output.samples,
            collectors::sample(
                &telemetry_client,
                &scenario.telemetry,
                start.elapsed().as_millis() as u64,
            )
            .await,
        );
        output.duration_ms = start.elapsed().as_millis() as u64;
        output.peak_concurrency = peak.load(Ordering::Relaxed);
        if let Some(metadata) = &scenario.transaction_metadata {
            let solana_url = std::env::var(&metadata.solana_rpc_url_env)
                .context("missing Solana metadata RPC endpoint")?;
            let er_url = std::env::var(&metadata.er_rpc_url_env)
                .context("missing ER metadata RPC endpoint")?;
            let mut log = MeasurementLog::from(output.transactions);
            output.warnings.extend(
                log.measure(
                    &solana_client::rpc_client::RpcClient::new_with_timeout(
                        solana_url,
                        Duration::from_millis(scenario.timeout_ms),
                    ),
                    &solana_client::rpc_client::RpcClient::new_with_timeout(
                        er_url,
                        Duration::from_millis(scenario.timeout_ms),
                    ),
                )
                .into_iter()
                .map(|_| "captured transaction metadata unavailable".to_owned()),
            );
            output.transactions = log.entries().to_vec();
        }
        output.attempts.sort_by_key(|value| value.elapsed_ms);
        output.capital.sort_by_key(|value| value.elapsed_ms);
        stages.push(output);
    }
    Ok(stages)
}
