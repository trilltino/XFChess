use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::scenario::{Metric, MetricKind};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Sample {
    pub elapsed_ms: u64,
    pub values: BTreeMap<String, f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MetricSummary {
    pub samples: usize,
    pub observed_samples: usize,
    pub scrape_errors: usize,
    pub counter_resets: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub delta: Option<f64>,
    pub last: Option<f64>,
    pub first: Option<f64>,
    pub baseline_rate_per_second: Option<f64>,
    pub above_baseline_delta: Option<f64>,
    pub complete: bool,
}

#[derive(Debug)]
struct ParsedSample {
    name: String,
    labels: BTreeMap<String, String>,
    value: f64,
}

fn split_unquoted(text: &str, separator: impl Fn(char) -> bool) -> Vec<&str> {
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    let mut parts = Vec::new();
    for (index, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && c == '\\' {
            escaped = true;
            continue;
        }
        if c == '"' {
            quoted = !quoted;
        }
        if !quoted && separator(c) {
            parts.push(&text[start..index]);
            start = index + c.len_utf8();
        }
    }
    parts.push(&text[start..]);
    parts
}

fn parse(text: &str) -> Result<Vec<ParsedSample>> {
    let mut result = Vec::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
    {
        let mut quoted = false;
        let mut escaped = false;
        let mut braces = 0i32;
        let split = line
            .char_indices()
            .find_map(|(index, c)| {
                if escaped {
                    escaped = false;
                    return None;
                }
                if quoted && c == '\\' {
                    escaped = true;
                    return None;
                }
                if c == '"' {
                    quoted = !quoted;
                }
                if !quoted {
                    if c == '{' {
                        braces += 1;
                    } else if c == '}' {
                        braces -= 1;
                    }
                }
                (!quoted && braces == 0 && c.is_whitespace()).then_some(index)
            })
            .context("missing metric value")?;
        let key = &line[..split];
        let value = line[split..]
            .split_whitespace()
            .next()
            .context("missing metric value")?
            .parse::<f64>()?;
        let (name, labels) = if let Some((name, rest)) = key.split_once('{') {
            let rest = rest.strip_suffix('}').context("unclosed metric labels")?;
            let mut labels = BTreeMap::new();
            for pair in split_unquoted(rest, |c| c == ',')
                .into_iter()
                .filter(|s| !s.trim().is_empty())
            {
                let (name, value) = pair.split_once('=').context("invalid metric label")?;
                let value: String = serde_json::from_str(value.trim())?;
                if labels.insert(name.trim().to_owned(), value).is_some() {
                    bail!("duplicate metric label");
                }
            }
            (name.to_owned(), labels)
        } else {
            (key.to_owned(), BTreeMap::new())
        };
        result.push(ParsedSample {
            name,
            labels,
            value,
        });
    }
    Ok(result)
}

pub async fn sample(
    client: &reqwest::Client,
    metrics: &[Metric],
    elapsed_ms: u64,
) -> BTreeMap<String, Sample> {
    let mut responses = BTreeMap::new();
    for metric in metrics {
        let key = (&metric.url, &metric.bearer_env);
        if responses.contains_key(&key) {
            continue;
        }
        let fetch = async {
            let mut request = client
                .get(&metric.url)
                .header("Accept", "text/plain;version=0.0.4");
            if let Some(name) = &metric.bearer_env {
                request =
                    request.bearer_auth(std::env::var(name).context("missing telemetry token")?);
            }
            let response = request.send().await?.error_for_status()?;
            let text = response.text().await?;
            parse(&text)
        }
        .await;
        responses.insert(key, fetch);
    }
    metrics
        .iter()
        .map(|metric| {
            let mut values = BTreeMap::new();
            let mut error = None;
            match &responses[&(&metric.url, &metric.bearer_env)] {
                Ok(parsed) => {
                    for sample in parsed.iter().filter(|sample| {
                        sample.name == metric.metric
                            && metric
                                .labels
                                .iter()
                                .all(|(k, v)| sample.labels.get(k) == Some(v))
                    }) {
                        if !sample.value.is_finite()
                            || (metric.kind != MetricKind::Gauge && sample.value < 0.0)
                        {
                            error = Some("metric contains a nonfinite value".into());
                            continue;
                        }
                        let identity =
                            serde_json::to_string(&sample.labels).expect("string map serializes");
                        if values.insert(identity, sample.value).is_some() {
                            error = Some("duplicate metric series".into());
                        }
                    }
                }
                Err(_) => error = Some("telemetry fetch or parse failed".into()),
            }
            (
                metric.name.clone(),
                Sample {
                    elapsed_ms,
                    values,
                    error,
                },
            )
        })
        .collect()
}

pub fn summarize(metric: &Metric, samples: &[Sample], baseline: &[Sample]) -> MetricSummary {
    let mut result = summary(metric.kind, samples);
    let idle = summary(metric.kind, baseline);
    if metric.kind != MetricKind::CpuUtilization {
        if let Some(seconds) = baseline
            .last()
            .zip(baseline.first())
            .map(|(last, first)| last.elapsed_ms.saturating_sub(first.elapsed_ms) as f64 / 1000.0)
            .filter(|n| *n > 0.0)
        {
            result.baseline_rate_per_second =
                idle.delta.filter(|_| idle.complete).map(|n| n / seconds);
        }
        let seconds = samples
            .last()
            .zip(samples.first())
            .map(|(last, first)| last.elapsed_ms.saturating_sub(first.elapsed_ms) as f64 / 1000.0)
            .unwrap_or(0.0);
        result.above_baseline_delta = result
            .delta
            .zip(result.baseline_rate_per_second)
            .filter(|_| result.complete)
            .map(|(delta, rate)| (delta - rate * seconds).max(0.0));
    }
    result
}

fn summary(kind: MetricKind, samples: &[Sample]) -> MetricSummary {
    let mut result = MetricSummary {
        samples: samples.len(),
        ..MetricSummary::default()
    };
    let mut previous = BTreeMap::<String, f64>::new();
    let mut delta = 0.0;
    let mut cpu_complete = true;
    for (index, sample) in samples.iter().enumerate() {
        result.scrape_errors += usize::from(sample.error.is_some());
        if sample.values.is_empty() {
            continue;
        }
        result.observed_samples += 1;
        let mut total_delta = 0.0;
        let mut idle_delta = 0.0;
        let mut has_idle = false;
        for (key, value) in &sample.values {
            let labels: BTreeMap<String, String> = if kind == MetricKind::CpuUtilization {
                serde_json::from_str(key).unwrap_or_default()
            } else {
                BTreeMap::new()
            };
            let mode = labels.get("mode").map(String::as_str);
            if matches!(mode, Some("guest" | "guest_nice")) {
                continue;
            }
            if kind == MetricKind::CpuUtilization && index > 0 && !previous.contains_key(key) {
                cpu_complete = false;
            }
            let increment = if let Some(prev) = previous.insert(key.clone(), *value) {
                if *value < prev {
                    result.counter_resets += usize::from(kind != MetricKind::Gauge);
                    *value
                } else {
                    *value - prev
                }
            } else if index == 0 {
                0.0
            } else {
                *value
            };
            total_delta += increment;
            if kind == MetricKind::CpuUtilization && mode == Some("idle") {
                has_idle = true;
                idle_delta += increment;
            }
        }
        if kind == MetricKind::CpuUtilization {
            cpu_complete &= has_idle;
        }
        delta += total_delta;
        let value = match kind {
            MetricKind::Counter => continue,
            MetricKind::Gauge => sample.values.values().sum(),
            MetricKind::CpuUtilization if index > 0 && total_delta > 0.0 => {
                (1.0 - idle_delta / total_delta).clamp(0.0, 1.0)
            }
            MetricKind::CpuUtilization => continue,
        };
        result.first.get_or_insert(value);
        result.last = Some(value);
        result.min = Some(result.min.map_or(value, |prev| prev.min(value)));
        result.max = Some(result.max.map_or(value, |prev| prev.max(value)));
    }
    result.complete = samples.len() >= 2
        && result.observed_samples == samples.len()
        && result.scrape_errors == 0
        && result.counter_resets == 0;
    if kind == MetricKind::Gauge {
        result.complete &= result.observed_samples == samples.len();
    }
    if kind == MetricKind::CpuUtilization {
        result.complete &=
            result.max.is_some() && cpu_complete && result.observed_samples == samples.len();
    }
    if kind == MetricKind::Counter && result.complete {
        result.delta = Some(delta);
    }
    if kind == MetricKind::Gauge && result.complete {
        result.delta = result
            .last
            .zip(result.first)
            .map(|(last, first)| (last - first).max(0.0));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(ms: u64, values: &[(&str, f64)]) -> Sample {
        Sample {
            elapsed_ms: ms,
            values: values.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            error: None,
        }
    }
    #[test]
    fn labels_spaces_escapes_and_timestamp() {
        let values = parse(
            "# TYPE calls counter\ncalls{route=\"POST /move,record\",path=\"C:\\\\foo\"} 12 123\n",
        )
        .unwrap();
        assert_eq!(values[0].labels["route"], "POST /move,record");
        assert_eq!(values[0].labels["path"], "C:\\foo");
        assert_eq!(values[0].value, 12.0);
    }
    #[test]
    fn counters_track_new_series_and_detect_individual_resets() {
        let values = [
            point(0, &[("a", 100.0)]),
            point(1000, &[("a", 105.0), ("b", 3.0)]),
        ];
        assert_eq!(summary(MetricKind::Counter, &values).delta, Some(8.0));
        let reset = [values[0].clone(), point(1000, &[("a", 1.0), ("b", 200.0)])];
        let result = summary(MetricKind::Counter, &reset);
        assert_eq!(result.counter_resets, 1);
        assert!(!result.complete);
        assert_eq!(result.delta, None);
    }
    #[test]
    fn missing_metrics_are_unknown_and_cpu_uses_idle_delta() {
        assert!(!summary(MetricKind::Counter, &[point(0, &[]), point(1000, &[])]).complete);
        assert!(
            !summary(
                MetricKind::Counter,
                &[point(0, &[]), point(1000, &[("a", 10.0)])]
            )
            .complete
        );
        let cpu = [
            point(
                0,
                &[(r#"{"mode":"idle"}"#, 100.0), (r#"{"mode":"user"}"#, 20.0)],
            ),
            point(
                1000,
                &[(r#"{"mode":"idle"}"#, 100.7), (r#"{"mode":"user"}"#, 20.3)],
            ),
        ];
        assert!((summary(MetricKind::CpuUtilization, &cpu).max.unwrap() - 0.3).abs() < 1e-9);
    }
}
