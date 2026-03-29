use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Utterance {
    pub speaker_label: String,
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
}

impl Utterance {
    pub fn new(speaker_label: &str, text: &str, start_time: f64, end_time: f64) -> Self {
        Self {
            speaker_label: speaker_label.to_string(),
            text: text.to_string(),
            start_time,
            end_time,
        }
    }

    pub fn duration(&self) -> f64 {
        self.end_time - self.start_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_calculated_correctly() {
        let u = Utterance::new("Speaker A", "Hello", 1.5, 4.0);
        assert!((u.duration() - 2.5).abs() < f64::EPSILON);
    }
}
