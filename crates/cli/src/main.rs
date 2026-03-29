use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use console::style;
use dialoguer::Input;
use indicatif::{ProgressBar, ProgressStyle};

use voxtract_application::services::speaker_mapping;
use voxtract_application::services::transcript_pipeline::TranscriptPipelineService;
use voxtract_domain::models::transcript::RawTranscript;
use voxtract_domain::models::video_source::VideoSource;
use voxtract_infra::adapters::assemblyai_transcriber::AssemblyAITranscriber;
use voxtract_infra::adapters::claude_polisher::ClaudePolisher;
use voxtract_infra::adapters::file_transcript_repository::FileTranscriptRepository;
use voxtract_infra::adapters::json_transcript_repository::JsonTranscriptRepository;
use voxtract_infra::adapters::srt_transcript_repository::SrtTranscriptRepository;
use voxtract_infra::adapters::ytdlp_audio_extractor::YtdlpAudioExtractor;
use voxtract_infra::settings::Settings;

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Markdown,
    Json,
    Srt,
}

#[derive(Parser)]
#[command(
    name = "voxtract",
    version,
    about = "YouTube video to polished, speaker-attributed transcript"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Transcribe a YouTube video to a polished transcript
    Transcribe {
        /// YouTube video URL
        url: String,

        /// Comma-separated speaker names in order of appearance
        #[arg(short, long)]
        speakers: Option<String>,

        /// Label of the primary speaker (e.g., 'Speaker A')
        #[arg(short, long)]
        primary: Option<String>,

        /// Output directory for transcript
        #[arg(short, long)]
        output_dir: Option<PathBuf>,

        /// Expected number of speakers
        #[arg(short = 'n', long)]
        expected_speakers: Option<i32>,

        /// Extract and transcribe only — print raw transcript, skip polish/save
        #[arg(long)]
        dry_run: bool,

        /// Output format
        #[arg(short, long, value_enum)]
        format: Option<OutputFormat>,
    },

    /// Transcribe multiple YouTube videos from a text file
    Batch {
        /// File containing one YouTube URL per line (# for comments)
        file: PathBuf,

        /// Output directory for transcripts
        #[arg(short, long)]
        output_dir: Option<PathBuf>,

        /// Extract and transcribe only, skip polish/save
        #[arg(long)]
        dry_run: bool,

        /// Output format
        #[arg(short, long, value_enum)]
        format: Option<OutputFormat>,
    },
}

fn make_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

fn validate_settings(settings: &Settings, dry_run: bool) {
    let mut missing = settings.validate();
    if dry_run {
        missing.retain(|k| k != "ANTHROPIC_API_KEY");
    }
    if !missing.is_empty() {
        eprintln!(
            "{} Missing env vars: {}",
            style("Error:").red().bold(),
            missing.join(", ")
        );
        eprintln!("Copy .env.example to .env and fill in your API keys.");
        std::process::exit(1);
    }
}

fn parse_batch_file(path: &Path) -> Vec<String> {
    let content = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("{} Failed to read file: {e}", style("Error:").red().bold());
        std::process::exit(1);
    });
    content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

fn print_raw_transcript(raw: &RawTranscript) {
    println!("\n{}", style("Raw Transcript (dry-run)").bold());
    println!("{}", style(format!("Source: {}", raw.source.url)).dim());
    println!(
        "{}",
        style(format!("Speakers: {}", raw.speaker_labels().join(", "))).dim()
    );
    println!();

    for utterance in &raw.utterances {
        let minutes = (utterance.start_time / 60.0) as u32;
        let seconds = (utterance.start_time % 60.0) as u32;
        print!("{} ", style(format!("[{minutes:02}:{seconds:02}]")).dim());
        print!("{} ", style(format!("{}:", utterance.speaker_label)).bold());
        println!("{}", utterance.text);
    }

    println!(
        "\n{}",
        style(format!("{} utterances total", raw.utterances.len())).dim()
    );
}

fn interactive_speaker_mapping(raw: &RawTranscript) -> HashMap<String, String> {
    let samples = speaker_mapping::get_speaker_samples(raw, 3);
    let stats = speaker_mapping::get_speaker_stats(raw);

    println!("\n{}", style("Speaker Identification").bold());
    println!("Here are sample utterances from each detected speaker:\n");

    for label in raw.speaker_labels() {
        let speaking_time = stats
            .iter()
            .find(|(l, _)| l == &label)
            .map(|(_, t)| *t)
            .unwrap_or(0.0);

        println!(
            "{}",
            style(format!("{label} ({speaking_time:.0}s speaking time)")).bold()
        );

        if let Some(texts) = samples.get(&label) {
            for text in texts {
                let truncated = if text.len() > 120 {
                    format!("{}...", &text[..120])
                } else {
                    text.clone()
                };
                println!("  {}", style(truncated).dim());
            }
        }
        println!();
    }

    let mut name_map = HashMap::new();
    for label in raw.speaker_labels() {
        let name: String = Input::new()
            .with_prompt(format!("Name for {label}"))
            .default(label.clone())
            .interact_text()
            .unwrap();
        name_map.insert(label, name);
    }

    name_map
}

