pub mod ai;

pub use ai::*;

#[derive(Debug, Clone)]
pub struct AiService {
    pub name: String,
    pub version: String,
}

impl AiService {
    pub fn new() -> Self {
        Self {
            name: "XFChess AI Service".to_string(),
            version: "0.1.0".to_string(),
        }
    }
}

impl Default for AiService {
    fn default() -> Self {
        Self::new()
    }
}
