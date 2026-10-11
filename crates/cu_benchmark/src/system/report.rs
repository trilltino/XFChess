use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use super::{
    collectors::{self, MetricSummary},
    pricing::{self, OperatingCosts},
    runner::ObservedStage,
    scenario::Scenario,
};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Capacity {
    Pass,
    Fail,
    Inconclusive,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StageReport {
    pub capacity: Capacity,
    pub reasons: Vec<String>,
    pub request_p50_ms: Option<f64>,
    pub request_p95_ms: Option<f64>,
    pub request_p99_ms: Option<f64>,
    pub schedule_lag_p95_ms: Option<f64>,
    pub attempt_error_ratio: Option<f64>,
    pub completed_games_per_second: f64,
    pub metrics: BTreeMap<String, MetricSummary>,
    pub operating_costs: OperatingCosts,
    pub measured_transaction_fees_lamports: u64,
    pub measured_cu: u64,
    pub recorded_transactions: usize,
    pub measured_transactions: usize,
    pub modeled_chain: Option<serde_json::Value>,
    pub estimated_monthly_chain_fees_lamports: Option<u64>,
    pub estimated_monthly_total_minor_units: Option<u64>,
    pub estimated_total_minor_units_per_game: Option<f64>,
    pub modeled_peak_reserved_capital_lamports: Option<u64>,
    pub observed: ObservedStage,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SystemReport {
    pub schema_version: u32,
    pub kind: String,
    pub scenario: String,
    pub workload_fingerprint: String,
    pub backend_origin: String,
    pub max_passing_observed_concurrency: Option<u32>,
    pub stages: Vec<StageReport>,
    pub planned_stages: Vec<super::scenario::Stage>,
    pub configured_pricing: serde_json::Value,
    pub planned_chain: Option<serde_json::Value>,
    pub warnings: Vec<String>,
}

pub fn fingerprint(scenario: &Scenario) -> Result<String> {
    let input = serde_json::json!({"backend":scenario.backend_url,"stages":scenario.stages,
        "workflow":scenario.workflow,"variables":scenario.variables,"actors":scenario.actors,
        "telemetry":scenario.telemetry,"baseline_ms":scenario.baseline_ms,"sampling_ms":scenario.sampling_ms,
        "drain_ms":scenario.drain_ms,"timeout_ms":scenario.timeout_ms,
        "sends_transactions":scenario.sends_transactions,"transaction_metadata":scenario.transaction_metadata});
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&input)?)))
}

fn percentile(mut values: Vec<f64>, percentile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    Some(
        values[((values.len() as f64 * percentile).ceil() as usize)
            .saturating_sub(1)
            .min(values.len() - 1)],
    )
}

fn chain(scenario: &Scenario, games: u32) -> Result<Option<crate::cost::estimate::Estimate>> {
    scenario
        .chain
        .as_ref()
        .map(|model| {
            let mut variant: crate::cost::estimate::GameVariant =
                serde_json::from_value(serde_json::to_value(&model.variant)?)?;
            variant.games = games;
            crate::cost::estimate::estimate(&variant, model.fee_profile)
        })
        .transpose()
}

