# YouTube to Polished Transcript (`yt2pt`)

YouTube video → polished, speaker-attributed transcript.

`yt2pt` is a Rust CLI that converts YouTube videos into clean, readable transcripts with speaker identification. It runs a 5-stage pipeline with pluggable providers — you choose which transcription service and which LLM to use.

## Pipeline

```
YouTube URL
    │
    ▼
┌─────────────────────┐
│ 1. Audio Extraction  │  yt-dlp → 16kHz mono WAV
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ 2. Transcription +   │  AssemblyAI or Deepgram
│    Speaker Diarize   │  → timestamped utterances with speaker labels
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ 3. Speaker Mapping   │  Interactive prompts or --speakers flag
│                      │  → "Speaker A" becomes "Alice"
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ 4. Polishing         │  Claude, OpenAI, Gemini, or Ollama
│                      │  → removes filler words, false starts, repetitions
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ 5. Output            │  Markdown, JSON, or SRT
│    + Manifest        │  → transcript file + manifest.json + index.html dashboard
└─────────────────────┘
```

## Providers

### Transcribers (`--transcriber`)

Transcribers handle speech-to-text conversion with speaker diarization (identifying who said what).

| Provider | Flag | Model | Pricing | API Key Env Var | Notes |
|----------|------|-------|---------|-----------------|-------|
| **AssemblyAI** (default) | `--transcriber assemblyai` | universal-3-pro | ~$0.29/hr | `ASSEMBLYAI_API_KEY` | Upload → poll workflow. Best accuracy for English. |
| **Deepgram** | `--transcriber deepgram` | Nova-3 | ~$0.25/hr | `DEEPGRAM_API_KEY` | Single-request workflow. Fast, competitive accuracy. |

Both transcribers produce the same output: a list of `Utterance` objects with `speaker_label`, `text`, `start_time`, and `end_time`. Speaker labels are auto-generated as "Speaker A", "Speaker B", etc.

### Polishers (`--polisher`)

