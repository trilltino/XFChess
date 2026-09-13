pub mod read_load;
pub mod stream;
pub mod tx_land;

#[cfg(feature = "geyser")]
pub mod geyser;

#[derive(Default, Clone)]
pub struct LatencyStats {
    pub samples: Vec<f64>,
    pub errors: u64,
    pub throttled: u64,
}

impl LatencyStats {
    pub fn record_ms(&mut self, ms: f64) {
        self.samples.push(ms);
    }

    pub fn record_error(&mut self) {
        self.errors += 1;
    }

    pub fn record_throttle(&mut self) {
        self.throttled += 1;
    }

    pub fn merge(&mut self, other: LatencyStats) {
        self.samples.extend(other.samples);
        self.errors += other.errors;
        self.throttled += other.throttled;
    }

    pub fn ok(&self) -> usize {
        self.samples.len()
    }

    pub fn total(&self) -> u64 {
        self.samples.len() as u64 + self.errors + self.throttled
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn min(&self) -> f64 {
        self.samples
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
            .max(0.0)
    }

    pub fn max(&self) -> f64 {
        self.samples.iter().cloned().fold(0.0, f64::max)
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let rank = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[rank.min(sorted.len() - 1)]
    }
}

pub fn redact_url(url: &str) -> String {
    if let Some(idx) = url.rfind('/') {
        let (head, tail) = url.split_at(idx + 1);
        if tail.len() >= 16 {
            return format!("{head}***");
        }
    }
    url.to_string()
}
