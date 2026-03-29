# Voxtract (Rust)

YouTube video → polished, speaker-attributed transcript.

Voxtract automates a 5-stage pipeline:

1. **Audio Extraction** — downloads audio via yt-dlp (16kHz mono WAV)
2. **Transcription + Diarization** — speech-to-text with speaker identification via AssemblyAI
3. **Speaker Mapping** — interactive or CLI-specified mapping of auto-labels to real names
4. **Polishing** — removes filler words, false starts, and verbal repetitions via Claude
5. **Output** — saves as Markdown, JSON, or SRT subtitles

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
- **AssemblyAI API key** — [get one here](https://www.assemblyai.com/) (~$0.29/hour of audio)
- **Anthropic API key** — [get one here](https://console.anthropic.com/) (~$0.10-0.30/hour for polishing)

## Quickstart

```bash
# 1. Clone the repo
git clone https://github.com/CosmicDreamer17/youtube-to-polished-transcript.git
cd youtube-to-polished-transcript

# 2. Set up API keys
cp .env.example .env
# Edit .env and add your ASSEMBLYAI_API_KEY and ANTHROPIC_API_KEY

# 3. Build
cargo build --release

# 4. Run on a YouTube video
./target/release/voxtract transcribe "https://www.youtube.com/watch?v=jNQXAC9IVRw"
```

You can also install it globally:

```bash
cargo install --path crates/cli
```

## Configuration

Environment variables (loaded from `.env` automatically):

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `ASSEMBLYAI_API_KEY` | Yes | — | AssemblyAI API key for transcription |
| `ANTHROPIC_API_KEY` | Yes | — | Anthropic API key for polishing (not needed with `--dry-run`) |
| `VOXTRACT_OUTPUT_DIR` | No | `./output` | Directory for saved transcripts |
| `VOXTRACT_OUTPUT_FORMAT` | No | `markdown` | Default output format (`markdown`, `json`, or `srt`) |

## Usage

### Single video

```bash
# Interactive speaker mapping (prompts you to name each speaker)
voxtract transcribe "https://www.youtube.com/watch?v=..."

# Pre-specified speakers (maps Speaker A → Alice, Speaker B → Bob)
voxtract transcribe "https://www.youtube.com/watch?v=..." -s "Alice,Bob"

# Mark a primary speaker
voxtract transcribe "https://www.youtube.com/watch?v=..." -s "Alice,Bob" -p "Speaker A"

# Hint expected number of speakers (improves diarization accuracy)
voxtract transcribe "https://www.youtube.com/watch?v=..." -n 3

# Dry run — transcribe only, skip polishing (saves Anthropic API cost)
voxtract transcribe "https://www.youtube.com/watch?v=..." --dry-run

# Output as JSON
voxtract transcribe "https://www.youtube.com/watch?v=..." -f json

# Output as SRT subtitles
voxtract transcribe "https://www.youtube.com/watch?v=..." -f srt

# Custom output directory
voxtract transcribe "https://www.youtube.com/watch?v=..." -o ./transcripts
```

### Batch processing

```bash
voxtract batch examples/urls.txt
voxtract batch examples/urls.txt -f json -o ./transcripts
```

Input file: one YouTube URL per line. Lines starting with `#` are comments. Already-processed videos are automatically skipped. In batch mode, speakers are auto-labeled (no interactive prompts).

### Example output

**Dry-run** (transcription only):
```
Transcription complete. Found 1 speaker, 1 utterances.

Raw Transcript (dry-run)
Source: https://www.youtube.com/watch?v=jNQXAC9IVRw
Speakers: Speaker A

[00:00] Speaker A: Alright, so here we are in front of the elephants.
The cool thing about these guys is that they have really, really,
really long trunks, and that's cool.

1 utterances total
```

**Markdown output** (`output/me-at-the-zoo.md`):
```markdown
# Transcript: Me at the zoo

**Source:** https://www.youtube.com/watch?v=jNQXAC9IVRw
**Date transcribed:** 2026-03-29
**Speakers:** Jawed

---

**Jawed:** Alright, so here we are in front of the elephants. The cool
thing about these guys is that they have really long trunks, and that's
cool. And that's pretty much all there is to say.
```

## Architecture

Cargo workspace with 4 crates enforcing hexagonal architecture at compile time:

```
domain       (no internal deps)    ← models, errors, port traits
application  (→ domain)            ← pipeline service, speaker mapping
infra        (→ domain)            ← adapters (yt-dlp, AssemblyAI, Claude, file repos)
cli          (→ all three)         ← clap CLI, composition root
```

Layer boundaries are enforced by Cargo's dependency graph — if `domain` doesn't list `infra` as a dependency, the code physically cannot import it. No runtime linter needed.

## Development

```bash
# Run all unit tests
cargo test --workspace

# Run integration tests (requires API keys and network access)
cargo test -p voxtract-infra --test integration_ytdlp -- --ignored       # yt-dlp only
cargo test -p voxtract-infra --test integration_assemblyai -- --ignored   # AssemblyAI (~$0.01)
cargo test -p voxtract-infra --test integration_claude -- --ignored       # Claude (~$0.001)

# Lint
cargo clippy --workspace -- -D warnings

# Format check
cargo fmt --check
```

## Cost Estimate

| Service | Cost |
|---------|------|
| AssemblyAI transcription | ~$0.29/hour of audio |
| Claude polishing | ~$0.10-0.30/hour of audio |
| **Total** | **~$0.40-0.60 per video hour** |

Use `--dry-run` to skip polishing and avoid the Anthropic API cost.

## License

MIT
