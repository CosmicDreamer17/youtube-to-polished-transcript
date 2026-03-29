use regex::Regex;
use std::sync::LazyLock;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::warn;
use yt2pt_domain::errors::Yt2ptError;
use yt2pt_domain::models::transcript::{PolishResult, Transcript};
use yt2pt_domain::models::utterance::Utterance;
use yt2pt_domain::ports::polisher::Polisher;

static RESPONSE_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[.*?\]: (.*)$").unwrap());

const POLISH_SYSTEM_PROMPT: &str = "\
You are a transcript editor. Your job is to clean up speech-to-text output so it reads \
naturally while preserving exactly what the speaker said.

Rules:
1. Remove filler words: um, uh, like (when used as filler), you know, sort of, kind of, \
I mean, right (when used as filler), basically, literally (when used as filler)
2. Remove false starts: \"I was going to— I decided to go\" → \"I decided to go\"
3. Remove verbal repetitions: \"I think I think we should\" → \"I think we should\"
4. Fix minor grammar only where the speaker clearly misspoke
5. Preserve the speaker's vocabulary, sentence structure, and rhetorical style
6. Do NOT rephrase or paraphrase — keep their actual words
7. Do NOT add words the speaker didn't say
8. Do NOT change technical terms, proper nouns, or domain-specific language
9. Do NOT merge separate utterances or change speaker attributions
10. Preserve emphasis and rhetorical devices (repetition for emphasis is NOT a verbal \
repetition — keep it)
11. For long utterances (more than ~100 words), insert paragraph breaks (blank lines) at \
natural topic transitions. Use \\n\\n within the utterance text to indicate a paragraph break. \
Do NOT split into separate lines — keep it as one [Speaker]: entry with internal breaks.

Input: Each line is formatted as [Speaker Name]: utterance text
Output: Return the same format with cleaned text. One line per utterance, same order.";

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f64,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

struct BatchResult {
    texts: Vec<String>,
    input_tokens: u64,
    output_tokens: u64,
}

pub struct OllamaPolisher {
    base_url: String,
    model: String,
    temperature: f64,
    batch_size_tokens: usize,
    client: Client,
}

impl OllamaPolisher {
    pub fn new(model: &str) -> Self {
        let base_url =
            std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".into());
        Self {
            base_url,
            model: model.to_string(),
            temperature: 0.2,
            batch_size_tokens: 2000,
            client: Client::new(),
        }
    }

    fn create_batches<'a>(&self, utterances: &'a [Utterance]) -> Vec<Vec<&'a Utterance>> {
        let mut batches: Vec<Vec<&Utterance>> = Vec::new();
        let mut current_batch: Vec<&Utterance> = Vec::new();
        let mut current_tokens: f64 = 0.0;

        for utterance in utterances {
            let est_tokens = utterance.text.split_whitespace().count() as f64 * 1.3;
            if current_tokens + est_tokens > self.batch_size_tokens as f64
                && !current_batch.is_empty()
            {
                batches.push(current_batch);
                current_batch = Vec::new();
                current_tokens = 0.0;
            }
            current_batch.push(utterance);
            current_tokens += est_tokens;
        }

        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        batches
    }

    async fn polish_batch(
        &self,
        batch: &[&Utterance],
        transcript: &Transcript,
    ) -> Result<BatchResult, Yt2ptError> {
        let lines: Vec<String> = batch
            .iter()
            .map(|u| {
                let name = transcript
                    .speaker_by_label(&u.speaker_label)
                    .map(|s| s.name())
                    .unwrap_or(&u.speaker_label);
                format!("[{name}]: {}", u.text)
            })
            .collect();

        let user_content = lines.join("\n");

        let request = OllamaRequest {
            model: self.model.clone(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: POLISH_SYSTEM_PROMPT.to_string(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: user_content,
                },
            ],
            stream: false,
            options: OllamaOptions {
                temperature: self.temperature,
            },
        };

        let url = format!("{}/api/chat", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                Yt2ptError::Polishing(format!(
                    "Ollama API error (is Ollama running at {}?): {e}",
                    self.base_url
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Yt2ptError::Polishing(format!(
                "Ollama returned {status}: {body}"
            )));
        }

        let ollama_response: OllamaResponse = response.json().await.map_err(|e| {
            Yt2ptError::Polishing(format!("Failed to parse Ollama response: {e}"))
        })?;

        let input_tokens = ollama_response.prompt_eval_count.unwrap_or(0);
        let output_tokens = ollama_response.eval_count.unwrap_or(0);
        let response_text = &ollama_response.message.content;

        let mut polished_texts: Vec<String> = Vec::new();
        let mut current_text_parts: Vec<String> = Vec::new();

        for line in response_text.trim().split('\n') {
            let trimmed = line.trim();
            if let Some(caps) = RESPONSE_LINE_RE.captures(trimmed) {
                if !current_text_parts.is_empty() {
                    polished_texts.push(current_text_parts.join("\n\n"));
                }
                current_text_parts = vec![caps[1].to_string()];
            } else if !trimmed.is_empty() {
                current_text_parts.push(trimmed.to_string());
            }
        }

        if !current_text_parts.is_empty() {
            polished_texts.push(current_text_parts.join("\n\n"));
        }

        if polished_texts.len() != batch.len() {
            warn!(
                "Ollama polisher response had {} entries but expected {} — using originals",
                polished_texts.len(),
                batch.len()
            );
            return Ok(BatchResult {
                texts: batch.iter().map(|u| u.text.clone()).collect(),
                input_tokens,
                output_tokens,
            });
        }

        Ok(BatchResult {
            texts: polished_texts,
            input_tokens,
            output_tokens,
        })
    }
}

impl Polisher for OllamaPolisher {
    async fn polish(&self, transcript: &Transcript) -> Result<PolishResult, Yt2ptError> {
        let batches = self.create_batches(&transcript.utterances);
        let mut polished_utterances: Vec<Utterance> = Vec::new();
        let mut total_input_tokens: u64 = 0;
        let mut total_output_tokens: u64 = 0;

        for batch in &batches {
            let result = self.polish_batch(batch, transcript).await?;
            total_input_tokens += result.input_tokens;
            total_output_tokens += result.output_tokens;
            for (utterance, new_text) in batch.iter().zip(result.texts.into_iter()) {
                polished_utterances.push(Utterance::new(
                    &utterance.speaker_label,
                    &new_text,
                    utterance.start_time,
                    utterance.end_time,
                ));
            }
        }

        Ok(PolishResult {
            transcript: Transcript {
                source: transcript.source.clone(),
                speakers: transcript.speakers.clone(),
                utterances: polished_utterances,
            },
            input_tokens: total_input_tokens,
            output_tokens: total_output_tokens,
        })
    }
}
