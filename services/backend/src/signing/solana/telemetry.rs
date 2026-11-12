#[derive(Debug, Clone)]
pub enum TxErrorCategory {
    // Client errors (fix the request)
    InvalidInstruction,
    InsufficientFunds,
    AccountNotFound,
    ProgramError(u32),

    // Server/transient errors (retry)
    RpcTimeout,
    RpcRateLimit,
    BlockhashExpired,

    // Infrastructure errors (alert ops)
    FeePayerExhausted,
    RpcUnavailable,
    Unknown,
}

impl std::fmt::Display for TxErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxErrorCategory::InvalidInstruction => write!(f, "InvalidInstruction"),
            TxErrorCategory::InsufficientFunds => write!(f, "InsufficientFunds"),
            TxErrorCategory::AccountNotFound => write!(f, "AccountNotFound"),
            TxErrorCategory::ProgramError(code) => write!(f, "ProgramError({})", code),
            TxErrorCategory::RpcTimeout => write!(f, "RpcTimeout"),
            TxErrorCategory::RpcRateLimit => write!(f, "RpcRateLimit"),
            TxErrorCategory::BlockhashExpired => write!(f, "BlockhashExpired"),
            TxErrorCategory::FeePayerExhausted => write!(f, "FeePayerExhausted"),
            TxErrorCategory::RpcUnavailable => write!(f, "RpcUnavailable"),
            TxErrorCategory::Unknown => write!(f, "Unknown"),
        }
    }
}

pub fn classify_error_str(error_str: &str) -> TxErrorCategory {
    if error_str.contains("insufficient funds") {
        return TxErrorCategory::InsufficientFunds;
    }
    if error_str.contains("blockhash not found") || error_str.contains("Blockhash not found") {
        return TxErrorCategory::BlockhashExpired;
    }
    if error_str.contains("rate limit") || error_str.contains("429") {
        return TxErrorCategory::RpcRateLimit;
    }
    if error_str.contains("timeout") {
        return TxErrorCategory::RpcTimeout;
    }
    if error_str.contains("unavailable") || error_str.contains("connection") {
        return TxErrorCategory::RpcUnavailable;
    }
    TxErrorCategory::Unknown
}
