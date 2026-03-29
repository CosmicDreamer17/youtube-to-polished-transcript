use std::collections::HashMap;

use yt2pt_domain::models::speaker::Speaker;
use yt2pt_domain::models::transcript::{RawTranscript, Transcript};

/// Return sample utterances for each speaker to help identification.
pub fn get_speaker_samples(
    raw: &RawTranscript,
    max_samples: usize,
) -> HashMap<String, Vec<String>> {
    let mut samples = HashMap::new();
    for label in raw.speaker_labels() {
        let utterances = raw.utterances_by_speaker(&label);
        let texts: Vec<String> = utterances
            .into_iter()
            .take(max_samples)
            .map(|u| u.text.clone())
            .collect();
        samples.insert(label, texts);
    }
    samples
}

/// Return total speaking time per speaker label, sorted descending.
pub fn get_speaker_stats(raw: &RawTranscript) -> Vec<(String, f64)> {
    let mut times: Vec<(String, f64)> = raw
        .speaker_labels()
        .into_iter()
        .map(|label| {
            let total: f64 = raw
                .utterances_by_speaker(&label)
                .iter()
                .map(|u| u.duration())
                .sum();
            (label, (total * 10.0).round() / 10.0)
        })
        .collect();
    times.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    times
}

/// Convert RawTranscript to Transcript with named speakers.
pub fn apply_mapping(
    raw: &RawTranscript,
    name_map: &HashMap<String, String>,
    primary_label: Option<&str>,
) -> Transcript {
    let speakers: Vec<Speaker> = raw
        .speaker_labels()
        .into_iter()
        .map(|label| {
            let display_name = name_map.get(&label).cloned().unwrap_or_default();
            let is_primary = primary_label.is_some_and(|p| p == label);
            Speaker::new(&label, &display_name, is_primary)
        })
        .collect();

    Transcript {
        source: raw.source.clone(),
        speakers,
        utterances: raw.utterances.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yt2pt_domain::models::utterance::Utterance;
    use yt2pt_domain::models::video_source::VideoSource;

    fn make_raw() -> RawTranscript {
        RawTranscript {
            source: VideoSource::new("dQw4w9WgXcQ").unwrap(),
            utterances: vec![
                Utterance::new("Speaker A", "Hello there", 0.0, 3.5),
                Utterance::new("Speaker B", "Hi how are you", 3.5, 6.0),
                Utterance::new("Speaker A", "I'm fine thanks", 6.0, 10.0),
            ],
            audio_duration_seconds: 10.0,
        }
    }

    #[test]
    fn test_get_speaker_samples() {
        let raw = make_raw();
        let samples = get_speaker_samples(&raw, 2);
        assert_eq!(samples["Speaker A"].len(), 2);
        assert_eq!(samples["Speaker B"].len(), 1);
        assert_eq!(samples["Speaker A"][0], "Hello there");
    }

    #[test]
    fn test_get_speaker_stats() {
        let raw = make_raw();
        let stats = get_speaker_stats(&raw);
        // Speaker A: 3.5 + 4.0 = 7.5, Speaker B: 2.5
        assert_eq!(stats[0].0, "Speaker A");
        assert!((stats[0].1 - 7.5).abs() < 0.1);
        assert_eq!(stats[1].0, "Speaker B");
        assert!((stats[1].1 - 2.5).abs() < 0.1);
    }

    #[test]
    fn test_apply_mapping() {
        let raw = make_raw();
        let mut name_map = HashMap::new();
        name_map.insert("Speaker A".to_string(), "Alice".to_string());
        name_map.insert("Speaker B".to_string(), "Bob".to_string());

        let transcript = apply_mapping(&raw, &name_map, Some("Speaker A"));
        assert_eq!(transcript.speakers.len(), 2);
        assert_eq!(transcript.speakers[0].name(), "Alice");
        assert!(transcript.speakers[0].is_primary);
        assert_eq!(transcript.speakers[1].name(), "Bob");
        assert!(!transcript.speakers[1].is_primary);
        assert_eq!(transcript.utterances.len(), 3);
    }

    #[test]
    fn test_apply_mapping_empty_names() {
        let raw = make_raw();
        let transcript = apply_mapping(&raw, &HashMap::new(), None);
        assert_eq!(transcript.speakers[0].name(), "Speaker A");
        assert!(!transcript.speakers[0].is_primary);
    }
}
