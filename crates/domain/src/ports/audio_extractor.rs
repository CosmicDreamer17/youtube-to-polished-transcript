use crate::errors::VoxtractError;
use crate::models::audio_file::AudioFile;
use crate::models::video_source::VideoSource;

pub trait AudioExtractor: Send + Sync {
    fn extract(
        &self,
        source: &VideoSource,
    ) -> impl std::future::Future<Output = Result<AudioFile, VoxtractError>> + Send;
}
