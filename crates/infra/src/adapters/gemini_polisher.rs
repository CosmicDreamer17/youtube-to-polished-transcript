use regex::Regex;
use std::sync::LazyLock;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::warn;
use voxtract_domain::errors::VoxtractError;
use voxtract_domain::models::transcript::{PolishResult, Transcript};
use voxtract_domain::models::utterance::Utterance;
use voxtract_domain::ports::polisher::Polisher;

static RESPONSE_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[.*?\]: (.*)$").unwrap());

// Same system prompt as the other polishers — polishing rules are model-agnostic.
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

// --- Gemini REST API types ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    contents: Vec<Content>,
    system_instruction: Content,
    generation_config: GenerationConfig,
}

#[derive(Serialize, Deserialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize, Deserialize)]
struct Part {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f64,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
    #[serde(default, rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Content,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    #[serde(default)]
    prompt_token_count: u64,
    #[serde(default)]
    candidates_token_count: u64,
}

struct BatchResult {
    texts: Vec<String>,
    input_tokens: u64,
    output_tokens: u64,
}

pub struct GeminiPolisher {
    api_key: String,
    model: String,
    temperature: f64,
    batch_size_tokens: usize,
    client: Client,
}

impl GeminiPolisher {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: "gemini-2.5-flash".to_string(),
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
    ) -> Result<BatchResult, VoxtractError> {
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

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part { text: user_content }],
            }],
            system_instruction: Content {
                parts: vec![Part {
                    text: POLISH_SYSTEM_PROMPT.to_string(),
                }],
            },
            generation_config: GenerationConfig {
                temperature: self.temperature,
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| VoxtractError::Polishing(format!("Gemini API error: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(VoxtractError::Polishing(format!(
                "Gemini API returned {status}: {body}"
            )));
        }

        let gemini_response: GeminiResponse = response.json().await.map_err(|e| {
            VoxtractError::Polishing(format!("Failed to parse Gemini response: {e}"))
        })?;

        let (input_tokens, output_tokens) = gemini_response
            .usage_metadata
            .map(|u| (u.prompt_token_count, u.candidates_token_count))
            .unwrap_or((0, 0));

        let response_text = gemini_response
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.as_str())
            .unwrap_or("");

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
                "Gemini polisher response had {} entries but expected {} — using originals",
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

impl Polisher for GeminiPolisher {
    async fn polish(&self, transcript: &Transcript) -> Result<PolishResult, VoxtractError> {
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
