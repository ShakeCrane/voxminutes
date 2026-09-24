//! OpenAI-compatible loopback ASR provider for user-run inference servers.
use async_trait::async_trait;
use super::provider::{TranscriptionError, TranscriptionProvider, TranscriptResult};
use crate::custom_local::{self, LocalModelProfile};

pub struct CustomLocalAsrProvider {
    profile: LocalModelProfile,
}
impl CustomLocalAsrProvider {
    pub fn new(profile: LocalModelProfile) -> Self { Self { profile } }
}
#[async_trait]
impl TranscriptionProvider for CustomLocalAsrProvider {
    async fn transcribe(&self, audio: Vec<f32>, language: Option<String>)
        -> Result<TranscriptResult, TranscriptionError> {
        let text = custom_local::transcribe(&self.profile, &audio, language)
            .await.map_err(TranscriptionError::EngineFailed)?;
        Ok(TranscriptResult { text, confidence: None, is_partial: false })
    }
    async fn is_model_loaded(&self) -> bool { true }
    async fn get_current_model(&self) -> Option<String> { Some(self.profile.name.clone()) }
    fn provider_name(&self) -> &'static str { "Custom local ASR" }
}
