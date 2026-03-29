use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::json;
use voxtract_domain::errors::VoxtractError;
use voxtract_domain::models::transcript::Transcript;
use voxtract_domain::ports::transcript_repository::TranscriptRepository;

use crate::util::slugify;

pub struct JsonTranscriptRepository {
    output_dir: PathBuf,
}

impl JsonTranscriptRepository {
    pub fn new(output_dir: &Path) -> Self {
        Self {
            output_dir: output_dir.to_path_buf(),
        }
    }
}

impl TranscriptRepository for JsonTranscriptRepository {
    async fn save(&self, transcript: &Transcript) -> Result<PathBuf, VoxtractError> {
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .map_err(|e| VoxtractError::Extraction(format!("Failed to create output dir: {e}")))?;

        let title = if transcript.source.title.is_empty() {
            &transcript.source.video_id
        } else {
            &transcript.source.title
        };
        let slug = slugify(title);
        let path = self.output_dir.join(format!("{slug}.json"));

        let data = json!({
            "source": {
                "url": transcript.source.url,
                "video_id": transcript.source.video_id,
                "title": transcript.source.title,
            },
            "date_transcribed": Utc::now().format("%Y-%m-%d").to_string(),
            "speakers": transcript.speakers.iter().map(|s| json!({
                "label": s.label,
                "display_name": s.display_name,
                "is_primary": s.is_primary,
            })).collect::<Vec<_>>(),
            "utterances": transcript.utterances.iter().map(|u| {
                let speaker_name = transcript
                    .speaker_by_label(&u.speaker_label)
                    .map(|s| s.name().to_string())
                    .unwrap_or_else(|| u.speaker_label.clone());
                json!({
                    "speaker_label": u.speaker_label,
                    "speaker_name": speaker_name,
                    "text": u.text,
                    "start_time": u.start_time,
                    "end_time": u.end_time,
                })
            }).collect::<Vec<_>>(),
        });

        let content = serde_json::to_string_pretty(&data)
            .map_err(|e| VoxtractError::Extraction(format!("JSON serialization failed: {e}")))?;

        tokio::fs::write(&path, content)
            .await
            .map_err(|e| VoxtractError::Extraction(format!("Failed to write file: {e}")))?;

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxtract_domain::models::speaker::Speaker;
    use voxtract_domain::models::utterance::Utterance;
    use voxtract_domain::models::video_source::VideoSource;

    #[tokio::test]
    async fn save_creates_json_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = JsonTranscriptRepository::new(dir.path());
        let transcript = Transcript {
            source: VideoSource::with_title(
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                "Test Video",
            )
            .unwrap(),
            speakers: vec![Speaker::new("Speaker A", "Alice", true)],
            utterances: vec![Utterance::new("Speaker A", "Hello", 0.0, 3.0)],
        };
        let path = repo.save(&transcript).await.unwrap();
        assert!(path.exists());
        assert_eq!(path.extension().unwrap(), "json");
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["source"]["title"], "Test Video");
        assert_eq!(parsed["utterances"][0]["speaker_name"], "Alice");
    }
}