/// Macro to run the transcribe pipeline with any repository type.
/// This avoids code duplication since the pipeline is generic over the repository.
macro_rules! run_transcribe {
    ($settings:expr, $expected_speakers:expr, $url:expr, $speakers:expr,
     $primary:expr, $dry_run:expr, $repo:expr) => {{
        let extractor = YtdlpAudioExtractor::new(&std::env::temp_dir().join("voxtract"));
        let transcriber =
            AssemblyAITranscriber::new(&$settings.assemblyai_api_key, $expected_speakers);
        let polisher = ClaudePolisher::new(&$settings.anthropic_api_key);
        let pipeline = TranscriptPipelineService::new(extractor, transcriber, polisher, $repo);

        let spinner = make_spinner("Extracting audio...");
        let raw = pipeline.extract_and_transcribe(&$url).await;
        spinner.finish_and_clear();

        let raw = match raw {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{} {e}", style("Error:").red());
                std::process::exit(1);
            }
        };

        let n_speakers = raw.speaker_labels().len();
        println!(
            "{} Found {} speaker{}, {} utterances.",
            style("Transcription complete.").green(),
            n_speakers,
            if n_speakers != 1 { "s" } else { "" },
            raw.utterances.len()
        );

        if $dry_run {
            print_raw_transcript(&raw);
            return;
        }

        let name_map = if let Some(ref speakers_str) = $speakers {
            let names: Vec<&str> = speakers_str.split(',').map(|s| s.trim()).collect();
            raw.speaker_labels()
                .into_iter()
                .zip(names.into_iter())
                .map(|(label, name)| (label, name.to_string()))
                .collect::<HashMap<String, String>>()
        } else {
            interactive_speaker_mapping(&raw)
        };

        let transcript = speaker_mapping::apply_mapping(&raw, &name_map, $primary.as_deref());

        let spinner = make_spinner("Polishing transcript...");
        let result = pipeline.polish_and_save(&transcript).await;
        spinner.finish_and_clear();

        match result {
            Ok(path) => {
                println!(
                    "{} Saved to: {}",
                    style("Done!").green().bold(),
                    path.display()
                );
            }
            Err(e) => {
                eprintln!("{} {e}", style("Error:").red());
                std::process::exit(1);
            }
        }
    }};
}

