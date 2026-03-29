use std::path::{Path, PathBuf};

use chrono::Utc;
use yt2pt_domain::errors::Yt2ptError;
use yt2pt_domain::models::manifest::ManifestEntry;

pub struct FileManifestRepository {
    output_dir: PathBuf,
}

impl FileManifestRepository {
    pub fn new(output_dir: &Path) -> Self {
        Self {
            output_dir: output_dir.to_path_buf(),
        }
    }

    pub async fn exists(&self, video_id: &str) -> bool {
        let manifest_path = self.output_dir.join("manifest.json");
        if !manifest_path.exists() {
            return false;
        }

        let content = tokio::fs::read_to_string(&manifest_path)
            .await
            .unwrap_or_else(|_| "[]".to_string());
        let entries: Vec<ManifestEntry> = serde_json::from_str(&content).unwrap_or_default();

        entries.iter().any(|e| e.video_id == video_id)
    }

    pub async fn append(&self, entry: &ManifestEntry) -> Result<(), Yt2ptError> {
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to create output dir: {e}")))?;

        // Read existing manifest or start fresh
        let manifest_path = self.output_dir.join("manifest.json");
        let mut entries: Vec<ManifestEntry> = if manifest_path.exists() {
            let content = tokio::fs::read_to_string(&manifest_path)
                .await
                .unwrap_or_else(|_| "[]".to_string());
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        entries.push(entry.clone());

        // Write manifest.json
        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| Yt2ptError::Extraction(format!("JSON serialization failed: {e}")))?;
        tokio::fs::write(&manifest_path, json)
            .await
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to write manifest: {e}")))?;

        // Generate index.html
        let html = render_html(&entries);
        tokio::fs::write(self.output_dir.join("index.html"), html)
            .await
            .map_err(|e| Yt2ptError::Extraction(format!("Failed to write index.html: {e}")))?;

        Ok(())
    }
}

fn format_duration(seconds: f64) -> String {
    let total = seconds as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}h {m:02}m {s:02}s")
    } else {
        format!("{m}m {s:02}s")
    }
}

fn format_cost(usd: f64) -> String {
    if usd < 0.01 {
        format!("${:.4}", usd)
    } else {
        format!("${:.2}", usd)
    }
}

fn render_html(entries: &[ManifestEntry]) -> String {
    let total_videos = entries.len();
    let total_duration: f64 = entries.iter().map(|e| e.duration_seconds).sum();
    let total_assemblyai: f64 = entries.iter().map(|e| e.assemblyai_cost_usd).sum();
    let total_claude: f64 = entries.iter().map(|e| e.claude_cost_usd).sum();
    let total_cost = total_assemblyai + total_claude;
    let generated_at = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();

    let mut sorted_entries = entries.to_vec();
    // Default sort: newest first
    sorted_entries.sort_by(|a, b| b.date_transcribed.cmp(&a.date_transcribed));

    let rows: String = sorted_entries.iter().map(|e| render_row(e)).collect();

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>YouTube to Polished Transcript — Inventory</title>
<style>
  :root {{
    --bg: #0f1117;
    --surface: #1a1d27;
    --surface2: #242836;
    --border: #2e3345;
    --text: #e4e6ef;
    --text-dim: #8b8fa3;
    --accent: #6c7bff;
    --accent-soft: rgba(108,123,255,0.12);
    --green: #34d399;
    --amber: #fbbf24;
    --red: #f87171;
    --radius: 10px;
  }}
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', sans-serif;
    background: var(--bg);
    color: var(--text);
    line-height: 1.6;
    padding: 2rem;
    max-width: 1200px;
    margin: 0 auto;
  }}
  h1 {{
    font-size: 1.75rem;
    font-weight: 700;
    margin-bottom: 0.25rem;
    background: linear-gradient(135deg, var(--accent), #a78bfa);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }}
  .subtitle {{ color: var(--text-dim); font-size: 0.9rem; margin-bottom: 2rem; }}
  .stats {{
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 1rem;
    margin-bottom: 2rem;
  }}
  .stat {{
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1.25rem;
  }}
  .stat-label {{ font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-dim); }}
  .stat-value {{ font-size: 1.5rem; font-weight: 700; margin-top: 0.25rem; }}
  .stat-value.cost {{ color: var(--green); }}
  table {{
    width: 100%;
    border-collapse: collapse;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }}
  thead th {{
    text-align: left;
    padding: 0.75rem 1rem;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim);
    background: var(--surface2);
    border-bottom: 1px solid var(--border);
    cursor: pointer;
    user-select: none;
  }}
  thead th:hover {{ color: var(--accent); }}
  tbody td {{
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--border);
    font-size: 0.875rem;
    vertical-align: top;
  }}
  tbody tr:last-child td {{ border-bottom: none; }}
  tbody tr:hover {{ background: var(--accent-soft); }}
  a {{ color: var(--accent); text-decoration: none; }}
  a:hover {{ text-decoration: underline; }}
  .speakers {{ color: var(--text-dim); font-size: 0.8rem; }}
  .primary {{ color: var(--accent); font-weight: 500; }}
  .format-badge {{
    display: inline-block;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent);
  }}
  .batch-badge {{
    display: inline-block;
    font-size: 0.65rem;
    font-family: monospace;
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
    background: var(--surface2);
    color: var(--text-dim);
    border: 1px solid var(--border);
  }}
  .meta {{
    margin-top: 2rem;
    text-align: center;
    font-size: 0.75rem;
    color: var(--text-dim);
  }}
  .empty {{
    text-align: center;
    padding: 3rem;
    color: var(--text-dim);
  }}
  @media (max-width: 768px) {{
    body {{ padding: 1rem; }}
    .stats {{ grid-template-columns: repeat(2, 1fr); }}
    table {{ font-size: 0.8rem; }}
    thead th, tbody td {{ padding: 0.5rem; }}
  }}
