use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::process::Command;
use yt2pt_domain::errors::Yt2ptError;
use yt2pt_domain::models::audio_file::AudioFile;
use yt2pt_domain::models::video_source::VideoSource;
use yt2pt_domain::ports::audio_extractor::AudioExtractor;

#[derive(Deserialize)]
struct YtDlpInfo {
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    title: Option<String>,
}

pub struct YtdlpAudioExtractor {
    output_dir: PathBuf,
}

impl YtdlpAudioExtractor {
    pub fn new(output_dir: &Path) -> Self {
        Self {
            output_dir: output_dir.to_path_buf(),
        }
    }

    fn find_audio_file(&self, video_id: &str) -> Option<PathBuf> {
        let pattern = format!("{video_id}.");
        let entries = std::fs::read_dir(&self.output_dir).ok()?;

        let candidates: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&pattern))
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Prefer wav, then other audio formats
        for ext in &["wav", "m4a", "mp3", "opus", "ogg", "webm"] {
            for c in &candidates {
                if c.extension().and_then(|e| e.to_str()) == Some(ext) {
                    return Some(c.clone());
                }
            }
        }
        Some(candidates[0].clone())
    }
}

#[async_trait]
impl AudioExtractor for YtdlpAudioExtractor {
    async fn extract(&self, source: &VideoSource) -> Result<AudioFile, Yt2ptError> {
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to create output dir: {e}")))?;

        let output_template = self.output_dir.join(format!("{}.%(ext)s", source.video_id));

        let output = Command::new("yt-dlp")
            .args([
                "--format",
                "bestaudio/best",
                "--extract-audio",
                "--audio-format",
                "wav",
                "--postprocessor-args",
                "-ar 16000 -ac 1",
                "--output",
                output_template.to_str().unwrap(),
                "--print-json",
                "--quiet",
                "--no-progress",
                "--no-warnings",
                &source.url,
            ])
            .output()
            .await
            .map_err(|e| {
                Yt2ptError::Extraction(format!("Failed to run yt-dlp (is it installed?): {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Yt2ptError::Extraction(format!(
                "yt-dlp failed: {stderr}"
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let info: YtDlpInfo = serde_json::from_str(&stdout).unwrap_or(YtDlpInfo {
            duration: None,
            title: None,
        });

        let audio_path = self.find_audio_file(&source.video_id).ok_or_else(|| {
            Yt2ptError::Extraction(format!(
                "Expected audio file not found for {} in {}",
                source.video_id,
                self.output_dir.display()
            ))
        })?;

        let fmt = audio_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("wav")
            .to_string();

        Ok(AudioFile {
            path: audio_path,
            duration_seconds: info.duration.unwrap_or(0.0),
            format: fmt,
            source_title: info.title.unwrap_or_default(),
        })
    }
}
