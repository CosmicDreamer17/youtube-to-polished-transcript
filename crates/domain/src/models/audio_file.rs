use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AudioFile {
    pub path: PathBuf,
    pub duration_seconds: f64,
    pub format: String,
    pub source_title: String,
}
