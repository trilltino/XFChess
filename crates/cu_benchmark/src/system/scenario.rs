use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::cost::estimate::{FeeProfile, GameVariant};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    pub backend_url: String,
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub variables: BTreeMap<String, Value>,
    #[serde(default)]
    pub actors: Vec<BTreeMap<String, Value>>,
    pub workflow: Workflow,
    #[serde(default)]
    pub telemetry: Vec<Metric>,
    #[serde(default = "sampling_ms")]
    pub sampling_ms: u64,
    #[serde(default = "sampling_ms")]
    pub baseline_ms: u64,
    #[serde(default)]
    pub drain_ms: u64,
    #[serde(default = "timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub sends_transactions: bool,
    pub limits: Limits,
    pub pricing: super::pricing::Pricing,
    pub chain: Option<ChainModel>,
    pub transaction_metadata: Option<TransactionMetadata>,
}

fn sampling_ms() -> u64 {
    1000
}
fn timeout_ms() -> u64 {
    30_000
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub name: String,
    pub games: u32,
    pub concurrency: u32,
    #[serde(default)]
    pub ramp_ms: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    #[serde(default)]
    pub setup: Vec<Request>,
    #[serde(default)]
    pub moves: Vec<Value>,
    #[serde(default)]
    pub each_move: Vec<Request>,
    #[serde(default)]
    pub move_interval_ms: u64,
    #[serde(default)]
    pub finish: Vec<Request>,
    #[serde(default)]
    pub cleanup: Vec<Request>,
    #[serde(default)]
    pub spectators: Vec<Subscription>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub name: String,
    pub method: String,
    pub path: String,
    pub body: Option<Value>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub capture: BTreeMap<String, String>,
    #[serde(default)]
    pub expect: BTreeMap<String, Value>,
    #[serde(default)]
    pub idempotent: bool,
    #[serde(default)]
    pub retries: u8,
    pub signature: Option<String>,
    pub layer: Option<crate::cost::measurements::ExecutionLayer>,
    #[serde(default)]
    pub capital_delta_lamports: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Subscription {
    pub name: String,
    pub path: String,
    pub count: u32,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Counter,
    Gauge,
    CpuUtilization,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    pub name: String,
    pub url: String,
    pub metric: String,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    pub kind: MetricKind,
    pub unit: String,
    #[serde(default)]
    pub required: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub bearer_env: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub request_p95_ms: f64,
    pub error_ratio: f64,
    pub min_duration_ms: u64,
    pub max_schedule_lag_ms: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChainModel {
    pub fee_profile: FeeProfile,
    pub variant: GameVariant,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionMetadata {
    pub solana_rpc_url_env: String,
    pub er_rpc_url_env: String,
}

impl Scenario {
    pub fn requests(&self) -> impl Iterator<Item = &Request> {
        self.workflow
            .setup
            .iter()
            .chain(&self.workflow.each_move)
            .chain(&self.workflow.finish)
            .chain(&self.workflow.cleanup)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(!self.name.trim().is_empty(), "scenario name is required");
        validate_url(&self.backend_url)?;
        ensure!(!self.stages.is_empty(), "at least one stage is required");
        let mut names = BTreeSet::new();
        for stage in &self.stages {
            ensure!(names.insert(&stage.name), "duplicate stage name");
            ensure!(
                stage.games > 0 && stage.concurrency > 0 && stage.concurrency <= stage.games,
                "stage concurrency must be between 1 and games"
            );
        }
        ensure!(
            self.sampling_ms > 0 && self.timeout_ms > 0,
            "sampling and timeout must be positive"
        );
        ensure!(
            self.baseline_ms > 0 || self.telemetry.is_empty(),
            "telemetry requires a baseline window"
        );
        ensure!(
            self.limits.request_p95_ms.is_finite() && self.limits.request_p95_ms > 0.0,
            "request p95 limit must be finite and positive"
        );
        ensure!(
            (0.0..=1.0).contains(&self.limits.error_ratio),
            "invalid error ratio limit"
        );
        ensure!(
            self.limits.max_schedule_lag_ms.is_finite() && self.limits.max_schedule_lag_ms >= 0.0,
            "invalid schedule lag limit"
        );
        ensure!(self.requests().next().is_some(), "workflow is empty");
        ensure!(
            self.workflow.moves.is_empty() == self.workflow.each_move.is_empty(),
            "moves and each_move must be supplied together"
        );
        names.clear();
        for request in self.requests() {
            ensure!(names.insert(&request.name), "request names must be unique");
            validate_path(&request.path)?;
            let method = reqwest::Method::from_bytes(request.method.as_bytes())?;
            ensure!(
                matches!(
                    method,
                    reqwest::Method::GET
                        | reqwest::Method::POST
                        | reqwest::Method::PUT
                        | reqwest::Method::PATCH
                        | reqwest::Method::DELETE
                ),
                "unsupported request method"
            );
            ensure!(
                request.retries <= 5 && (request.retries == 0 || request.idempotent),
                "retries require an explicitly idempotent request and are limited to five"
            );
            ensure!(
                request.signature.is_some() == request.layer.is_some(),
                "signature capture requires an execution layer"
            );
            for pointer in request
                .capture
                .values()
                .chain(request.expect.keys())
                .chain(request.signature.iter())
            {
                ensure!(
                    pointer.is_empty() || pointer.starts_with('/'),
                    "captures and expectations use JSON pointers"
                );
            }
            for key in request.capture.keys() {
                ensure!(!is_reserved(key), "cannot capture reserved variable {key}");
            }
        }
        for subscription in &self.workflow.spectators {
            validate_path(&subscription.path)?;
            ensure!(subscription.count > 0, "spectator count must be positive");
        }
        for variables in std::iter::once(&self.variables).chain(&self.actors) {
            for key in variables.keys() {
                ensure!(!is_reserved(key), "reserved variable {key}");
            }
        }
        names.clear();
        for metric in &self.telemetry {
            ensure!(names.insert(&metric.name), "duplicate telemetry name");
            validate_url(&metric.url)?;
            ensure!(
                metric.kind != MetricKind::CpuUtilization || !metric.labels.contains_key("mode"),
                "CPU utilization requires all CPU modes"
            );
            ensure!(
                metric.min.is_none_or(f64::is_finite) && metric.max.is_none_or(f64::is_finite),
                "metric limits must be finite"
            );
            ensure!(
                metric.min.zip(metric.max).is_none_or(|(a, b)| a <= b),
                "metric min exceeds max"
            );
        }
        self.pricing.validate(&self.telemetry)?;
        if let Some(chain) = &self.chain {
            crate::cost::estimate::estimate(&chain.variant, chain.fee_profile)?;
        }
        Ok(())
    }
}

pub fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "run_id" | "game_id" | "game_index" | "stage" | "move" | "move_index"
    )
}

pub fn validate_url(raw: &str) -> Result<url::Url> {
    let url = url::Url::parse(raw).context("invalid endpoint URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
        "endpoint must use HTTP(S)"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "use environment-backed authorization instead of URL credentials"
    );
    Ok(url)
}

pub fn validate_path(path: &str) -> Result<()> {
    ensure!(
        path.starts_with('/') && !path.starts_with("//") && !path.contains('\\'),
        "workflow paths must remain on the backend origin"
    );
    Ok(())
}
