use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::errors::VoxtractError;

static YOUTUBE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/embed/)([a-zA-Z0-9_-]{11})")
        .unwrap()
});

static BARE_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([a-zA-Z0-9_-]{11})$").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSource {
    pub url: String,
    pub title: String,
    pub video_id: String,
}

impl VideoSource {
    pub fn new(url: &str) -> Result<Self, VoxtractError> {
        let video_id = Self::extract_video_id(url)?;
        Ok(Self {
            url: url.to_string(),
            title: String::new(),
            video_id,
        })
    }

    pub fn with_title(url: &str, title: &str) -> Result<Self, VoxtractError> {
        let video_id = Self::extract_video_id(url)?;
        Ok(Self {
            url: url.to_string(),
            title: title.to_string(),
            video_id,
        })
    }

    pub fn with_all(url: &str, title: &str, video_id: &str) -> Self {
        Self {
            url: url.to_string(),
            title: title.to_string(),
            video_id: video_id.to_string(),
        }
    }

    fn extract_video_id(url: &str) -> Result<String, VoxtractError> {
        if let Some(caps) = YOUTUBE_RE.captures(url) {
            return Ok(caps[1].to_string());
        }
        if let Some(caps) = BARE_ID_RE.captures(url) {
            return Ok(caps[1].to_string());
        }
        Err(VoxtractError::InvalidInput(format!(
            "Could not extract video ID from: {url}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard_url() {
        let vs = VideoSource::new("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap();
        assert_eq!(vs.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn parse_short_url() {
        let vs = VideoSource::new("https://youtu.be/dQw4w9WgXcQ").unwrap();
        assert_eq!(vs.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn parse_embed_url() {
        let vs = VideoSource::new("https://www.youtube.com/embed/dQw4w9WgXcQ").unwrap();
        assert_eq!(vs.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn parse_bare_id() {
        let vs = VideoSource::new("dQw4w9WgXcQ").unwrap();
        assert_eq!(vs.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn reject_invalid_url() {
        assert!(VideoSource::new("not-a-valid-url").is_err());
    }

    #[test]
    fn with_title_sets_both() {
        let vs =
            VideoSource::with_title("https://www.youtube.com/watch?v=dQw4w9WgXcQ", "Test Video")
                .unwrap();
        assert_eq!(vs.video_id, "dQw4w9WgXcQ");
        assert_eq!(vs.title, "Test Video");
    }
}
