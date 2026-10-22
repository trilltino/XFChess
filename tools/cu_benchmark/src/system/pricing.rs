use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    collectors::MetricSummary,
    scenario::{Metric, MetricKind},
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pricing {
    #[serde(default)]
    pub configured: bool,
    pub currency: String,
    pub monthly_games: u32,
    #[serde(default = "month_seconds")]
    pub billing_period_seconds: u64,
    #[serde(default)]
    pub fixed: Vec<FixedCost>,
    #[serde(default)]
    pub metered: Vec<MeteredCost>,
    #[serde(default)]
    pub exchange_rates_millionths: BTreeMap<String, u64>,
    #[serde(default)]
    pub prepaid: Vec<PrepaidCredit>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    pub sol_price_minor_units: Option<u64>,
}

fn month_seconds() -> u64 {
    30 * 24 * 60 * 60
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixedCost {
    pub name: String,
    pub monthly_minor_units: u64,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrepaidCredit {
    pub provider: String,
    pub currency: String,
    pub deposit_minor_units: u64,
    pub validity_months: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeteredCost {
    pub name: String,
    pub currency: Option<String>,
    pub metric: String,
    pub quantity_per_unit: u64,
    pub minor_units_per_unit: u64,
    #[serde(default)]
    pub included_monthly_quantity: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PricedUsage {
    pub name: String,
    pub currency: String,
    pub measured_run_quantity: Option<f64>,
    pub projected_monthly_quantity: Option<f64>,
    pub run_cost_at_list_rate_minor_units: Option<u64>,
    pub monthly_overage_minor_units: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OperatingCosts {
    pub configured: bool,
    pub currency: String,
    pub fixed_monthly_minor_units: Option<u64>,
    pub metered: Vec<PricedUsage>,
    pub allocated_fixed_run_minor_units: Option<u64>,
    pub infrastructure_monthly_minor_units: Option<u64>,
    pub completed_games: u32,
    pub monthly_games: u32,
    pub prepaid: Vec<PrepaidCredit>,
}

impl Pricing {
    pub fn validate(&self, metrics: &[Metric]) -> Result<()> {
        ensure!(!self.currency.is_empty(), "pricing currency is required");
        ensure!(
            self.exchange_rates_millionths
                .values()
                .all(|rate| *rate > 0),
            "exchange rates must be positive"
        );
        ensure!(
            self.prepaid
                .iter()
                .all(|credit| !credit.currency.is_empty() && credit.validity_months > 0),
            "invalid prepaid credit"
        );
        ensure!(
            self.monthly_games > 0 && self.billing_period_seconds > 0,
            "billing period and monthly games must be positive"
        );
        for cost in &self.metered {
            ensure!(
                cost.quantity_per_unit > 0,
                "metered billing unit must be positive"
            );
            let metric = metrics
                .iter()
                .find(|metric| metric.name == cost.metric)
                .context("metered cost references an unknown telemetry metric")?;
            ensure!(
                metric.kind != MetricKind::CpuUtilization,
                "CPU utilization cannot be used as a billable quantity"
            );
        }
        self.fixed.iter().try_fold(0u64, |sum, cost| {
            sum.checked_add(cost.monthly_minor_units)
                .context("fixed cost overflow")
        })?;
        Ok(())
    }
}

fn convert(amount: u64, currency: &str, pricing: &Pricing) -> Result<Option<u64>> {
    if currency == pricing.currency {
        return Ok(Some(amount));
    }
    pricing
        .exchange_rates_millionths
        .get(currency)
        .map(|rate| {
            u64::try_from((u128::from(amount) * u128::from(*rate)).div_ceil(1_000_000))
                .context("currency conversion overflow")
        })
        .transpose()
}

fn money(quantity: f64, cost: &MeteredCost) -> Result<u64> {
    ensure!(
        quantity.is_finite() && quantity >= 0.0 && quantity < u64::MAX as f64 / 1_000_000.0,
        "usage quantity is out of range"
    );
    let micro_quantity = (quantity * 1_000_000.0).ceil() as u64;
    let numerator = u128::from(micro_quantity) * u128::from(cost.minor_units_per_unit);
    let denominator = u128::from(cost.quantity_per_unit) * 1_000_000;
    u64::try_from(numerator.div_ceil(denominator)).context("metered cost overflow")
}

pub fn quote(
    pricing: &Pricing,
    metrics: &BTreeMap<String, MetricSummary>,
    completed: u32,
    duration_ms: u64,
) -> Result<OperatingCosts> {
    let fixed = pricing
        .fixed
        .iter()
        .try_fold(Some(0u64), |sum, cost| -> Result<_> {
            let amount = convert(
                cost.monthly_minor_units,
                cost.currency.as_deref().unwrap_or(&pricing.currency),
                pricing,
            )?;
            sum.zip(amount)
                .map(|(sum, amount)| sum.checked_add(amount).context("fixed cost overflow"))
                .transpose()
        })?;
    let allocated = fixed
        .map(|fixed| {
            u64::try_from(
                (u128::from(fixed) * u128::from(duration_ms))
                    .div_ceil(u128::from(pricing.billing_period_seconds) * 1000),
            )
        })
        .transpose()?;
    let mut monthly = pricing.configured.then_some(fixed).flatten();
    let mut metered = Vec::new();
    for cost in &pricing.metered {
        let metric = metrics.get(&cost.metric);
        let run_quantity = metric.filter(|m| m.complete).and_then(|m| m.delta);
        let projected = metric
            .filter(|m| m.complete)
            .and_then(|m| m.above_baseline_delta.zip(m.baseline_rate_per_second))
            .filter(|_| completed > 0)
            .map(|(incremental, rate)| {
                metric
                    .filter(|m| m.delta.is_some() && m.first.is_some())
                    .and_then(|m| m.last)
                    .unwrap_or(0.0)
                    + incremental * f64::from(pricing.monthly_games) / f64::from(completed)
                    + rate * pricing.billing_period_seconds as f64
            });
        let run_cost = run_quantity.map(|q| money(q, cost)).transpose()?;
        let monthly_cost = projected
            .map(|q| money((q - cost.included_monthly_quantity as f64).max(0.0), cost))
            .transpose()?;
        let currency = cost.currency.as_deref().unwrap_or(&pricing.currency);
        let converted = monthly_cost
            .map(|amount| convert(amount, currency, pricing))
            .transpose()?
            .flatten();
        monthly = match (monthly, converted) {
            (Some(sum), Some(value)) => {
                Some(sum.checked_add(value).context("monthly cost overflow")?)
            }
            _ => None,
        };
        metered.push(PricedUsage {
            name: cost.name.clone(),
            currency: currency.into(),
            measured_run_quantity: run_quantity,
            projected_monthly_quantity: projected,
            run_cost_at_list_rate_minor_units: run_cost,
            monthly_overage_minor_units: monthly_cost,
        });
    }
    Ok(OperatingCosts {
        configured: pricing.configured,
        currency: pricing.currency.clone(),
        fixed_monthly_minor_units: pricing.configured.then_some(fixed).flatten(),
        metered,
        allocated_fixed_run_minor_units: pricing.configured.then_some(allocated).flatten(),
        infrastructure_monthly_minor_units: monthly,
        completed_games: completed,
        monthly_games: pricing.monthly_games,
        prepaid: pricing.prepaid.clone(),
    })
}

pub fn lamports_to_money(lamports: u64, price_minor_units: u64) -> Result<u64> {
    u64::try_from((u128::from(lamports) * u128::from(price_minor_units)).div_ceil(1_000_000_000))
        .context("SOL conversion overflow")
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    #[test]
    fn monthly_fixed_cost_is_once_and_included_usage_is_applied_monthly() -> anyhow::Result<()> {
        let pricing: Pricing = serde_json::from_value(serde_json::json!({
            "configured":true,"currency":"GBP","monthly_games":100,"billing_period_seconds":1000,
            "fixed":[{"name":"VPS","monthly_minor_units":2000}],
            "metered":[{"name":"RPC","metric":"rpc","quantity_per_unit":100,
                "minor_units_per_unit":10,"included_monthly_quantity":500}]
        }))
        .context(concat!(file!(), ":", line!()))?;
        let metric = MetricSummary {
            complete: true,
            delta: Some(100.0),
            above_baseline_delta: Some(80.0),
            baseline_rate_per_second: Some(2.0),
            ..MetricSummary::default()
        };
        let result = quote(
            &pricing,
            &BTreeMap::from([("rpc".into(), metric)]),
            10,
            10_000,
        )
        .context(concat!(file!(), ":", line!()))?;
        assert_eq!(result.fixed_monthly_minor_units, Some(2000));
        assert_eq!(result.allocated_fixed_run_minor_units, Some(20));
        assert_eq!(result.metered[0].monthly_overage_minor_units, Some(230));
        assert_eq!(result.infrastructure_monthly_minor_units, Some(2230));
        Ok(())
    }
    #[test]
    fn unavailable_usage_and_zero_completions_do_not_become_zero_costs() -> anyhow::Result<()> {
        let pricing: Pricing = serde_json::from_value(serde_json::json!({"currency":"GBP","monthly_games":100,
            "metered":[{"name":"RPC","metric":"rpc","quantity_per_unit":1,"minor_units_per_unit":1}]})).context(concat!(file!(), ":", line!()))?;
        assert!(quote(&pricing, &BTreeMap::new(), 0, 1)
            .context(concat!(file!(), ":", line!()))?
            .infrastructure_monthly_minor_units
            .is_none());
        assert_eq!(
            lamports_to_money(1, 6000).context(concat!(file!(), ":", line!()))?,
            1
        );
        assert!(lamports_to_money(u64::MAX, u64::MAX).is_err());
        Ok(())
    }

    #[test]
    fn currencies_need_an_explicit_rate_and_prepaid_credit_is_separate() -> anyhow::Result<()> {
        let mut pricing: Pricing = serde_json::from_value(serde_json::json!({
            "configured":true,"currency":"EUR","monthly_games":100,
            "fixed":[{"name":"VPS","monthly_minor_units":599}],
            "metered":[{"name":"Triton","currency":"USD","metric":"rpc","quantity_per_unit":1000000,"minor_units_per_unit":1000}],
            "prepaid":[{"provider":"Triton","currency":"USD","deposit_minor_units":12500,"validity_months":12}]
        })).context(concat!(file!(), ":", line!()))?;
        let metrics = BTreeMap::from([(
            "rpc".into(),
            MetricSummary {
                complete: true,
                delta: Some(1_000_000.0),
                above_baseline_delta: Some(1_000_000.0),
                baseline_rate_per_second: Some(0.0),
                ..MetricSummary::default()
            },
        )]);
        let result =
            quote(&pricing, &metrics, 100, 1000).context(concat!(file!(), ":", line!()))?;
        assert_eq!(result.infrastructure_monthly_minor_units, None);
        assert_eq!(result.metered[0].monthly_overage_minor_units, Some(1000));
        assert_eq!(result.metered[0].currency, "USD");
        pricing
            .exchange_rates_millionths
            .insert("USD".into(), 900_000);
        let result =
            quote(&pricing, &metrics, 100, 1000).context(concat!(file!(), ":", line!()))?;
        assert_eq!(result.infrastructure_monthly_minor_units, Some(1499));
        assert_eq!(result.prepaid[0].deposit_minor_units, 12500);
        Ok(())
    }
}
