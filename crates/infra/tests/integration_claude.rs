/// Integration tests for ClaudePolisher.
/// Run with: cargo test -p yt2pt-infra --test integration_claude -- --ignored
/// Requires ANTHROPIC_API_KEY env var.
use yt2pt_domain::models::speaker::Speaker;
use yt2pt_domain::models::transcript::Transcript;
use yt2pt_domain::models::utterance::Utterance;
use yt2pt_domain::models::video_source::VideoSource;
use yt2pt_domain::ports::polisher::Polisher;
use yt2pt_infra::adapters::claude_polisher::ClaudePolisher;

#[tokio::test]
#[ignore] // Requires Anthropic API key and network access (~$0.001)
async fn polish_removes_filler_words() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");

    let polisher = ClaudePolisher::new(&api_key);
    let transcript = Transcript {
        source: VideoSource::with_title("https://www.youtube.com/watch?v=dQw4w9WgXcQ", "Test")
            .unwrap(),
        speakers: vec![Speaker::new("Speaker A", "Alice", true)],
        utterances: vec![Utterance::new(
            "Speaker A",
            "So um I think like you know this is basically uh really important",
            0.0,
            5.0,
        )],
    };

    let result = polisher.polish(&transcript).await.unwrap();
    assert!(result.input_tokens > 0);
    assert!(result.output_tokens > 0);
    assert_eq!(result.transcript.utterances.len(), 1);
    let text = &result.transcript.utterances[0].text;
    // Filler words should be removed
    assert!(!text.contains(" um "));
    assert!(!text.contains(" uh "));
    // Core meaning preserved
    assert!(text.contains("important"));
}