</style>
</head>
<body>
<h1>YouTube to Polished Transcript</h1>
<p class="subtitle">Transcript Inventory — {generated_at}</p>

<div class="stats">
  <div class="stat">
    <div class="stat-label">Videos</div>
    <div class="stat-value">{total_videos}</div>
  </div>
  <div class="stat">
    <div class="stat-label">Total Duration</div>
    <div class="stat-value">{total_duration_fmt}</div>
  </div>
  <div class="stat">
    <div class="stat-label">AssemblyAI Cost</div>
    <div class="stat-value cost">{total_assemblyai_fmt}</div>
  </div>
  <div class="stat">
    <div class="stat-label">Claude Cost</div>
    <div class="stat-value cost">{total_claude_fmt}</div>
  </div>
  <div class="stat">
    <div class="stat-label">Total Cost</div>
    <div class="stat-value cost">{total_cost_fmt}</div>
  </div>
</div>

{table_html}

<p class="meta">Generated by <a href="https://github.com/CosmicDreamer17/youtube-to-polished-transcript">yt2pt</a> — manifest.json available in the same directory for programmatic access</p>

<script>
document.querySelectorAll('thead th[data-sort]').forEach(th => {{
  th.addEventListener('click', () => {{
    const table = th.closest('table');
    const tbody = table.querySelector('tbody');
    const rows = Array.from(tbody.querySelectorAll('tr'));
    const idx = parseInt(th.dataset.sort);
    const asc = th.dataset.dir !== 'asc';
    th.dataset.dir = asc ? 'asc' : 'desc';
    rows.sort((a, b) => {{
      const av = a.cells[idx]?.textContent.trim() || '';
      const bv = b.cells[idx]?.textContent.trim() || '';
      const an = parseFloat(av.replace(/[^0-9.\\-]/g, ''));
      const bn = parseFloat(bv.replace(/[^0-9.\\-]/g, ''));
      if (!isNaN(an) && !isNaN(bn)) return asc ? an - bn : bn - an;
      return asc ? av.localeCompare(bv) : bv.localeCompare(av);
    }});
    rows.forEach(r => tbody.appendChild(r));
  }});
}});
</script>
</body>
</html>"##,
        generated_at = generated_at,
        total_videos = total_videos,
        total_duration_fmt = format_duration(total_duration),
        total_assemblyai_fmt = format_cost(total_assemblyai),
        total_claude_fmt = format_cost(total_claude),
        total_cost_fmt = format_cost(total_cost),
        table_html = if entries.is_empty() {
            r#"<div class="empty">No transcripts yet. Run <code>yt2pt transcribe</code> to get started.</div>"#.to_string()
        } else {
            format!(
                r#"<table>
<thead>
<tr>
  <th data-sort="0">Title</th>
  <th data-sort="1">Speakers</th>
  <th data-sort="2">Duration</th>
  <th data-sort="3">Date</th>
  <th data-sort="4">Cost</th>
  <th data-sort="5">Format</th>
  <th data-sort="6">Batch</th>
  <th>File</th>
</tr>
</thead>
<tbody>
{rows}
</tbody>
</table>"#,
                rows = rows
            )
        },
    )
}

