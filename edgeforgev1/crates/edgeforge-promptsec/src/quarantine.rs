// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
use async_trait::async_trait;
use edgeforge_core::Prompt;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SanitizedSummary {
    pub text: String,
    pub confidence: f32,
}
#[async_trait]
pub trait QuarantineReader: Send + Sync + 'static {
    async fn read_and_summarize(
        &self,
        resource: &str,
        intent: &str,
    ) -> Result<SanitizedSummary, QuarantineError>;
}
#[derive(Debug, thiserror::Error)]
pub enum QuarantineError {
    #[error("unreadable: {0}")]
    Unreadable(String),
    #[error("reader failed: {0}")]
    ReaderFailed(String),
    #[error("suspicious content")]
    SuspiciousContent,
    #[error("low confidence: {0}")]
    LowConfidence(f32),
}
impl From<QuarantineError> for edgeforge_core::EdgeForgeError {
    fn from(e: QuarantineError) -> Self {
        Self::SandboxDenied(e.to_string())
    }
}
pub struct DefaultQuarantineReader;
impl DefaultQuarantineReader {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl QuarantineReader for DefaultQuarantineReader {
    async fn read_and_summarize(
        &self,
        resource: &str,
        _intent: &str,
    ) -> Result<SanitizedSummary, QuarantineError> {
        if resource.starts_with("file://") {
            let path = resource.trim_start_matches("file://");
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| QuarantineError::Unreadable(e.to_string()))?;
            Ok(SanitizedSummary {
                text: content.chars().take(200).collect(),
                confidence: 1.0,
            })
        } else {
            Err(QuarantineError::Unreadable("unsupported scheme".into()))
        }
    }
}
