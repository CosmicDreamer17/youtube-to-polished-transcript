use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use voxtract_domain::errors::VoxtractError;
use voxtract_domain::models::audio_file::AudioFile;
use voxtract_domain::models::transcript::RawTranscript;
use voxtract_domain::models::utterance::Utterance;
use voxtract_domain::models::video_source::VideoSource;
use voxtract_domain::ports::transcriber::Transcriber;

const API_URL: &str = "https://api.deepgram.com/v1/listen";

#[derive(Deserialize)]
struct DeepgramResponse {
    results: DeepgramResults,
}

#[derive(Deserialize)]
struct DeepgramResults {
    utterances: Option<Vec<DeepgramUtterance>>,
}

#[derive(Deserialize)]
struct DeepgramUtterance {
    speaker: u32,
    transcript: String,
    start: f64,
    end: f64,
}

pub struct DeepgramTranscriber {
    api_key: String,
    client: Client,
}

impl DeepgramTranscriber {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: Client::new(),
        }
    }

    fn make_speaker_label(idx: usize) -> String {
        if idx < 26 {
            format!("Speaker {}", (b'A' + idx as u8) as char)
        } else {
            let prefix = (b'A' + ((idx - 26) / 26) as u8) as char;
            let suffix = (b'A' + ((idx - 26) % 26) as u8) as char;
            format!("Speaker {prefix}{suffix}")
        }
    }
}

impl Transcriber for DeepgramTranscriber {
    async fn transcribe(
        &self,
        audio: &AudioFile,
        source: &VideoSource,
    ) -> Result<RawTranscript, VoxtractError> {
        let data = tokio::fs::read(&audio.path)
            .await
            .map_err(|e| VoxtractError::Transcription(format!("Failed to read audio file: {e}")))?;

        // Deepgram API: send audio with query params for diarization
        let response = self
            .client
            .post(API_URL)
            .header("Authorization", format!("Token {}", self.api_key))
            .header("Content-Type", "audio/wav")
            .query(&[
                ("model", "nova-3"),
                ("diarize", "true"),
                ("utterances", "true"),
                ("punctuate", "true"),
                ("smart_format", "true"),
            ])
            .body(data)
            .send()
            .await
            .map_err(|e| VoxtractError::Transcription(format!("Deepgram API error: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(VoxtractError::Transcription(format!(
                "Deepgram returned {status}: {body}"
            )));
        }

        let dg_response: DeepgramResponse = response.json().await.map_err(|e| {
            VoxtractError::Transcription(format!("Failed to parse Deepgram response: {e}"))
        })?;

        let dg_utterances = dg_response.results.utterances.ok_or_else(|| {
            VoxtractError::Transcription(
                "Deepgram returned no utterances — audio may be silent or unrecognizable"
                    .to_string(),
            )
        })?;

        if dg_utterances.is_empty() {
            return Err(VoxtractError::Transcription(
                "Deepgram returned no utterances — audio may be silent or unrecognizable"
                    .to_string(),
            ));
        }

        // Map Deepgram's numeric speaker IDs to "Speaker A", "Speaker B", etc.
        let mut speaker_counter: HashMap<u32, String> = HashMap::new();
        let mut utterances = Vec::new();

        for u in dg_utterances {
            let label = if let Some(existing) = speaker_counter.get(&u.speaker) {
                existing.clone()
            } else {
                let idx = speaker_counter.len();
                let new_label = Self::make_speaker_label(idx);
                speaker_counter.insert(u.speaker, new_label.clone());
                new_label
            };

            utterances.push(Utterance::new(&label, &u.transcript, u.start, u.end));
        }

        Ok(RawTranscript {
            source: source.clone(),
            utterances,
            audio_duration_seconds: audio.duration_seconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaker_label_generation() {
        assert_eq!(DeepgramTranscriber::make_speaker_label(0), "Speaker A");
        assert_eq!(DeepgramTranscriber::make_speaker_label(25), "Speaker Z");
        assert_eq!(DeepgramTranscriber::make_speaker_label(26), "Speaker AA");
    }
}
