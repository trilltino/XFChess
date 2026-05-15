#![allow(dead_code)]
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct AuthState {
  pub token: Option<String>,
  pub backend_url: String,
  pub is_authenticated: bool,
}

impl Default for AuthState {
  fn default() -> Self {
    Self {
      token: None,
      backend_url: "http://127.0.0.1:8090".to_string(),
      is_authenticated: false,
    }
  }
}

impl AuthState {
  pub fn new() -> Arc<Mutex<Self>> {
    Arc::new(Mutex::new(Self::default()))
  }

  pub fn set_token(&mut self, token: String) {
    self.token = Some(token);
    self.is_authenticated = true;
  }

  pub fn clear_token(&mut self) {
    self.token = None;
    self.is_authenticated = false;
  }

  pub fn set_backend_url(&mut self, url: String) {
    self.backend_url = url;
  }
}
