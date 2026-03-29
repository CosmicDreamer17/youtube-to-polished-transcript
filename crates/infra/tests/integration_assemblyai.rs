/// Integration tests for AssemblyAITranscriber.
/// Run with: cargo test -p yt2pt-infra --test integration_assemblyai -- --ignored
/// Requires ASSEMBLYAI_API_KEY env var.
use yt2pt_domain::models::video_source::VideoSource;
use yt2pt_domain::ports::audio_extractor::AudioExtractor;
use yt2pt_domain::ports::transcriber::Transcriber;
use yt2pt_infra::adapters::assemblyai_transcriber::AssemblyAITranscriber;
use yt2pt_infra::adapters::ytdlp_audio_extractor::YtdlpAudioExtractor;

#[tokio::test]
#[ignore] // Requires AssemblyAI API key, yt-dlp, and network access (~$0.01)
async fn transcribe_short_video() {
    let api_key = std::env::var("ASSEMBLYAI_API_KEY").expect("ASSEMBLYAI_API_KEY must be set");

    let dir = tempfile::tempdir().unwrap();
    let extractor = YtdlpAudioExtractor::new(dir.path());
    let source = VideoSource::new("https://www.youtube.com/watch?v=jNQXAC9IVRw").unwrap();
    let audio = extractor.extract(&source).await.unwrap();

    let transcriber = AssemblyAITranscriber::new(&api_key, None);
    let raw = transcriber.transcribe(&audio, &source).await.unwrap();

    assert!(!raw.utterances.is_empty());
    assert!(!raw.speaker_labels().is_empty());
    // The video has one speaker
    assert!(raw.speaker_labels().len() <= 2);
}
