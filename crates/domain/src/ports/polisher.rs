use crate::errors::VoxtractError;
use crate::models::transcript::{PolishResult, Transcript};

pub trait Polisher: Send + Sync {
    fn polish(
        &self,
        transcript: &Transcript,
    ) -> impl std::future::Future<Output = Result<PolishResult, VoxtractError>> + Send;
}
