use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub video_title: String,
    pub youtube_url: String,
    pub video_id: String,
    pub speakers: Vec<ManifestSpeaker>,
    pub primary_speaker: Option<String>,
    pub duration_seconds: f64,
    pub date_transcribed: String,
    pub assemblyai_cost_usd: f64,
    pub claude_cost_usd: f64,
    pub claude_input_tokens: u64,
    pub claude_output_tokens: u64,
    pub output_file: String,
    pub output_format: String,
    pub batch_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSpeaker {
    pub label: String,
    pub name: String,
}

impl ManifestEntry {
    /// AssemblyAI cost: $0.29 per hour of audio.
    pub fn compute_assemblyai_cost(duration_seconds: f64) -> f64 {
        duration_seconds / 3600.0 * 0.29
    }

    /// Claude cost: $3/M input tokens + $15/M output tokens (Sonnet pricing).
    pub fn compute_claude_cost(input_tokens: u64, output_tokens: u64) -> f64 {
        input_tokens as f64 / 1_000_000.0 * 3.0 + output_tokens as f64 / 1_000_000.0 * 15.0
    }
}
