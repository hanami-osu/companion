use async_trait::async_trait;
use thiserror::Error;

use crate::activity::model::RecentPlay;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UploadResult {
    Unsupported,
}

#[derive(Debug, Error)]
pub enum UploadError {
    #[error("Hanami Web does not expose a play-ingestion endpoint yet")]
    Unsupported,
}

#[async_trait]
pub trait PlayUploader: Send + Sync {
    async fn upload(&self, play: &RecentPlay) -> Result<UploadResult, UploadError>;
}

pub struct UnsupportedPlayUploader;

#[async_trait]
impl PlayUploader for UnsupportedPlayUploader {
    async fn upload(&self, _play: &RecentPlay) -> Result<UploadResult, UploadError> {
        Err(UploadError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unavailable_uploader_never_claims_success() {
        let uploader = UnsupportedPlayUploader;
        let result = uploader
            .upload(&RecentPlay {
                id: "test".into(),
                score_id: None,
                beatmap: Default::default(),
                started_at: chrono::Utc::now(),
                ended_at: chrono::Utc::now(),
                player_name: None,
                score: 0,
                accuracy: 0.0,
                combo: 0,
                misses: 0,
                mods: vec![],
                pp: None,
                completion: Some(0.5),
                outcome: crate::activity::model::PlayOutcome::Quit,
                rank: None,
            })
            .await;
        assert!(matches!(result, Err(UploadError::Unsupported)));
    }
}
