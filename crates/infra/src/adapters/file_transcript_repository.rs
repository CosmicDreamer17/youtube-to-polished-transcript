use std::path::{Path, PathBuf};

use chrono::Utc;
use yt2pt_domain::errors::Yt2ptError;
use yt2pt_domain::models::transcript::Transcript;
use yt2pt_domain::ports::transcript_repository::TranscriptRepository;

use crate::util::slugify;

pub struct FileTranscriptRepository {
    output_dir: PathBuf,
}

impl FileTranscriptRepository {
    pub fn new(output_dir: &Path) -> Self {
        Self {
            output_dir: output_dir.to_path_buf(),
        }
    }

    fn render(&self, transcript: &Transcript) -> String {
        let title = if transcript.source.title.is_empty() {
            &transcript.source.video_id
        } else {
            &transcript.source.title
        };

        let speaker_names: Vec<&str> = transcript.speakers.iter().map(|s| s.name()).collect();
        let duration_min = (transcript.duration_seconds() / 60.0) as u64;

        let mut lines = vec![
            format!("# Transcript: {title}"),
            String::new(),
            format!("**Source:** {}", transcript.source.url),
            format!("**Date transcribed:** {}", Utc::now().format("%Y-%m-%d")),
            format!("**Speakers:** {}", speaker_names.join(", ")),
        ];

        if let Some(primary) = transcript.primary_speaker() {
            lines.push(format!("**Primary speaker:** {}", primary.name()));
        }

        if duration_min > 0 {
            lines.push(format!("**Duration:** {duration_min} minutes"));
        }

        lines.push(String::new());
        lines.push("---".to_string());
        lines.push(String::new());

        for utterance in &transcript.utterances {
            let name = transcript
                .speaker_by_label(&utterance.speaker_label)
                .map(|s| s.name())
                .unwrap_or(&utterance.speaker_label);
            lines.push(format!("**{name}:** {}", utterance.text));
            lines.push(String::new());
        }

        lines.join("\n")
    }
}

impl TranscriptRepository for FileTranscriptRepository {
    async fn save(&self, transcript: &Transcript) -> Result<PathBuf, Yt2ptError> {
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to create output dir: {e}")))?;

        let title = if transcript.source.title.is_empty() {
            &transcript.source.video_id
        } else {
            &transcript.source.title
        };
        let slug = slugify(title);
        let filename = format!("{slug}.md");
        let path = self.output_dir.join(filename);

        let content = self.render(transcript);
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to write file: {e}")))?;

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yt2pt_domain::models::speaker::Speaker;
    use yt2pt_domain::models::utterance::Utterance;
    use yt2pt_domain::models::video_source::VideoSource;

    fn make_transcript() -> Transcript {
        Transcript {
            source: VideoSource::with_title(
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                "Test Video",
            )
            .unwrap(),
            speakers: vec![
                Speaker::new("Speaker A", "Alice", true),
                Speaker::new("Speaker B", "Bob", false),
            ],
            utterances: vec![
                Utterance::new("Speaker A", "Hello world", 0.0, 3.0),
                Utterance::new("Speaker B", "Hi there", 3.0, 5.0),
            ],
        }
    }

    #[test]
    fn render_contains_title() {
        let repo = FileTranscriptRepository::new(Path::new("/tmp"));
        let content = repo.render(&make_transcript());
        assert!(content.contains("# Transcript: Test Video"));
        assert!(content.contains("**Alice:** Hello world"));
        assert!(content.contains("**Bob:** Hi there"));
        assert!(content.contains("**Primary speaker:** Alice"));
    }

    #[tokio::test]
    async fn save_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = FileTranscriptRepository::new(dir.path());
        let path = repo.save(&make_transcript()).await.unwrap();
        assert!(path.exists());
        assert!(path.extension().unwrap() == "md");
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("Test Video"));
    }
}
