use crate::models::speaker::Speaker;
use crate::models::utterance::Utterance;
use crate::models::video_source::VideoSource;

#[derive(Debug, Clone)]
pub struct RawTranscript {
    pub source: VideoSource,
    pub utterances: Vec<Utterance>,
}

impl RawTranscript {
    pub fn speaker_labels(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut labels = Vec::new();
        for u in &self.utterances {
            if seen.insert(u.speaker_label.clone()) {
                labels.push(u.speaker_label.clone());
            }
        }
        labels
    }

    pub fn utterances_by_speaker(&self, label: &str) -> Vec<&Utterance> {
        self.utterances
            .iter()
            .filter(|u| u.speaker_label == label)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Transcript {
    pub source: VideoSource,
    pub speakers: Vec<Speaker>,
    pub utterances: Vec<Utterance>,
}

impl Transcript {
    pub fn primary_speaker(&self) -> Option<&Speaker> {
        self.speakers.iter().find(|s| s.is_primary)
    }

    pub fn speaker_by_label(&self, label: &str) -> Option<&Speaker> {
        self.speakers.iter().find(|s| s.label == label)
    }

    pub fn duration_seconds(&self) -> f64 {
        self.utterances
            .iter()
            .map(|u| u.end_time)
            .fold(0.0_f64, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::speaker::Speaker;

    fn make_test_transcript() -> Transcript {
        Transcript {
            source: VideoSource::with_title("https://www.youtube.com/watch?v=dQw4w9WgXcQ", "Test")
                .unwrap(),
            speakers: vec![
                Speaker::new("Speaker A", "Alice", true),
                Speaker::new("Speaker B", "Bob", false),
            ],
            utterances: vec![
                Utterance::new("Speaker A", "Hello", 0.0, 3.5),
                Utterance::new("Speaker B", "Hi there", 3.5, 6.0),
                Utterance::new("Speaker A", "How are you?", 6.0, 10.0),
            ],
        }
    }

    #[test]
    fn primary_speaker_found() {
        let t = make_test_transcript();
        let p = t.primary_speaker().unwrap();
        assert_eq!(p.name(), "Alice");
    }

    #[test]
    fn speaker_by_label_found() {
        let t = make_test_transcript();
        let s = t.speaker_by_label("Speaker B").unwrap();
        assert_eq!(s.name(), "Bob");
    }

    #[test]
    fn duration_seconds() {
        let t = make_test_transcript();
        assert!((t.duration_seconds() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn raw_transcript_speaker_labels() {
        let raw = RawTranscript {
            source: VideoSource::new("dQw4w9WgXcQ").unwrap(),
            utterances: vec![
                Utterance::new("Speaker A", "Hello", 0.0, 1.0),
                Utterance::new("Speaker B", "Hi", 1.0, 2.0),
                Utterance::new("Speaker A", "Bye", 2.0, 3.0),
            ],
        };
        assert_eq!(raw.speaker_labels(), vec!["Speaker A", "Speaker B"]);
    }

    #[test]
    fn raw_transcript_utterances_by_speaker() {
        let raw = RawTranscript {
            source: VideoSource::new("dQw4w9WgXcQ").unwrap(),
            utterances: vec![
                Utterance::new("Speaker A", "Hello", 0.0, 1.0),
                Utterance::new("Speaker B", "Hi", 1.0, 2.0),
                Utterance::new("Speaker A", "Bye", 2.0, 3.0),
            ],
        };
        let a_utts = raw.utterances_by_speaker("Speaker A");
        assert_eq!(a_utts.len(), 2);
        assert_eq!(a_utts[0].text, "Hello");
        assert_eq!(a_utts[1].text, "Bye");
    }
}