Polishers clean up raw speech-to-text output using an LLM. All polishers use the same system prompt with 11 rules (remove filler words, fix false starts, preserve the speaker's voice, etc.). They batch utterances into ~2000-token chunks for efficiency.

| Provider | Flag | Model | Pricing | API Key Env Var | Notes |
|----------|------|-------|---------|-----------------|-------|
| **Claude** (default) | `--polisher claude` | claude-sonnet-4 | $3/$15 per M tokens | `ANTHROPIC_API_KEY` | Anthropic Messages API. Best at preserving speaker voice. |
| **OpenAI** | `--polisher openai` | gpt-4o | $2.50/$10 per M tokens | `OPENAI_API_KEY` | Chat Completions API. Widely available. |
| **Gemini** | `--polisher gemini` | gemini-2.5-flash | Free tier available | `GOOGLE_API_KEY` | Google Generative Language API. Cost-effective. |
| **Ollama** | `--polisher ollama` | llama3.1 (configurable) | Free (local) | None | Runs locally via [Ollama](https://ollama.com). Use `--ollama-model` to pick model. |

All polishers return the polished transcript plus token usage counts (for cost tracking in the manifest).

### Mixing and matching

Any transcriber works with any polisher. Choose based on your API keys, budget, or quality preferences:

```bash
# Default: AssemblyAI + Claude
yt2pt transcribe "https://youtube.com/watch?v=..."

# Budget option: Deepgram + Gemini
yt2pt transcribe "https://youtube.com/watch?v=..." --transcriber deepgram --polisher gemini

# Free local polishing: AssemblyAI + Ollama
yt2pt transcribe "https://youtube.com/watch?v=..." --polisher ollama --ollama-model llama3.1

# OpenAI everything: Deepgram + GPT-4o
yt2pt transcribe "https://youtube.com/watch?v=..." --transcriber deepgram --polisher openai
```

## Prerequisites

- **Rust** 1.85+ ([install](https://rustup.rs/))
- **FFmpeg** — for audio conversion
  ```bash
  # macOS
  brew install ffmpeg
  # Ubuntu/Debian
  sudo apt install ffmpeg
  ```
- **yt-dlp** — for YouTube audio download
  ```bash
  # macOS
  brew install yt-dlp
  # Ubuntu/Debian
  sudo apt install yt-dlp
  # Or via pip
  pip install yt-dlp
  ```

## Quickstart

```bash
# 1. Clone and build
git clone https://github.com/CosmicDreamer17/youtube-to-polished-transcript.git
cd youtube-to-polished-transcript
cargo build --release

# 2. Set up API keys
cp .env.example .env
# Edit .env — only the keys for your chosen providers are needed

# 3. Transcribe a video
./target/release/yt2pt transcribe "https://www.youtube.com/watch?v=jNQXAC9IVRw"
```

Or install globally: `cargo install --path crates/cli` (installs the `yt2pt` binary)

## Configuration

Environment variables (loaded from `.env` automatically):

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `ASSEMBLYAI_API_KEY` | If using AssemblyAI | — | [assemblyai.com](https://www.assemblyai.com/) |
| `DEEPGRAM_API_KEY` | If using Deepgram | — | [deepgram.com](https://deepgram.com/) |
| `ANTHROPIC_API_KEY` | If using Claude | — | [console.anthropic.com](https://console.anthropic.com/) |
| `OPENAI_API_KEY` | If using OpenAI | — | [platform.openai.com](https://platform.openai.com/) |
| `GOOGLE_API_KEY` | If using Gemini | — | [aistudio.google.com](https://aistudio.google.com/) |
| `YT2PT_OUTPUT_DIR` | No | `./output` | Directory for saved transcripts |
| `YT2PT_OUTPUT_FORMAT` | No | `markdown` | Default output format |
| `OLLAMA_BASE_URL` | No | `http://localhost:11434` | Ollama server URL |

Only the API keys for your chosen providers are validated. For example, using `--transcriber deepgram --polisher ollama` only requires `DEEPGRAM_API_KEY`.

## Usage

### Single video

```bash
# Interactive speaker mapping (prompts you to name each speaker)
yt2pt transcribe "https://www.youtube.com/watch?v=..."

# Pre-specified speakers
yt2pt transcribe "https://www.youtube.com/watch?v=..." -s "Alice,Bob" -p "Speaker A"

# Hint expected number of speakers
yt2pt transcribe "https://www.youtube.com/watch?v=..." -n 3

# Dry run — transcribe only, skip polishing (no polisher API key needed)
yt2pt transcribe "https://www.youtube.com/watch?v=..." --dry-run

# JSON output
yt2pt transcribe "https://www.youtube.com/watch?v=..." -f json

# SRT subtitles
yt2pt transcribe "https://www.youtube.com/watch?v=..." -f srt

# Custom output directory
yt2pt transcribe "https://www.youtube.com/watch?v=..." -o ./transcripts
```

### Batch processing

```bash
yt2pt batch examples/urls.txt
yt2pt batch examples/urls.txt -f json -o ./transcripts --transcriber deepgram --polisher gemini
```

Input file: one YouTube URL per line. Lines starting with `#` are comments. Already-processed videos are automatically skipped. In batch mode, speakers are auto-labeled (no interactive prompts).

## Output

Each transcription produces three things in the output directory:

1. **Transcript file** (`*.md`, `*.json`, or `*.srt`) — the polished transcript
2. **`manifest.json`** — structured inventory of all transcripts with metadata:
   ```json
   {
     "video_title": "Me at the zoo",
     "youtube_url": "https://www.youtube.com/watch?v=jNQXAC9IVRw",
     "speakers": [{"label": "Speaker A", "name": "Jawed"}],
     "primary_speaker": "Jawed",
     "duration_seconds": 19.0,
     "assemblyai_cost_usd": 0.0015,
     "claude_cost_usd": 0.0021,
     "claude_input_tokens": 412,
     "claude_output_tokens": 59,
     "output_format": "markdown",
     "batch_id": null
   }
   ```
3. **`index.html`** — a self-contained dark-themed dashboard with summary stats, sortable table, cost breakdowns, and links to source videos. Open it in any browser.

The manifest accumulates over time — each new transcription appends to the existing data and regenerates the HTML.

## Architecture

Cargo workspace with 4 crates enforcing hexagonal architecture at compile time:

```
crates/
├── domain/       ← models, errors, port traits (zero I/O dependencies)
│   ├── models/   VideoSource, Speaker, Utterance, AudioFile,
│   │             RawTranscript, Transcript, PolishResult, ManifestEntry
│   └── ports/    AudioExtractor, Transcriber, Polisher, TranscriptRepository
│
├── application/  ← pipeline orchestration, speaker mapping (depends on domain only)
│
├── infra/        ← all provider adapters (depends on domain only)
│   └── adapters/
│       ├── ytdlp_audio_extractor      (AudioExtractor)
│       ├── assemblyai_transcriber     (Transcriber)
│       ├── deepgram_transcriber       (Transcriber)
│       ├── claude_polisher            (Polisher)
│       ├── openai_polisher            (Polisher)
│       ├── gemini_polisher            (Polisher)
│       ├── ollama_polisher            (Polisher)
│       ├── file_transcript_repository (TranscriptRepository → Markdown)
│       ├── json_transcript_repository (TranscriptRepository → JSON)
│       ├── srt_transcript_repository  (TranscriptRepository → SRT)
│       └── manifest_repository        (manifest.json + index.html)
│
└── cli/          ← clap CLI, composition root (depends on all three)
```

**Adding a new provider** requires only:
1. Create a new adapter in `crates/infra/src/adapters/` implementing the relevant trait
2. Add it to `mod.rs`
3. Add a variant to the CLI enum and dispatch macro in `main.rs`
4. Add the API key to `Settings` if needed

No changes to domain, application, or existing adapters.

## Development

```bash
# Run all unit tests
cargo test --workspace

# Run integration tests (requires API keys and network access)
cargo test -p voxtract-infra --test integration_ytdlp -- --ignored
cargo test -p voxtract-infra --test integration_assemblyai -- --ignored
cargo test -p voxtract-infra --test integration_claude -- --ignored

# Lint and format
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

## License

MIT
