use std::collections::HashMap;
use std::path::PathBuf;

use voxtract_domain::errors::VoxtractError;
use voxtract_domain::models::transcript::{RawTranscript, Transcript};
use voxtract_domain::models::video_source::VideoSource;
use voxtract_domain::ports::audio_extractor::AudioExtractor;
use voxtract_domain::ports::polisher::Polisher;
use voxtract_domain::ports::transcriber::Transcriber;
use voxtract_domain::ports::transcript_repository::TranscriptRepository;

use crate::services::speaker_mapping;

pub struct TranscriptPipelineService<E, T, P, R>
where
    E: AudioExtractor,
    T: Transcriber,
    P: Polisher,
    R: TranscriptRepository,
{
    audio_extractor: E,
    transcriber: T,
    polisher: P,
    repository: R,
}

impl<E, T, P, R> TranscriptPipelineService<E, T, P, R>
where
    E: AudioExtractor,
    T: Transcriber,
    P: Polisher,
    R: TranscriptRepository,
{
    pub fn new(audio_extractor: E, transcriber: T, polisher: P, repository: R) -> Self {
        Self {
            audio_extractor,
            transcriber,
            polisher,
            repository,
        }
    }

    /// Stages 1-2: Download audio and transcribe with diarization.
    pub async fn extract_and_transcribe(&self, url: &str) -> Result<RawTranscript, VoxtractError> {
        let source = VideoSource::new(url)?;
        let audio = self.audio_extractor.extract(&source).await?;

        // Update source with title from yt-dlp if we got one
        let source = if !audio.source_title.is_empty() && source.title.is_empty() {
            VideoSource::with_all(&source.url, &audio.source_title, &source.video_id)
        } else {
            source
        };

        self.transcriber.transcribe(&audio, &source).await
    }

    /// Stages 4-5: Polish transcript and save to file.
    pub async fn polish_and_save(&self, transcript: &Transcript) -> Result<PathBuf, VoxtractError> {
        let polished = self.polisher.polish(transcript).await?;
        self.repository.save(&polished).await
    }

    /// Run the full pipeline: extract, transcribe, map speakers, polish, save.
    pub async fn run(
        &self,
        url: &str,
        speaker_map: &HashMap<String, String>,
        primary_speaker_label: Option<&str>,
    ) -> Result<PathBuf, VoxtractError> {
        let raw = self.extract_and_transcribe(url).await?;
        let transcript = speaker_mapping::apply_mapping(&raw, speaker_map, primary_speaker_label);
        self.polish_and_save(&transcript).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use voxtract_domain::models::audio_file::AudioFile;
    use voxtract_domain::models::transcript::Transcript;
    use voxtract_domain::models::utterance::Utterance;

    struct MockExtractor;
    impl AudioExtractor for MockExtractor {
        async fn extract(&self, _source: &VideoSource) -> Result<AudioFile, VoxtractError> {
            Ok(AudioFile {
                path: PathBuf::from("/tmp/test.wav"),
                duration_seconds: 60.0,
                format: "wav".to_string(),
                source_title: "Mock Video Title".to_string(),
            })
        }
    }

    struct MockTranscriber;
    impl Transcriber for MockTranscriber {
        async fn transcribe(
            &self,
            _audio: &AudioFile,
            source: &VideoSource,
        ) -> Result<RawTranscript, VoxtractError> {
            Ok(RawTranscript {
                source: source.clone(),
                utterances: vec![
                    Utterance::new("Speaker A", "Hello world", 0.0, 3.0),
                    Utterance::new("Speaker B", "Hi there", 3.0, 5.0),
                ],
            })
        }
    }

    struct MockPolisher;
    impl Polisher for MockPolisher {
        async fn polish(&self, transcript: &Transcript) -> Result<Transcript, VoxtractError> {
            Ok(transcript.clone())
        }
    }

    struct MockRepository;
    impl TranscriptRepository for MockRepository {
        async fn save(&self, _transcript: &Transcript) -> Result<PathBuf, VoxtractError> {
            Ok(PathBuf::from("/tmp/output/test.md"))
        }
    }

    #[tokio::test]
    async fn test_extract_and_transcribe() {
        let pipeline = TranscriptPipelineService::new(
            MockExtractor,
            MockTranscriber,
            MockPolisher,
            MockRepository,
        );
        let raw = pipeline
            .extract_and_transcribe("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
            .await
            .unwrap();
        assert_eq!(raw.utterances.len(), 2);
        assert_eq!(raw.source.title, "Mock Video Title");
    }

    #[tokio::test]
    async fn test_run_full_pipeline() {
        let pipeline = TranscriptPipelineService::new(
            MockExtractor,
            MockTranscriber,
            MockPolisher,
            MockRepository,
        );
        let mut names = HashMap::new();
        names.insert("Speaker A".to_string(), "Alice".to_string());
        let path = pipeline
            .run(
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                &names,
                Some("Speaker A"),
            )
            .await
            .unwrap();
        assert_eq!(path, PathBuf::from("/tmp/output/test.md"));
    }
}
