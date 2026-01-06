use clap::{Parser, ValueEnum};
use serde::Deserialize;

use translator::{
    csv_pipeline::{translate_csv, CsvTranslateConfig},
    provider::{mock::MockProvider, nllb_rest::NllbRestProvider},
    Translator,
};

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ProviderKind {
    Mock,
    Nllb,
    Gemini, // stub for now
}

#[derive(Debug, Deserialize)]
struct TargetEntry {
    col: String,
    lang: String,
}

#[derive(Parser, Debug)]
#[command(name = "translate_csv")]
#[command(about = "Translate a CSV into multiple language columns", long_about = None)]
struct Args {
    /// Translation provider to use
    #[arg(long, value_enum, default_value_t = ProviderKind::Mock)]
    provider: ProviderKind,

    /// Input CSV path
    #[arg(long, default_value = "input.csv")]
    input: String,

    /// Output CSV path
    #[arg(long, default_value = "output.csv")]
    output: String,

    /// Name of the source column in the CSV
    #[arg(long, default_value = "source")]
    source_col: String,

    /// Source language code (FLORES-200 style, e.g. eng_Latn)
    #[arg(long, default_value = "eng_Latn")]
    src_lang: String,

    /// Optional JSON file containing targets: [{"col":"fr","lang":"fra_Latn"}, ...]
    #[arg(long)]
    targets_file: Option<String>,

    /// Target language mappings in the form: <outputColumn>=<langCode>
    /// Example: --target fr=fra_Latn --target es=spa_Latn
    #[arg(long = "target", value_parser = parse_target, num_args = 0..)]
    targets: Vec<(String, String)>,

    /// Max number of concurrent translation requests
    #[arg(long, default_value_t = 8)]
    max_concurrency: usize,

    /// Base URL for NLLB REST server (required when provider=nllb)
    #[arg(long)]
    nllb_base_url: Option<String>,

    /// Gemini API key (reserved for future; not implemented yet)
    #[arg(long)]
    gemini_api_key: Option<String>,

    /// Request timeout in milliseconds (NLLB)
    #[arg(long, default_value_t = 60000)]
    timeout_ms: u64,

    /// Connect timeout in milliseconds (NLLB)
    #[arg(long, default_value_t = 5000)]
    connect_timeout_ms: u64,

    /// Number of retries for transient failures (NLLB)
    #[arg(long, default_value_t = 3)]
    retries: usize,

    /// Base backoff in milliseconds (NLLB)
    #[arg(long, default_value_t = 250)]
    backoff_ms: u64,

    /// Max backoff cap in milliseconds (NLLB)
    #[arg(long, default_value_t = 4000)]
    backoff_cap_ms: u64,
}

fn parse_target(s: &str) -> Result<(String, String), String> {
    let (left, right) = s
        .split_once('=')
        .ok_or_else(|| "target must be in the form <col>=<langCode>, e.g. fr=fra_Latn".to_string())?;
    let col = left.trim();
    let lang = right.trim();
    if col.is_empty() || lang.is_empty() {
        return Err("target must be in the form <col>=<langCode>, e.g. fr=fra_Latn".to_string());
    }
    Ok((col.to_string(), lang.to_string()))
}

fn load_targets_file(path: &str) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let s = std::fs::read_to_string(path)?;
    let entries: Vec<TargetEntry> = serde_json::from_str(&s)?;
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        let col = e.col.trim();
        let lang = e.lang.trim();
        if col.is_empty() || lang.is_empty() {
            return Err(format!("Invalid entry in targets file: col/lang must be non-empty").into());
        }
        out.push((col.to_string(), lang.to_string()));
    }
    Ok(out)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Targets resolution order:
    // 1) targets loaded from --targets-file (if provided)
    // 2) plus any repeated --target flags
    // 3) if still empty, fall back to a small default set
    let mut targets: Vec<(String, String)> = Vec::new();

    if let Some(path) = &args.targets_file {
        targets = load_targets_file(path)?;
    }

    targets.extend(args.targets);

    if targets.is_empty() {
        targets = vec![
            ("fr".to_string(), "fra_Latn".to_string()),
            ("es".to_string(), "spa_Latn".to_string()),
            ("de".to_string(), "deu_Latn".to_string()),
        ];
    }

    let cfg = CsvTranslateConfig {
        source_col: args.source_col,
        src_lang: args.src_lang,
        targets,
        max_concurrency: args.max_concurrency,
    };

    match args.provider {
        ProviderKind::Mock => {
            let translator = Translator::new(MockProvider::default());
            translate_csv(&translator, args.input, args.output, cfg).await?;
        }
        ProviderKind::Nllb => {
            use std::time::Duration;

            let base = args.nllb_base_url.ok_or(
                "Missing --nllb-base-url (example: --nllb-base-url http://localhost:8080)",
            )?;

            let nllb = NllbRestProvider::new(base)
                .with_timeout(Duration::from_millis(args.timeout_ms))
                .with_connect_timeout(Duration::from_millis(args.connect_timeout_ms))
                .with_max_retries(args.retries)
                .with_backoff(
                    Duration::from_millis(args.backoff_ms),
                    Duration::from_millis(args.backoff_cap_ms),
                );

            let translator = Translator::new(nllb);
            translate_csv(&translator, args.input, args.output, cfg).await?;
        }

        ProviderKind::Gemini => {
            let _ = args.gemini_api_key;
            return Err("Gemini provider not implemented yet (stub).".into());
        }
    }

    println!("Wrote translated CSV successfully.");
    Ok(())
}
