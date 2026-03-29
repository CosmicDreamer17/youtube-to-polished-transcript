use std::path::PathBuf;

use crate::errors::VoxtractError;
use crate::models::transcript::Transcript;

pub trait TranscriptRepository: Send + Sync {
    fn save(
        &self,
        transcript: &Transcript,
    ) -> impl std::future::Future<Output = Result<PathBuf, VoxtractError>> + Send;
}