pub fn build(scenario: &Scenario, observations: Vec<ObservedStage>) -> Result<SystemReport> {
    let mut report = SystemReport { schema_version:1,kind:if observations.is_empty() { "plan" } else { "measurement" }.into(),
        scenario:scenario.name.clone(),workload_fingerprint:fingerprint(scenario)?,
        backend_origin:super::scenario::validate_url(&scenario.backend_url)?.origin().ascii_serialization(),
        max_passing_observed_concurrency:None,stages:Vec::new(),planned_stages:scenario.stages.clone(),
        configured_pricing:serde_json::to_value(&scenario.pricing)?,
        planned_chain:chain(scenario,scenario.pricing.monthly_games)?.map(serde_json::to_value).transpose()?,
        warnings:vec![
            "Prices are user-configured assumptions; currency uses two decimal minor units. Wagers and prizes are separate cashflows.".into(),
            "Each stage is an independent workload experiment; stage game counts are not pooled into one forecast.".into(),
            "Server counters include background work and other traffic. Baseline subtraction estimates attribution; use an isolated target and a separate load-generator host.".into(),
            "Monthly usage assumes the observed workload mix and stable background rates. Capacity is proven only against configured limits at observed concurrency; higher concurrency is untested.".into(),
            "HTTP body bytes exclude headers, TLS and transport overhead; use server network counters for bandwidth billing.".into(),
            "Chain forecasts use the supplied fee profile and lifecycle model. Captured transaction fees and CU remain separate measurements; unrecorded transactions and missing deposit/refund observations are outside their coverage.".into(),
            "Capital events are declared reservations, not measured wallet transfers. Chain funding totals are cumulative before refunds, not peak capital.".into(),
        ] };
    if !observations.is_empty() {
        ensure!(
            observations.len() == scenario.stages.len(),
            "observation stage count differs from scenario"
        );
    }
    for (observed, configured) in observations.into_iter().zip(&scenario.stages) {
        for samples in observed
            .samples
            .values()
            .chain(observed.baseline_samples.values())
        {
            ensure!(
                samples
                    .windows(2)
                    .all(|window| window[0].elapsed_ms <= window[1].elapsed_ms)
                    && samples
                        .iter()
                        .all(|sample| sample.values.values().all(|value| value.is_finite())),
                "invalid telemetry samples"
            );
        }
        ensure!(
            observed.name == configured.name
                && observed.requested_games == configured.games
                && observed.requested_concurrency == configured.concurrency,
            "observation workload differs from scenario"
        );
        ensure!(
            observed.completed_games <= configured.games
                && observed.peak_concurrency <= configured.concurrency,
            "invalid observed game counts"
        );
        let latencies: Vec<_> = observed.attempts.iter().map(|a| a.latency_ms).collect();
        ensure!(
            observed.attempts.iter().all(|a| a.latency_ms.is_finite()
                && a.latency_ms >= 0.0
                && a.schedule_lag_ms.is_finite()
                && a.schedule_lag_ms >= 0.0
                && a.game_index < configured.games),
            "invalid request observations"
        );
        let p95 = percentile(latencies.clone(), 0.95);
        let lag = percentile(
            observed
                .attempts
                .iter()
                .map(|a| a.schedule_lag_ms)
                .collect(),
            0.95,
        );
        let errors = observed
            .attempts
            .iter()
            .filter(|a| a.error.is_some())
            .count();
        let error_ratio =
            (!observed.attempts.is_empty()).then(|| errors as f64 / observed.attempts.len() as f64);
        let metrics: BTreeMap<_, _> = scenario
            .telemetry
            .iter()
            .map(|metric| {
                let samples = observed
                    .samples
                    .get(&metric.name)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let baseline = observed
                    .baseline_samples
                    .get(&metric.name)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                (
                    metric.name.clone(),
                    collectors::summarize(metric, samples, baseline),
                )
            })
            .collect();
        let mut reasons = Vec::new();
        let mut failed = false;
        let mut incomplete = false;
        if observed.completed_games != configured.games {
            failed = true;
            reasons.push("games did not all complete".into());
        }
        if observed.cleanup_failed_games > 0 {
            failed = true;
            reasons.push("cleanup failed".into());
        }
        if observed.streams.errors > 0 {
            failed = true;
            reasons.push("spectator streams failed".into());
        }
        if p95.is_none_or(|n| n > scenario.limits.request_p95_ms) {
            failed = true;
            reasons.push("request p95 exceeded limit or was unavailable".into());
        }
        if error_ratio.is_none_or(|n| n > scenario.limits.error_ratio) {
            failed = true;
            reasons.push("attempt error ratio exceeded limit or was unavailable".into());
        }
        if lag.is_some_and(|n| n > scenario.limits.max_schedule_lag_ms) {
            failed = true;
            reasons.push("move schedule fell behind".into());
        }
        if observed.duration_ms < scenario.limits.min_duration_ms {
            incomplete = true;
            reasons.push("run was shorter than the required observation window".into());
        }
        if observed.peak_concurrency < configured.concurrency {
            incomplete = true;
            reasons.push("requested concurrency was not reached".into());
        }
        for metric in &scenario.telemetry {
            let summary = &metrics[&metric.name];
            if !summary.complete {
                if metric.required || metric.min.is_some() || metric.max.is_some() {
                    incomplete = true;
                    reasons.push(format!("{} telemetry is incomplete", metric.name));
                }
                continue;
            }
            let value_max = summary.max.or(summary.delta);
            let value_min = summary.min.or(summary.delta);
            if metric
                .max
                .zip(value_max)
                .is_some_and(|(limit, value)| value > limit)
                || metric
                    .min
                    .zip(value_min)
                    .is_some_and(|(limit, value)| value < limit)
            {
                failed = true;
                reasons.push(format!("{} exceeded its resource limit", metric.name));
            }
        }
        let capacity = if failed {
            Capacity::Fail
        } else if incomplete {
            Capacity::Inconclusive
        } else {
            Capacity::Pass
        };
        if capacity == Capacity::Pass {
            report.max_passing_observed_concurrency = Some(
                report
                    .max_passing_observed_concurrency
                    .unwrap_or(0)
                    .max(observed.peak_concurrency),
            );
        }
        let operating_costs = pricing::quote(
            &scenario.pricing,
            &metrics,
            observed.completed_games,
            observed.duration_ms,
        )?;
        let modeled = chain(scenario, configured.games)?;
        let monthly_chain = chain(scenario, scenario.pricing.monthly_games)?;
        let monthly_fees = monthly_chain
            .as_ref()
            .map(|c| c.nonrefundable_fees_lamports)
            .or_else(|| (!scenario.sends_transactions).then_some(0));
        let chain_money = match (monthly_fees, scenario.pricing.sol_price_minor_units) {
            (Some(0), _) => Some(0),
            (Some(fees), Some(price)) => Some(pricing::lamports_to_money(fees, price)?),
            _ => None,
        };
        let monthly_total = operating_costs
            .infrastructure_monthly_minor_units
            .zip(chain_money)
            .map(|(a, b)| a.checked_add(b).context("combined cost overflow"))
            .transpose()?
            .filter(|_| capacity == Capacity::Pass);
        let mut fee_total = 0u64;
        let mut cu_total = 0u64;
        let mut measured = 0;
        let mut signatures = std::collections::BTreeSet::new();
        for tx in &observed.transactions {
            if let Some(sig) = &tx.signature {
                if !signatures.insert(sig) {
                    continue;
                }
            } else {
                continue;
            }
            if let Some(fee) = tx.fee_lamports {
                fee_total = fee_total.checked_add(fee).context("fee sum overflow")?;
                measured += 1;
            }
            if let Some(cu) = tx.cu_consumed {
                cu_total = cu_total.checked_add(cu).context("CU sum overflow")?;
            }
        }
        let mut reserved = 0i128;
        let mut peak = 0i128;
        let mut capital_complete = !observed.capital.is_empty();
        let mut capital_events: Vec<_> = observed.capital.iter().collect();
        capital_events.sort_by_key(|event| (event.elapsed_ms, event.delta_lamports < 0));
        for event in capital_events {
            reserved += i128::from(event.delta_lamports);
            peak = peak.max(reserved);
            capital_complete &= reserved >= 0;
        }
        if measured < signatures.len() {
            reasons.push("captured transaction metadata coverage is partial".into());
        }
        report.stages.push(StageReport {
            capacity,
            reasons,
            request_p50_ms: percentile(latencies.clone(), 0.5),
            request_p95_ms: p95,
            request_p99_ms: percentile(latencies, 0.99),
            schedule_lag_p95_ms: lag,
            attempt_error_ratio: error_ratio,
            completed_games_per_second: if observed.duration_ms > 0 {
                f64::from(observed.completed_games) * 1000.0 / observed.duration_ms as f64
            } else {
                0.0
            },
            metrics,
            operating_costs,
            measured_transaction_fees_lamports: fee_total,
            measured_cu: cu_total,
            recorded_transactions: signatures.len(),
            measured_transactions: measured,
            modeled_chain: modeled.map(serde_json::to_value).transpose()?,
            estimated_monthly_chain_fees_lamports: monthly_fees,
            estimated_monthly_total_minor_units: monthly_total,
            estimated_total_minor_units_per_game: monthly_total
                .map(|n| n as f64 / f64::from(scenario.pricing.monthly_games)),
            modeled_peak_reserved_capital_lamports: if capital_complete {
                Some(u64::try_from(peak)?)
            } else {
                None
            },
            observed,
        });
    }
    Ok(report)
}
