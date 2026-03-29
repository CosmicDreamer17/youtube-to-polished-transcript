use std::path::PathBuf;

pub struct Settings {
    pub assemblyai_api_key: String,
    pub anthropic_api_key: String,
    pub output_dir: PathBuf,
    pub output_format: String,
}

impl Settings {
    pub fn from_env() -> Self {
        Self {
            assemblyai_api_key: std::env::var("ASSEMBLYAI_API_KEY").unwrap_or_default(),
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            output_dir: PathBuf::from(
                std::env::var("VOXTRACT_OUTPUT_DIR").unwrap_or_else(|_| "output".to_string()),
            ),
            output_format: std::env::var("VOXTRACT_OUTPUT_FORMAT")
                .unwrap_or_else(|_| "markdown".to_string()),
        }
    }

    pub fn validate(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if self.assemblyai_api_key.is_empty() {
            missing.push("ASSEMBLYAI_API_KEY".to_string());
        }
        if self.anthropic_api_key.is_empty() {
            missing.push("ANTHROPIC_API_KEY".to_string());
        }
        missing
    }
}
