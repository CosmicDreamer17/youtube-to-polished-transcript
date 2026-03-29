use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use console::style;
use dialoguer::Input;
use indicatif::{ProgressBar, ProgressStyle};
use uuid::Uuid;

use voxtract_application::services::speaker_mapping;
use voxtract_application::services::transcript_pipeline::{
    PipelineResult, TranscriptPipelineService,
};
use voxtract_domain::models::manifest::{ManifestEntry, ManifestSpeaker};
use voxtract_domain::models::transcript::RawTranscript;
use voxtract_domain::models::video_source::VideoSource;
use voxtract_infra::adapters::assemblyai_transcriber::AssemblyAITranscriber;
use voxtract_infra::adapters::claude_polisher::ClaudePolisher;
use voxtract_infra::adapters::deepgram_transcriber::DeepgramTranscriber;
use voxtract_infra::adapters::file_transcript_repository::FileTranscriptRepository;
use voxtract_infra::adapters::json_transcript_repository::JsonTranscriptRepository;
use voxtract_infra::adapters::manifest_repository::FileManifestRepository;
use voxtract_infra::adapters::ollama_polisher::OllamaPolisher;
use voxtract_infra::adapters::openai_polisher::OpenAIPolisher;
use voxtract_infra::adapters::srt_transcript_repository::SrtTranscriptRepository;
use voxtract_infra::adapters::ytdlp_audio_extractor::YtdlpAudioExtractor;
use voxtract_infra::settings::Settings;

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Markdown,
    Json,
    Srt,
}

