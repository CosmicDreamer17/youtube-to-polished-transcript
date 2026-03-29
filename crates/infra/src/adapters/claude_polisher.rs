use regex::Regex;
use std::sync::LazyLock;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::warn;
use voxtract_domain::errors::VoxtractError;
use voxtract_domain::models::transcript::Transcript;
use voxtract_domain::models::utterance::Utterance;
use voxtract_domain::ports::polisher::Polisher;

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

const API_URL: &str = "https://api.anthropic.com/v1/messages";

#[derive(Serialize)]
struct MessageRequest {
    model: String,
    max_tokens: u32,
    temperature: f64,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct MessageResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

pub struct ClaudePolisher {
    api_key: String,
    model: String,
    temperature: f64,
    batch_size_tokens: usize,
    client: Client,
}

impl ClaudePolisher {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
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
    ) -> Result<Vec<String>, VoxtractError> {
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

        let request = MessageRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            temperature: self.temperature,
            system: POLISH_SYSTEM_PROMPT.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_content,
            }],
        };

        let response = self
            .client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| VoxtractError::Polishing(format!("Claude API error: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(VoxtractError::Polishing(format!(
                "Claude API returned {status}: {body}"
            )));
        }

        let msg_response: MessageResponse = response.json().await.map_err(|e| {
            VoxtractError::Polishing(format!("Failed to parse Claude response: {e}"))
        })?;

        let response_text = msg_response
            .content
            .first()
            .map(|b| b.text.as_str())
            .unwrap_or("");

        // Parse response by grouping lines between [Speaker]: prefixes
        let mut polished_texts: Vec<String> = Vec::new();
        let mut current_text_parts: Vec<String> = Vec::new();

        for line in response_text.trim().split('\n') {
            let trimmed = line.trim();
            if let Some(caps) = RESPONSE_LINE_RE.captures(trimmed) {
                // New speaker entry — save previous if any
                if !current_text_parts.is_empty() {
                    polished_texts.push(current_text_parts.join("\n\n"));
                }
                current_text_parts = vec![caps[1].to_string()];
            } else if !trimmed.is_empty() {
                // Continuation line (paragraph break in same utterance)
                current_text_parts.push(trimmed.to_string());
            }
            // Empty lines are paragraph separators — skip them
        }

        if !current_text_parts.is_empty() {
            polished_texts.push(current_text_parts.join("\n\n"));
        }

        // If count doesn't match, fall back to originals for safety
        if polished_texts.len() != batch.len() {
            warn!(
                "Polisher response had {} entries but expected {} — using originals",
                polished_texts.len(),
                batch.len()
            );
            return Ok(batch.iter().map(|u| u.text.clone()).collect());
        }

        Ok(polished_texts)
    }
}

impl Polisher for ClaudePolisher {
    async fn polish(&self, transcript: &Transcript) -> Result<Transcript, VoxtractError> {
        let batches = self.create_batches(&transcript.utterances);
        let mut polished_utterances: Vec<Utterance> = Vec::new();

        for batch in &batches {
            let polished_texts = self.polish_batch(batch, transcript).await?;
            for (utterance, new_text) in batch.iter().zip(polished_texts.into_iter()) {
                polished_utterances.push(Utterance::new(
                    &utterance.speaker_label,
                    &new_text,
                    utterance.start_time,
                    utterance.end_time,
                ));
            }
        }

        Ok(Transcript {
            source: transcript.source.clone(),
            speakers: transcript.speakers.clone(),
            utterances: polished_utterances,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_line_regex() {
        let re = &*RESPONSE_LINE_RE;
        let caps = re.captures("[Alice]: Hello world").unwrap();
        assert_eq!(&caps[1], "Hello world");

        let caps = re.captures("[Speaker A]: Testing 123").unwrap();
        assert_eq!(&caps[1], "Testing 123");

        assert!(re.captures("Just plain text").is_none());
    }

    #[test]
    fn test_create_batches() {
        let polisher = ClaudePolisher {
            api_key: String::new(),
            model: String::new(),
            temperature: 0.2,
            batch_size_tokens: 10, // Very small for testing
            client: Client::new(),
        };

        let utterances = vec![
            Utterance::new("A", "short text", 0.0, 1.0),
            Utterance::new("B", "another short text here", 1.0, 2.0),
            Utterance::new("A", "more text to fill the batch up nicely", 2.0, 3.0),
        ];

        let batches = polisher.create_batches(&utterances);
        assert!(batches.len() >= 2); // Should split given small batch size
    }
}