/// Macro for batch processing with any repository type.
macro_rules! run_batch {
    ($settings:expr, $urls:expr, $dry_run:expr, $repo:expr) => {{
        let extractor = YtdlpAudioExtractor::new(&std::env::temp_dir().join("voxtract"));
        let transcriber = AssemblyAITranscriber::new(&$settings.assemblyai_api_key, None);
        let polisher = ClaudePolisher::new(&$settings.anthropic_api_key);
        let pipeline = TranscriptPipelineService::new(extractor, transcriber, polisher, $repo);

        let total = $urls.len();
        println!(
            "{}",
            style(format!(
                "Processing {total} video{}...",
                if total != 1 { "s" } else { "" }
            ))
            .bold()
        );
        println!();

        let mut succeeded = 0u32;
        let mut failed = 0u32;
        let mut skipped = 0u32;

        for (i, url) in $urls.iter().enumerate() {
            let prefix = style(format!("[{}/{}]", i + 1, total)).dim();

            // Check if already processed
            if !$dry_run {
                if let Ok(vid) = VideoSource::new(url) {
                    if let Ok(entries) = std::fs::read_dir(&$settings.output_dir) {
                        let exists = entries.filter_map(|e| e.ok()).any(|e| {
                            e.file_name()
                                .to_str()
                                .map_or(false, |n| n.contains(&vid.video_id))
                        });
                        if exists {
                            println!(
                                "{prefix} {} {url} (already exists)",
                                style("Skipping").yellow()
                            );
                            skipped += 1;
                            continue;
                        }
                    }
                }
            }

            let spinner = make_spinner(&format!("{prefix} Extracting and transcribing..."));
            let raw = pipeline.extract_and_transcribe(url).await;
            spinner.finish_and_clear();

            match raw {
                Ok(raw) => {
                    let title = if raw.source.title.is_empty() {
                        &raw.source.video_id
                    } else {
                        &raw.source.title
                    };
                    let n_speakers = raw.speaker_labels().len();

                    if $dry_run {
                        println!(
                            "{prefix} {} — {} speaker{}, {} utterances",
                            style(title).green(),
                            n_speakers,
                            if n_speakers != 1 { "s" } else { "" },
                            raw.utterances.len()
                        );
                        succeeded += 1;
                        continue;
                    }

                    // Auto-map speakers (no interactive in batch mode)
                    let transcript = speaker_mapping::apply_mapping(&raw, &HashMap::new(), None);

                    let spinner = make_spinner(&format!("{prefix} Polishing..."));
                    let result = pipeline.polish_and_save(&transcript).await;
                    spinner.finish_and_clear();

                    match result {
                        Ok(path) => {
                            println!(
                                "{prefix} {} -> {}",
                                style(title).green(),
                                path.file_name().unwrap_or_default().to_string_lossy()
                            );
                            succeeded += 1;
                        }
                        Err(e) => {
                            eprintln!("{prefix} {} {url} — {e}", style("Failed:").red());
                            failed += 1;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{prefix} {} {url} — {e}", style("Failed:").red());
                    failed += 1;
                }
            }
        }

        println!();
        let mut parts = Vec::new();
        if succeeded > 0 {
            parts.push(format!(
                "{}",
                style(format!("{succeeded} succeeded")).green()
            ));
        }
        if failed > 0 {
            parts.push(format!("{}", style(format!("{failed} failed")).red()));
        }
        if skipped > 0 {
            parts.push(format!("{}", style(format!("{skipped} skipped")).yellow()));
        }
        println!("{} {}", style("Done!").bold(), parts.join(", "));
    }};
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();

    match cli.command {
        Commands::Transcribe {
            url,
            speakers,
            primary,
            output_dir,
            expected_speakers,
            dry_run,
            format,
        } => {
            let mut settings = Settings::from_env();
            if let Some(dir) = output_dir {
                settings.output_dir = dir;
            }
            if let Some(fmt) = &format {
                settings.output_format = match fmt {
                    OutputFormat::Markdown => "markdown".to_string(),
                    OutputFormat::Json => "json".to_string(),
                    OutputFormat::Srt => "srt".to_string(),
                };
            }
            validate_settings(&settings, dry_run);

            match format.unwrap_or(OutputFormat::Markdown) {
                OutputFormat::Markdown => {
                    let repo = FileTranscriptRepository::new(&settings.output_dir);
                    run_transcribe!(
                        settings,
                        expected_speakers,
                        url,
                        speakers,
                        primary,
                        dry_run,
                        repo
                    );
                }
                OutputFormat::Json => {
                    let repo = JsonTranscriptRepository::new(&settings.output_dir);
                    run_transcribe!(
                        settings,
                        expected_speakers,
                        url,
                        speakers,
                        primary,
                        dry_run,
                        repo
                    );
                }
                OutputFormat::Srt => {
                    let repo = SrtTranscriptRepository::new(&settings.output_dir);
                    run_transcribe!(
                        settings,
                        expected_speakers,
                        url,
                        speakers,
                        primary,
                        dry_run,
                        repo
                    );
                }
            }
        }
        Commands::Batch {
            file,
            output_dir,
            dry_run,
            format,
        } => {
            let mut settings = Settings::from_env();
            if let Some(dir) = output_dir {
                settings.output_dir = dir;
            }
            if let Some(fmt) = &format {
                settings.output_format = match fmt {
                    OutputFormat::Markdown => "markdown".to_string(),
                    OutputFormat::Json => "json".to_string(),
                    OutputFormat::Srt => "srt".to_string(),
                };
            }
            validate_settings(&settings, dry_run);

            let urls = parse_batch_file(&file);
            if urls.is_empty() {
                println!("{}", style("No URLs found in file.").yellow());
                return;
            }

            match format.unwrap_or(OutputFormat::Markdown) {
                OutputFormat::Markdown => {
                    let repo = FileTranscriptRepository::new(&settings.output_dir);
                    run_batch!(settings, urls, dry_run, repo);
                }
                OutputFormat::Json => {
                    let repo = JsonTranscriptRepository::new(&settings.output_dir);
                    run_batch!(settings, urls, dry_run, repo);
                }
                OutputFormat::Srt => {
                    let repo = SrtTranscriptRepository::new(&settings.output_dir);
                    run_batch!(settings, urls, dry_run, repo);
                }
            }
        }
    }
}
