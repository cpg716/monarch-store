use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlpmProgressEvent {
    pub event_type: String,
    pub package: Option<String>,
    pub percent: Option<u8>,
    pub downloaded: Option<u64>,
    pub total: Option<u64>,
    pub message: String,
}

impl AlpmProgressEvent {
    #[allow(dead_code)]
    pub fn is_complete(&self) -> bool {
        matches!(
            self.event_type.as_str(),
            "install_complete" | "extract_complete" | "transaction_complete"
        )
    }

    #[allow(dead_code)]
    pub fn is_error(&self) -> bool {
        self.event_type == "error" || self.message.to_lowercase().contains("error")
    }
}
