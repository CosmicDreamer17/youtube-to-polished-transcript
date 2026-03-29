use thiserror::Error;

/// Top-level error type for all voxtract domain errors.
#[derive(Debug, Error)]
pub enum VoxtractError {
    #[error("Extraction failed: {0}")]
    Extraction(String),

    #[error("Transcription failed: {0}")]
    Transcription(String),

    #[error("Polishing failed: {0}")]
    Polishing(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}
