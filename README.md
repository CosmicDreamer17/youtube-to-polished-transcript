# Voxtract (Rust)

YouTube video → polished, speaker-attributed transcript.

Voxtract automates a 5-stage pipeline:

1. **Audio Extraction** — downloads audio via yt-dlp (16kHz mono WAV)
2. **Transcription + Diarization** — speech-to-text with speaker identification via AssemblyAI
3. **Speaker Mapping** — interactive or CLI-specified mapping of auto-labels to real names
4. **Polishing** — removes filler words, false starts, and verbal repetitions via Claude
5. **Output** — saves as Markdown, JSON, or SRT subtitles

## Requirements

- **Rust** 1.85+ (edition 2024)
- **FFmpeg** (for audio processing)
- **yt-dlp** (installed on PATH)
- **AssemblyAI API key** (~$0.29/hour of audio)
- **Anthropic API key** (~$0.10-0.30/hour for polishing)

## Installation

```bash
cargo install --path crates/cli
```

Or build from source:

```bash
cargo build --release
# Binary at target/release/voxtract
```

## Configuration

Copy `.env.example` to `.env` and fill in your API keys:

```bash
cp .env.example .env
```

Required environment variables:
- `ASSEMBLYAI_API_KEY`
- `ANTHROPIC_API_KEY`

Optional:
- `VOXTRACT_OUTPUT_DIR` (default: `./output`)
- `VOXTRACT_OUTPUT_FORMAT` (default: `markdown`)

## Usage

### Single video

```bash
# Interactive speaker mapping
voxtract transcribe "https://www.youtube.com/watch?v=..."

# Pre-specified speakers
voxtract transcribe "https://www.youtube.com/watch?v=..." -s "Alice,Bob" -p "Speaker A"

# Dry run (transcription only, no polishing — saves API cost)
voxtract transcribe "https://www.youtube.com/watch?v=..." --dry-run

# JSON output
voxtract transcribe "https://www.youtube.com/watch?v=..." -f json

# SRT subtitles
voxtract transcribe "https://www.youtube.com/watch?v=..." -f srt
```

### Batch processing

```bash
voxtract batch examples/urls.txt
voxtract batch examples/urls.txt -f json -o ./transcripts
```

Input file: one YouTube URL per line. Lines starting with `#` are comments.

## Architecture

Cargo workspace with 4 crates enforcing hexagonal architecture at compile time:

```
domain       (no internal deps)    ← models, errors, port traits
application  (→ domain)            ← pipeline service, speaker mapping
infra        (→ domain)            ← adapters (yt-dlp, AssemblyAI, Claude, file repos)
cli          (→ all three)         ← clap CLI, composition root
```

Layer boundaries are enforced by Cargo's dependency graph — no import-linter needed.

## Development

```bash
cargo test --workspace          # Run all 32 unit tests
cargo clippy --workspace        # Lint
cargo fmt --check               # Format check
```

## Cost Estimate

- AssemblyAI: ~$0.29/hour of audio
- Claude polishing: ~$0.10-0.30/hour
- **Total: ~$0.40-0.60 per video hour**

## License

MIT
