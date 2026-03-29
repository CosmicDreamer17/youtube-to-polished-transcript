use std::path::PathBuf;

pub struct Settings {
    pub assemblyai_api_key: String,
    pub anthropic_api_key: String,
    pub openai_api_key: String,
    pub google_api_key: String,
    pub deepgram_api_key: String,
    pub output_dir: PathBuf,
    pub output_format: String,
}

impl Settings {
    pub fn from_env() -> Self {
        Self {
            assemblyai_api_key: std::env::var("ASSEMBLYAI_API_KEY").unwrap_or_default(),
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            openai_api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            google_api_key: std::env::var("GOOGLE_API_KEY").unwrap_or_default(),
            deepgram_api_key: std::env::var("DEEPGRAM_API_KEY").unwrap_or_default(),
            output_dir: PathBuf::from(
                std::env::var("VOXTRACT_OUTPUT_DIR").unwrap_or_else(|_| "output".to_string()),
            ),
            output_format: std::env::var("VOXTRACT_OUTPUT_FORMAT")
                .unwrap_or_else(|_| "markdown".to_string()),
        }
    }

    /// Validate that the required API keys are present for the chosen providers.
    pub fn validate_for(&self, transcriber: &str, polisher: &str, dry_run: bool) -> Vec<String> {
        let mut missing = Vec::new();

        match transcriber {
            "assemblyai" => {
                if self.assemblyai_api_key.is_empty() {
                    missing.push("ASSEMBLYAI_API_KEY".to_string());
                }
            }
            "deepgram" => {
                if self.deepgram_api_key.is_empty() {
                    missing.push("DEEPGRAM_API_KEY".to_string());
                }
            }
            _ => {}
        }

        if !dry_run {
            match polisher {
                "claude" => {
                    if self.anthropic_api_key.is_empty() {
                        missing.push("ANTHROPIC_API_KEY".to_string());
                    }
                }
                "openai" => {
                    if self.openai_api_key.is_empty() {
                        missing.push("OPENAI_API_KEY".to_string());
                    }
                }
                "gemini" => {
                    if self.google_api_key.is_empty() {
                        missing.push("GOOGLE_API_KEY".to_string());
                    }
                }
                "ollama" => {} // No API key needed
                _ => {}
            }
        }

        missing
    }
}
