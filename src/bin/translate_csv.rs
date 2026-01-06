//! CSV Translation CLI
//!
//! A command-line tool for translating CSV files into multiple languages.
//!
//! # Usage
//!
//! ```bash
//! # Using mock provider (for testing)
//! translate_csv --input data.csv --output translated.csv
//!
//! # Using NLLB REST provider
//! translate_csv --provider nllb --nllb-url http://localhost:8080 \
//!     --input data.csv --output translated.csv \
//!     --target fr=fra_Latn --target es=spa_Latn
//!
//! # Custom source column and language
//! translate_csv --source-col text --src-lang deu_Latn \
//!     --target en=eng_Latn --input german.csv --output english.csv
//! ```
//!
//! # Providers
//!
//! - `mock`: Fake translations for testing (default)
//! - `nllb`: NLLB REST API server (requires `--nllb-url`)
//!
//! # Exit Codes
//!
//! - 0: Success
//! - 1: Error (see stderr for details)

use clap::{Parser, ValueEnum};
use std::process::ExitCode;
use std::time::Duration;
use translator::{
    provider::mock::MockProvider,
    provider::nllb_rest::NllbRestProvider,
    translate_csv, CsvTranslateConfig,
};

/// Available translation providers.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProviderKind {
    /// Fake translations for testing and development
    Mock,
    /// NLLB REST API server (requires --nllb-url)
    Nllb,
}

/// Translate a CSV column into multiple languages.
///
/// Reads an input CSV, translates the specified source column into each
/// target language, and writes the output CSV with new translation columns.
#[derive(Parser, Debug)]
#[command(name = "translate_csv")]
#[command(version)]
#[command(about = "Translate a CSV column into multiple languages")]
#[command(long_about = None)]
struct Args {
    /// Translation provider to use
    #[arg(long, value_enum, default_value_t = ProviderKind::Mock)]
    provider: ProviderKind,

    /// Input CSV file path
    #[arg(long, default_value = "input.csv")]
    input: String,

    /// Output CSV file path
    #[arg(long, default_value = "output.csv")]
    output: String,

    /// Name of the column containing source text
    #[arg(long, default_value = "source")]
    source_col: String,

    /// Source language code (e.g., eng_Latn for English)
    #[arg(long, default_value = "eng_Latn")]
    src_lang: String,

    /// Target language mappings: COLUMN=LANG_CODE
    ///
    /// Examples:
    ///   --target fr=fra_Latn
    ///   --target es=spa_Latn
    ///   --target de=deu_Latn
    #[arg(long = "target", value_parser = parse_target)]
    targets: Vec<(String, String)>,

    /// NLLB server URL (required when provider=nllb)
    #[arg(long)]
    nllb_url: Option<String>,

    /// Request timeout in milliseconds
    #[arg(long, default_value_t = 60_000)]
    timeout_ms: u64,
}

/// Parse "column=lang_code" format.
fn parse_target(s: &str) -> Result<(String, String), String> {
    let (col, lang) = s.split_once('=').ok_or_else(|| {
        format!(
            "Invalid target format: '{}'. Expected COLUMN=LANG_CODE (e.g., fr=fra_Latn)",
            s
        )
    })?;

    let col = col.trim();
    let lang = lang.trim();

    if col.is_empty() {
        return Err("Column name cannot be empty".to_string());
    }
    if lang.is_empty() {
        return Err("Language code cannot be empty".to_string());
    }

    Ok((col.to_owned(), lang.to_owned()))
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    if let Err(e) = run(args).await {
        eprintln!("Error: {e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Default targets if none specified
    let targets = if args.targets.is_empty() {
        vec![
            ("fr".into(), "fra_Latn".into()),
            ("es".into(), "spa_Latn".into()),
        ]
    } else {
        args.targets
    };

    let config = CsvTranslateConfig {
        source_col: args.source_col,
        src_lang: args.src_lang,
        targets,
    };

    match args.provider {
        ProviderKind::Mock => {
            println!("Using mock provider (for testing only)");
            translate_csv(&MockProvider, &args.input, &args.output, config).await?;
        }
        ProviderKind::Nllb => {
            let url = args.nllb_url.ok_or(
                "NLLB provider requires --nllb-url. Example: --nllb-url http://localhost:8080"
            )?;

            println!("Using NLLB provider at {}", url);

            let provider = NllbRestProvider::new(url)
                .with_timeout(Duration::from_millis(args.timeout_ms));

            translate_csv(&provider, &args.input, &args.output, config).await?;
        }
    }

    println!("Translation complete: {} -> {}", args.input, args.output);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target_accepts_valid_format() {
        let result = parse_target("fr=fra_Latn");
        assert!(result.is_ok());

        let (col, lang) = result.unwrap();
        assert_eq!(col, "fr");
        assert_eq!(lang, "fra_Latn");
    }

    #[test]
    fn parse_target_trims_whitespace() {
        let result = parse_target("  fr  =  fra_Latn  ");
        assert!(result.is_ok());

        let (col, lang) = result.unwrap();
        assert_eq!(col, "fr");
        assert_eq!(lang, "fra_Latn");
    }

    #[test]
    fn parse_target_rejects_missing_equals() {
        let result = parse_target("fr-fra_Latn");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected COLUMN=LANG_CODE"));
    }

    #[test]
    fn parse_target_rejects_empty_column() {
        let result = parse_target("=fra_Latn");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Column name cannot be empty"));
    }

    #[test]
    fn parse_target_rejects_empty_language() {
        let result = parse_target("fr=");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Language code cannot be empty"));
    }
}
