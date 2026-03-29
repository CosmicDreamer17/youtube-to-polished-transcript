use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;
use voxtract_domain::errors::VoxtractError;
use voxtract_domain::models::audio_file::AudioFile;
use voxtract_domain::models::transcript::RawTranscript;
use voxtract_domain::models::utterance::Utterance;
use voxtract_domain::models::video_source::VideoSource;
use voxtract_domain::ports::transcriber::Transcriber;

const BASE_URL: &str = "https://api.assemblyai.com/v2";

#[derive(Serialize)]
struct TranscriptionRequest {
    audio_url: String,
    speaker_labels: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    speakers_expected: Option<i32>,
}

#[derive(Deserialize)]
struct UploadResponse {
    upload_url: String,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    id: String,
    status: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    utterances: Option<Vec<AaiUtterance>>,
}

#[derive(Deserialize)]
struct AaiUtterance {
    speaker: String,
    text: String,
    start: u64,
    end: u64,
}

pub struct AssemblyAITranscriber {
    api_key: String,
    speakers_expected: Option<i32>,
    client: Client,
}

impl AssemblyAITranscriber {
    pub fn new(api_key: &str, speakers_expected: Option<i32>) -> Self {
        Self {
            api_key: api_key.to_string(),
            speakers_expected,
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

    async fn upload_file(&self, path: &std::path::Path) -> Result<String, VoxtractError> {
        let data = tokio::fs::read(path)
            .await
            .map_err(|e| VoxtractError::Transcription(format!("Failed to read audio file: {e}")))?;

        let resp: UploadResponse = self
            .client
            .post(format!("{BASE_URL}/upload"))
            .header("authorization", &self.api_key)
            .header("content-type", "application/octet-stream")
            .body(data)
            .send()
            .await
            .map_err(|e| VoxtractError::Transcription(format!("Upload failed: {e}")))?
            .json()
            .await
            .map_err(|e| {
                VoxtractError::Transcription(format!("Upload response parse failed: {e}"))
            })?;

        Ok(resp.upload_url)
    }

    async fn submit_transcription(&self, audio_url: &str) -> Result<String, VoxtractError> {
        let request = TranscriptionRequest {
            audio_url: audio_url.to_string(),
            speaker_labels: true,
            speakers_expected: self.speakers_expected,
        };

        let resp: TranscriptionResponse = self
            .client
            .post(format!("{BASE_URL}/transcript"))
            .header("authorization", &self.api_key)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| VoxtractError::Transcription(format!("Submit failed: {e}")))?
            .json()
            .await
            .map_err(|e| {
                VoxtractError::Transcription(format!("Submit response parse failed: {e}"))
            })?;

        Ok(resp.id)
    }

    async fn poll_until_complete(
        &self,
        transcript_id: &str,
    ) -> Result<TranscriptionResponse, VoxtractError> {
        let url = format!("{BASE_URL}/transcript/{transcript_id}");
        let max_attempts = 600; // 30 minutes at 3s intervals

        for _ in 0..max_attempts {
            let resp: TranscriptionResponse = self
                .client
                .get(&url)
                .header("authorization", &self.api_key)
                .send()
                .await
                .map_err(|e| VoxtractError::Transcription(format!("Poll failed: {e}")))?
                .json()
                .await
                .map_err(|e| VoxtractError::Transcription(format!("Poll parse failed: {e}")))?;

            match resp.status.as_str() {
                "completed" => return Ok(resp),
                "error" => {
                    let msg = resp.error.unwrap_or_else(|| "Unknown error".to_string());
                    return Err(VoxtractError::Transcription(format!(
                        "AssemblyAI returned error: {msg}"
                    )));
                }
                status => {
                    debug!("Transcription status: {status}, waiting...");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }

        Err(VoxtractError::Transcription(
            "Transcription timed out after 30 minutes".to_string(),
        ))
    }
}

impl Transcriber for AssemblyAITranscriber {
    async fn transcribe(
        &self,
        audio: &AudioFile,
        source: &VideoSource,
    ) -> Result<RawTranscript, VoxtractError> {
        let upload_url = self.upload_file(&audio.path).await?;
        let transcript_id = self.submit_transcription(&upload_url).await?;
        let response = self.poll_until_complete(&transcript_id).await?;

        let aai_utterances = response.utterances.ok_or_else(|| {
            VoxtractError::Transcription(
                "AssemblyAI returned no utterances — audio may be silent or unrecognizable"
                    .to_string(),
            )
        })?;

        if aai_utterances.is_empty() {
            return Err(VoxtractError::Transcription(
                "AssemblyAI returned no utterances — audio may be silent or unrecognizable"
                    .to_string(),
            ));
        }

        let mut speaker_counter: HashMap<String, String> = HashMap::new();
        let mut utterances = Vec::new();

        for u in aai_utterances {
            let label = if let Some(existing) = speaker_counter.get(&u.speaker) {
                existing.clone()
            } else {
                let idx = speaker_counter.len();
                let new_label = Self::make_speaker_label(idx);
                speaker_counter.insert(u.speaker.clone(), new_label.clone());
                new_label
            };

            utterances.push(Utterance::new(
                &label,
                &u.text,
                u.start as f64 / 1000.0,
                u.end as f64 / 1000.0,
            ));
        }

        Ok(RawTranscript {
            source: source.clone(),
            utterances,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaker_label_generation() {
        assert_eq!(AssemblyAITranscriber::make_speaker_label(0), "Speaker A");
        assert_eq!(AssemblyAITranscriber::make_speaker_label(1), "Speaker B");
        assert_eq!(AssemblyAITranscriber::make_speaker_label(25), "Speaker Z");
        assert_eq!(AssemblyAITranscriber::make_speaker_label(26), "Speaker AA");
        assert_eq!(AssemblyAITranscriber::make_speaker_label(27), "Speaker AB");
    }
}
