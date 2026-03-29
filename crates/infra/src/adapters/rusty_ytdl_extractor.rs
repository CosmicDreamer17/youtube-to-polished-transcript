use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rusty_ytdl::Video;
use yt2pt_domain::errors::Yt2ptError;
use yt2pt_domain::models::audio_file::AudioFile;
use yt2pt_domain::models::video_source::VideoSource;
use yt2pt_domain::ports::audio_extractor::AudioExtractor;

pub struct RustyYtdlExtractor {
    output_dir: PathBuf,
}

impl RustyYtdlExtractor {
    pub fn new(output_dir: &Path) -> Self {
        Self {
            output_dir: output_dir.to_path_buf(),
        }
    }
}

#[async_trait]
impl AudioExtractor for RustyYtdlExtractor {
    async fn extract(&self, source: &VideoSource) -> Result<AudioFile, Yt2ptError> {
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to create output dir: {e}")))?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".parse().unwrap());
        
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to build reqwest client: {e}")))?;

        let video = Video::new_with_options(
            &source.url,
            rusty_ytdl::VideoOptions {
                request_options: rusty_ytdl::RequestOptions {
                    client: Some(client),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .map_err(|e| Yt2ptError::Extraction(format!("Failed to initialize rusty-ytdl: {e}")))?;

        let info = video
            .get_info()
            .await
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to get video info: {e}")))?;

        let title = info.video_details.title.clone();
        let duration = info.video_details.length_seconds.parse::<f64>().unwrap_or(0.0);

        let output_path = self.output_dir.join(format!("{}.wav", source.video_id));

        let mut download_options = rusty_ytdl::VideoOptions::default();
        download_options.filter = rusty_ytdl::VideoSearchOptions::Audio;

        video
            .download(&output_path)
            .await
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to download audio: {e}")))?;

        Ok(AudioFile {
            path: output_path,
            duration_seconds: duration,
            format: "wav".to_string(), // Note: may actually be m4a/webm if using .download() directly
            source_title: title,
        })
    }
}