fn render_row(entry: &ManifestEntry) -> String {
    let speakers_html: Vec<String> = entry
        .speakers
        .iter()
        .map(|s| {
            if Some(&s.name) == entry.primary_speaker.as_ref() {
                format!(r#"<span class="primary">{}</span>"#, html_escape(&s.name))
            } else {
                html_escape(&s.name)
            }
        })
        .collect();

    let total_cost = entry.assemblyai_cost_usd + entry.claude_cost_usd;
    let cost_title = format!(
        "AssemblyAI: {} | Claude: {} ({} in / {} out tokens)",
        format_cost(entry.assemblyai_cost_usd),
        format_cost(entry.claude_cost_usd),
        entry.claude_input_tokens,
        entry.claude_output_tokens,
    );

    let batch_html = if let Some(ref bid) = entry.batch_id {
        format!(r#"<span class="batch-badge" title="{bid}">{}</span>"#, &bid[..8.min(bid.len())])
    } else {
        "-".to_string()
    };

    format!(
        r#"<tr>
  <td><a href="{url}" target="_blank" rel="noopener">{title}</a></td>
  <td class="speakers">{speakers}</td>
  <td>{duration}</td>
  <td>{date}</td>
  <td title="{cost_title}">{cost}</td>
  <td><span class="format-badge">{format}</span></td>
  <td>{batch}</td>
  <td><a href="{file}">{file}</a></td>
</tr>"#,
        url = html_escape(&entry.youtube_url),
        title = html_escape(&entry.video_title),
        speakers = speakers_html.join(", "),
        duration = format_duration(entry.duration_seconds),
        date = html_escape(&entry.date_transcribed),
        cost = format_cost(total_cost),
        format = html_escape(&entry.output_format),
        batch = batch_html,
        file = html_escape(&entry.output_file),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use yt2pt_domain::models::manifest::ManifestSpeaker;

    fn make_entry() -> ManifestEntry {
        ManifestEntry {
            video_title: "Test Video".to_string(),
            youtube_url: "https://www.youtube.com/watch?v=abc123".to_string(),
            video_id: "abc123".to_string(),
            speakers: vec![
                ManifestSpeaker {
                    label: "Speaker A".to_string(),
                    name: "Alice".to_string(),
                },
                ManifestSpeaker {
                    label: "Speaker B".to_string(),
                    name: "Bob".to_string(),
                },
            ],
            primary_speaker: Some("Alice".to_string()),
            duration_seconds: 300.0,
            date_transcribed: "2026-03-29".to_string(),
            assemblyai_cost_usd: 0.024,
            claude_cost_usd: 0.005,
            claude_input_tokens: 1500,
            claude_output_tokens: 1200,
            output_file: "test-video.md".to_string(),
            output_format: "markdown".to_string(),
            batch_id: None,
        }
    }

    #[test]
    fn html_contains_key_elements() {
        let entries = vec![make_entry()];
        let html = render_html(&entries);
        assert!(html.contains("Test Video"));
        assert!(html.contains("Alice"));
        assert!(html.contains("Bob"));
        assert!(html.contains("5m 00s"));
        assert!(html.contains("manifest.json"));
    }

    #[test]
    fn empty_manifest_shows_message() {
        let html = render_html(&[]);
        assert!(html.contains("No transcripts yet"));
    }

    #[tokio::test]
    async fn append_creates_files() {
        let dir = tempfile::tempdir().unwrap();
        let repo = FileManifestRepository::new(dir.path());
        repo.append(&make_entry()).await.unwrap();

        assert!(dir.path().join("manifest.json").exists());
        assert!(dir.path().join("index.html").exists());

        let manifest: Vec<ManifestEntry> = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.len(), 1);
        assert_eq!(manifest[0].video_title, "Test Video");
    }

    #[tokio::test]
    async fn append_accumulates() {
        let dir = tempfile::tempdir().unwrap();
        let repo = FileManifestRepository::new(dir.path());

        let mut entry1 = make_entry();
        entry1.video_title = "Video 1".to_string();
        repo.append(&entry1).await.unwrap();

        let mut entry2 = make_entry();
        entry2.video_title = "Video 2".to_string();
        repo.append(&entry2).await.unwrap();

        let manifest: Vec<ManifestEntry> = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.len(), 2);
    }
}
