use async_trait::async_trait;
use crate::errors::Yt2ptError;
use crate::models::audio_file::AudioFile;
use crate::models::video_source::VideoSource;

#[async_trait]
pub trait AudioExtractor: Send + Sync {
    async fn extract(
        &self,
        source: &VideoSource,
    ) -> Result<AudioFile, Yt2ptError>;
}
