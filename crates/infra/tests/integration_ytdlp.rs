/// Integration tests for YtdlpAudioExtractor.
/// Run with: cargo test -p yt2pt-infra --test integration_ytdlp -- --ignored
use yt2pt_domain::models::video_source::VideoSource;
use yt2pt_domain::ports::audio_extractor::AudioExtractor;
use yt2pt_infra::adapters::ytdlp_audio_extractor::YtdlpAudioExtractor;

#[tokio::test]
#[ignore] // Requires yt-dlp and network access
async fn extract_short_video() {
    let dir = tempfile::tempdir().unwrap();
    let extractor = YtdlpAudioExtractor::new(dir.path());

    // 18-second "Me at the zoo" — first YouTube video ever
    let source = VideoSource::new("https://www.youtube.com/watch?v=jNQXAC9IVRw").unwrap();
    let audio = extractor.extract(&source).await.unwrap();

    assert!(audio.path.exists());
    assert_eq!(audio.format, "wav");
    assert!(audio.duration_seconds > 10.0);
    assert!(!audio.source_title.is_empty());
}
