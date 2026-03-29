use crate::errors::VoxtractError;
use crate::models::audio_file::AudioFile;
use crate::models::transcript::RawTranscript;
use crate::models::video_source::VideoSource;

pub trait Transcriber: Send + Sync {
    fn transcribe(
        &self,
        audio: &AudioFile,
        source: &VideoSource,
    ) -> impl std::future::Future<Output = Result<RawTranscript, VoxtractError>> + Send;
}
