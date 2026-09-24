#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoneyFlowState {
    Idle,
    Signing {
        action: String,
    },
    Submitted {
        action: String,
        signature: String,
    },
    PendingReconciliation {
        action: String,
        signature: Option<String>,
        reason: String,
    },
    Confirmed {
        action: String,
        signature: Option<String>,
    },
    Failed {
        action: String,
        reason: String,
    },
    NeedsAdminReview {
        action: String,
        signature: Option<String>,
        reason: String,
    },
}

impl Default for MoneyFlowState {
    fn default() -> Self {
        Self::Idle
    }
}

impl MoneyFlowState {
    pub fn signing(action: impl Into<String>) -> Self {
        Self::Signing {
            action: action.into(),
        }
    }

    pub fn submitted(action: impl Into<String>, signature: impl ToString) -> Self {
        Self::Submitted {
            action: action.into(),
            signature: signature.to_string(),
        }
    }

    pub fn pending(
        action: impl Into<String>,
        signature: Option<impl ToString>,
        reason: impl Into<String>,
    ) -> Self {
        Self::PendingReconciliation {
            action: action.into(),
            signature: signature.map(|s| s.to_string()),
            reason: reason.into(),
        }
    }

    pub fn confirmed(action: impl Into<String>, signature: Option<impl ToString>) -> Self {
        Self::Confirmed {
            action: action.into(),
            signature: signature.map(|s| s.to_string()),
        }
    }

    pub fn failed(action: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Failed {
            action: action.into(),
            reason: reason.into(),
        }
    }

    pub fn admin_review(
        action: impl Into<String>,
        signature: Option<impl ToString>,
        reason: impl Into<String>,
    ) -> Self {
        Self::NeedsAdminReview {
            action: action.into(),
            signature: signature.map(|s| s.to_string()),
            reason: reason.into(),
        }
    }

    pub fn status(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Signing { .. } => "signing",
            Self::Submitted { .. } => "submitted",
            Self::PendingReconciliation { .. } => "pending_reconciliation",
            Self::Confirmed { .. } => "confirmed",
            Self::Failed { .. } => "failed",
            Self::NeedsAdminReview { .. } => "needs_admin_review",
        }
    }

    pub fn action(&self) -> Option<&str> {
        match self {
            Self::Idle => None,
            Self::Signing { action }
            | Self::Submitted { action, .. }
            | Self::PendingReconciliation { action, .. }
            | Self::Confirmed { action, .. }
            | Self::Failed { action, .. }
            | Self::NeedsAdminReview { action, .. } => Some(action),
        }
    }

    pub fn signature(&self) -> Option<&str> {
        match self {
            Self::Submitted { signature, .. } => Some(signature),
            Self::PendingReconciliation { signature, .. }
            | Self::Confirmed { signature, .. }
            | Self::NeedsAdminReview { signature, .. } => signature.as_deref(),
            _ => None,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::PendingReconciliation { reason, .. }
            | Self::Failed { reason, .. }
            | Self::NeedsAdminReview { reason, .. } => Some(reason),
            _ => None,
        }
    }
}