impl OutputFormat {
    fn as_str(&self) -> &str {
        match self {
            OutputFormat::Markdown => "markdown",
            OutputFormat::Json => "json",
            OutputFormat::Srt => "srt",
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum TranscriberChoice {
    Assemblyai,
    Deepgram,
}

#[derive(Debug, Clone, ValueEnum)]
enum PolisherChoice {
    Claude,
    Openai,
    Ollama,
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

        /// Transcription provider
        #[arg(long, value_enum, default_value = "assemblyai")]
        transcriber: TranscriberChoice,

        /// Polishing provider
        #[arg(long, value_enum, default_value = "claude")]
        polisher: PolisherChoice,

        /// Ollama model name (only used with --polisher ollama)
        #[arg(long, default_value = "llama3.1")]
        ollama_model: String,
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

        /// Transcription provider
        #[arg(long, value_enum, default_value = "assemblyai")]
        transcriber: TranscriberChoice,

        /// Polishing provider
        #[arg(long, value_enum, default_value = "claude")]
        polisher: PolisherChoice,

        /// Ollama model name (only used with --polisher ollama)
        #[arg(long, default_value = "llama3.1")]
        ollama_model: String,
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

fn validate_settings(settings: &Settings, transcriber: &str, polisher: &str, dry_run: bool) {
    let missing = settings.validate_for(transcriber, polisher, dry_run);
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

fn build_manifest_entry(
    raw: &RawTranscript,
    transcript: &voxtract_domain::models::transcript::Transcript,
    result: &PipelineResult,
    output_format: &str,
    batch_id: Option<&str>,
) -> ManifestEntry {
    ManifestEntry {
        video_title: transcript.source.title.clone(),
        youtube_url: transcript.source.url.clone(),
        video_id: transcript.source.video_id.clone(),
        speakers: transcript
            .speakers
            .iter()
            .map(|s| ManifestSpeaker {
                label: s.label.clone(),
                name: s.name().to_string(),
            })
            .collect(),
        primary_speaker: transcript.primary_speaker().map(|s| s.name().to_string()),
        duration_seconds: raw.audio_duration_seconds,
        date_transcribed: Utc::now().format("%Y-%m-%d").to_string(),
        assemblyai_cost_usd: ManifestEntry::compute_assemblyai_cost(raw.audio_duration_seconds),
        claude_cost_usd: ManifestEntry::compute_claude_cost(
            result.input_tokens,
            result.output_tokens,
        ),
        claude_input_tokens: result.input_tokens,
        claude_output_tokens: result.output_tokens,
        output_file: result
            .output_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        output_format: output_format.to_string(),
        batch_id: batch_id.map(|s| s.to_string()),
    }
}

/// Run the transcribe pipeline with any combination of transcriber, polisher, and repository.
/// This macro handles the generic type explosion from the pipeline service.
macro_rules! run_pipeline {
    ($transcriber:expr, $polisher:expr, $repo:expr, $settings:expr,
     $url:expr, $speakers:expr, $primary:expr, $dry_run:expr, $fmt_str:expr) => {{
        let manifest_repo = FileManifestRepository::new(&$settings.output_dir);
        let extractor = YtdlpAudioExtractor::new(&std::env::temp_dir().join("voxtract"));
        let pipeline = TranscriptPipelineService::new(extractor, $transcriber, $polisher, $repo);

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
            Ok(pipeline_result) => {
                println!(
                    "{} Saved to: {}",
                    style("Done!").green().bold(),
                    pipeline_result.output_path.display()
                );
                let entry =
                    build_manifest_entry(&raw, &transcript, &pipeline_result, $fmt_str, None);
                if let Err(e) = manifest_repo.append(&entry).await {
                    eprintln!(
                        "{} Failed to update manifest: {e}",
                        style("Warning:").yellow()
                    );
                }
            }
            Err(e) => {
                eprintln!("{} {e}", style("Error:").red());
                std::process::exit(1);
            }
        }
    }};
}

macro_rules! run_batch_pipeline {
    ($transcriber:expr, $polisher:expr, $repo:expr, $settings:expr,
     $urls:expr, $dry_run:expr, $fmt_str:expr) => {{
        let manifest_repo = FileManifestRepository::new(&$settings.output_dir);
        let batch_id = Uuid::new_v4().to_string();
        let extractor = YtdlpAudioExtractor::new(&std::env::temp_dir().join("voxtract"));
        let pipeline = TranscriptPipelineService::new(extractor, $transcriber, $polisher, $repo);

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

                    let transcript = speaker_mapping::apply_mapping(&raw, &HashMap::new(), None);

                    let spinner = make_spinner(&format!("{prefix} Polishing..."));
                    let result = pipeline.polish_and_save(&transcript).await;
                    spinner.finish_and_clear();

                    match result {
                        Ok(pipeline_result) => {
                            println!(
                                "{prefix} {} -> {}",
                                style(title).green(),
                                pipeline_result
                                    .output_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                            );
                            let entry = build_manifest_entry(
                                &raw,
                                &transcript,
                                &pipeline_result,
                                $fmt_str,
                                Some(&batch_id),
                            );
                            if let Err(e) = manifest_repo.append(&entry).await {
                                eprintln!(
                                    "{} Failed to update manifest: {e}",
                                    style("Warning:").yellow()
                                );
                            }
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

/// Dispatch to the correct combination of transcriber + polisher + repository.
/// Each combination produces a unique monomorphized pipeline type, so we use
/// macros to generate all the variants without boxing.
macro_rules! dispatch {
    (transcribe: $tc:expr, $pc:expr, $repo:expr, $settings:expr,
     $url:expr, $speakers:expr, $primary:expr, $dry_run:expr, $fmt_str:expr, $ollama_model:expr) => {
        match ($tc, $pc) {
            (TranscriberChoice::Assemblyai, PolisherChoice::Claude) => {
                let t = AssemblyAITranscriber::new(&$settings.assemblyai_api_key, None);
                let p = ClaudePolisher::new(&$settings.anthropic_api_key);
                run_pipeline!(
                    t, p, $repo, $settings, $url, $speakers, $primary, $dry_run, $fmt_str
                );
            }
            (TranscriberChoice::Assemblyai, PolisherChoice::Openai) => {
                let t = AssemblyAITranscriber::new(&$settings.assemblyai_api_key, None);
                let p = OpenAIPolisher::new(&$settings.openai_api_key);
                run_pipeline!(
                    t, p, $repo, $settings, $url, $speakers, $primary, $dry_run, $fmt_str
                );
            }
            (TranscriberChoice::Assemblyai, PolisherChoice::Ollama) => {
                let t = AssemblyAITranscriber::new(&$settings.assemblyai_api_key, None);
                let p = OllamaPolisher::new(&$ollama_model);
                run_pipeline!(
                    t, p, $repo, $settings, $url, $speakers, $primary, $dry_run, $fmt_str
                );
            }
            (TranscriberChoice::Deepgram, PolisherChoice::Claude) => {
                let t = DeepgramTranscriber::new(&$settings.deepgram_api_key);
                let p = ClaudePolisher::new(&$settings.anthropic_api_key);
                run_pipeline!(
                    t, p, $repo, $settings, $url, $speakers, $primary, $dry_run, $fmt_str
                );
            }
            (TranscriberChoice::Deepgram, PolisherChoice::Openai) => {
                let t = DeepgramTranscriber::new(&$settings.deepgram_api_key);
                let p = OpenAIPolisher::new(&$settings.openai_api_key);
                run_pipeline!(
                    t, p, $repo, $settings, $url, $speakers, $primary, $dry_run, $fmt_str
                );
            }
            (TranscriberChoice::Deepgram, PolisherChoice::Ollama) => {
                let t = DeepgramTranscriber::new(&$settings.deepgram_api_key);
                let p = OllamaPolisher::new(&$ollama_model);
                run_pipeline!(
                    t, p, $repo, $settings, $url, $speakers, $primary, $dry_run, $fmt_str
                );
            }
        }
    };
    (batch: $tc:expr, $pc:expr, $repo:expr, $settings:expr,
     $urls:expr, $dry_run:expr, $fmt_str:expr, $ollama_model:expr) => {
        match ($tc, $pc) {
            (TranscriberChoice::Assemblyai, PolisherChoice::Claude) => {
                let t = AssemblyAITranscriber::new(&$settings.assemblyai_api_key, None);
                let p = ClaudePolisher::new(&$settings.anthropic_api_key);
                run_batch_pipeline!(t, p, $repo, $settings, $urls, $dry_run, $fmt_str);
            }
            (TranscriberChoice::Assemblyai, PolisherChoice::Openai) => {
                let t = AssemblyAITranscriber::new(&$settings.assemblyai_api_key, None);
                let p = OpenAIPolisher::new(&$settings.openai_api_key);
                run_batch_pipeline!(t, p, $repo, $settings, $urls, $dry_run, $fmt_str);
            }
            (TranscriberChoice::Assemblyai, PolisherChoice::Ollama) => {
                let t = AssemblyAITranscriber::new(&$settings.assemblyai_api_key, None);
                let p = OllamaPolisher::new(&$ollama_model);
                run_batch_pipeline!(t, p, $repo, $settings, $urls, $dry_run, $fmt_str);
            }
            (TranscriberChoice::Deepgram, PolisherChoice::Claude) => {
                let t = DeepgramTranscriber::new(&$settings.deepgram_api_key);
                let p = ClaudePolisher::new(&$settings.anthropic_api_key);
                run_batch_pipeline!(t, p, $repo, $settings, $urls, $dry_run, $fmt_str);
            }
            (TranscriberChoice::Deepgram, PolisherChoice::Openai) => {
                let t = DeepgramTranscriber::new(&$settings.deepgram_api_key);
                let p = OpenAIPolisher::new(&$settings.openai_api_key);
                run_batch_pipeline!(t, p, $repo, $settings, $urls, $dry_run, $fmt_str);
            }
            (TranscriberChoice::Deepgram, PolisherChoice::Ollama) => {
                let t = DeepgramTranscriber::new(&$settings.deepgram_api_key);
                let p = OllamaPolisher::new(&$ollama_model);
                run_batch_pipeline!(t, p, $repo, $settings, $urls, $dry_run, $fmt_str);
            }
        }
    };
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
            expected_speakers: _,
            dry_run,
            format,
            transcriber,
            polisher,
            ollama_model,
        } => {
            let mut settings = Settings::from_env();
            if let Some(dir) = output_dir {
                settings.output_dir = dir;
            }
            let fmt = format.unwrap_or(OutputFormat::Markdown);
            settings.output_format = fmt.as_str().to_string();

            let tc_name = match transcriber {
                TranscriberChoice::Assemblyai => "assemblyai",
                TranscriberChoice::Deepgram => "deepgram",
            };
            let pc_name = match polisher {
                PolisherChoice::Claude => "claude",
                PolisherChoice::Openai => "openai",
                PolisherChoice::Ollama => "ollama",
            };
            validate_settings(&settings, tc_name, pc_name, dry_run);

            match fmt {
                OutputFormat::Markdown => {
                    let repo = FileTranscriptRepository::new(&settings.output_dir);
                    dispatch!(transcribe: transcriber, polisher, repo, settings, url, speakers, primary, dry_run, "markdown", ollama_model);
                }
                OutputFormat::Json => {
                    let repo = JsonTranscriptRepository::new(&settings.output_dir);
                    dispatch!(transcribe: transcriber, polisher, repo, settings, url, speakers, primary, dry_run, "json", ollama_model);
                }
                OutputFormat::Srt => {
                    let repo = SrtTranscriptRepository::new(&settings.output_dir);
                    dispatch!(transcribe: transcriber, polisher, repo, settings, url, speakers, primary, dry_run, "srt", ollama_model);
                }
            }
        }
        Commands::Batch {
            file,
            output_dir,
            dry_run,
            format,
            transcriber,
            polisher,
            ollama_model,
        } => {
            let mut settings = Settings::from_env();
            if let Some(dir) = output_dir {
                settings.output_dir = dir;
            }
            let fmt = format.unwrap_or(OutputFormat::Markdown);
            settings.output_format = fmt.as_str().to_string();

            let tc_name = match transcriber {
                TranscriberChoice::Assemblyai => "assemblyai",
                TranscriberChoice::Deepgram => "deepgram",
            };
            let pc_name = match polisher {
                PolisherChoice::Claude => "claude",
                PolisherChoice::Openai => "openai",
                PolisherChoice::Ollama => "ollama",
            };
            validate_settings(&settings, tc_name, pc_name, dry_run);

            let urls = parse_batch_file(&file);
            if urls.is_empty() {
                println!("{}", style("No URLs found in file.").yellow());
                return;
            }

            match fmt {
                OutputFormat::Markdown => {
                    let repo = FileTranscriptRepository::new(&settings.output_dir);
                    dispatch!(batch: transcriber, polisher, repo, settings, urls, dry_run, "markdown", ollama_model);
                }
                OutputFormat::Json => {
                    let repo = JsonTranscriptRepository::new(&settings.output_dir);
                    dispatch!(batch: transcriber, polisher, repo, settings, urls, dry_run, "json", ollama_model);
                }
                OutputFormat::Srt => {
                    let repo = SrtTranscriptRepository::new(&settings.output_dir);
                    dispatch!(batch: transcriber, polisher, repo, settings, urls, dry_run, "srt", ollama_model);
                }
            }
        }
    }
}
