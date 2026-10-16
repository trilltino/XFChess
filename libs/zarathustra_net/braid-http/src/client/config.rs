#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientConfig {
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub connection_timeout_secs: u64,
    pub enable_logging: bool,
    pub proxy_url: String,
    pub request_timeout_ms: u64,
    pub max_idle_connections_per_host: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        ClientConfig {
            max_retries: 3,
            retry_delay_ms: 1000,
            connection_timeout_secs: 30,
            enable_logging: false,
            proxy_url: String::new(),
            request_timeout_ms: 30000,
            max_idle_connections_per_host: 100,
        }
    }
}
