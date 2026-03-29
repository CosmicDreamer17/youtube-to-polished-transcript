use std::path::{Path, PathBuf};

use yt2pt_domain::errors::Yt2ptError;
use yt2pt_domain::models::transcript::Transcript;
use yt2pt_domain::ports::transcript_repository::TranscriptRepository;

use crate::util::slugify;

pub struct SrtTranscriptRepository {
    output_dir: PathBuf,
}

impl SrtTranscriptRepository {
    pub fn new(output_dir: &Path) -> Self {
        Self {
            output_dir: output_dir.to_path_buf(),
        }
    }
}

fn seconds_to_timecode(seconds: f64) -> String {
    let hours = (seconds / 3600.0) as u32;
    let minutes = ((seconds % 3600.0) / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    let millis = ((seconds % 1.0) * 1000.0) as u32;
    format!("{hours:02}:{minutes:02}:{secs:02},{millis:03}")
}

impl TranscriptRepository for SrtTranscriptRepository {
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
        let path = self.output_dir.join(format!("{slug}.srt"));

        let mut lines: Vec<String> = Vec::new();
        for (i, u) in transcript.utterances.iter().enumerate() {
            let name = transcript
                .speaker_by_label(&u.speaker_label)
                .map(|s| s.name())
                .unwrap_or(&u.speaker_label);
            let start = seconds_to_timecode(u.start_time);
            let end = seconds_to_timecode(u.end_time);
            lines.push(format!("{}", i + 1));
            lines.push(format!("{start} --> {end}"));
            lines.push(format!("{name}: {}", u.text));
            lines.push(String::new());
        }

        let content = lines.join("\n");
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

    #[test]
    fn test_seconds_to_timecode() {
        assert_eq!(seconds_to_timecode(0.0), "00:00:00,000");
        assert_eq!(seconds_to_timecode(61.5), "00:01:01,500");
        assert_eq!(seconds_to_timecode(3661.123), "01:01:01,123");
    }

    #[tokio::test]
    async fn save_creates_srt_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SrtTranscriptRepository::new(dir.path());
        let transcript = Transcript {
            source: VideoSource::with_title(
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                "Test Video",
            )
            .unwrap(),
            speakers: vec![Speaker::new("Speaker A", "Alice", true)],
            utterances: vec![Utterance::new("Speaker A", "Hello", 0.0, 3.5)],
        };
        let path = repo.save(&transcript).await.unwrap();
        assert!(path.exists());
        assert_eq!(path.extension().unwrap(), "srt");
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("00:00:00,000 --> 00:00:03,500"));
        assert!(content.contains("Alice: Hello"));
    }
}
