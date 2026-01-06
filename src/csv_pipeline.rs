//! CSV translation pipeline.
//!
//! This module handles the core CSV-to-CSV translation workflow:
//!
//! 1. Read input CSV and locate the source text column
//! 2. Extract all source texts into a batch
//! 3. Translate the batch into each target language
//! 4. Write output CSV with original columns plus new translation columns
//!
//! # Example
//!
//! Input CSV:
//! ```csv
//! id,source,category
//! 1,Hello,greeting
//! 2,Goodbye,farewell
//! ```
//!
//! Output CSV (with French and Spanish targets):
//! ```csv
//! id,source,category,fr,es
//! 1,Hello,greeting,Bonjour,Hola
//! 2,Goodbye,farewell,Au revoir,Adios
//! ```
//!
//! # Performance Characteristics
//!
//! - **Memory**: Loads entire CSV into memory. For very large files (millions of rows),
//!   consider streaming or chunking approaches.
//!
//! - **Batching**: All texts are translated in a single batch per language.
//!   This minimizes HTTP round-trips but may hit provider size limits.
//!
//! - **Parallelism**: Languages are translated sequentially. Future versions
//!   may parallelize across languages.

use crate::error::TranslateError;
use crate::provider::TranslationProvider;
use csv_async::{AsyncReaderBuilder, AsyncWriterBuilder, StringRecord};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs::File;

/// Configuration for CSV translation.
///
/// Specifies which column to translate and into which languages.
///
/// # Example
///
/// ```rust
/// use translator::CsvTranslateConfig;
///
/// let config = CsvTranslateConfig {
///     source_col: "text".into(),
///     src_lang: "eng_Latn".into(),
///     targets: vec![
///         ("fr".into(), "fra_Latn".into()),    // Output column "fr", French
///         ("es".into(), "spa_Latn".into()),    // Output column "es", Spanish
///     ],
/// };
/// ```
#[derive(Debug, Clone)]
pub struct CsvTranslateConfig {
    /// Name of the CSV column containing text to translate.
    ///
    /// Must match a column header in the input CSV exactly.
    pub source_col: String,

    /// Source language code (e.g., "eng_Latn" for English).
    ///
    /// Format depends on the provider. NLLB uses BCP-47 with script tags.
    pub src_lang: String,

    /// Target languages as (output_column_name, language_code) pairs.
    ///
    /// Each pair generates a new column in the output CSV:
    /// - First element: Column header name in output (e.g., "fr")
    /// - Second element: Language code for the provider (e.g., "fra_Latn")
    pub targets: Vec<(String, String)>,
}

/// Translate a CSV file from one language to multiple target languages.
///
/// Reads the input CSV, translates the specified source column into all
/// target languages, and writes the output CSV with new columns for each
/// translation.
///
/// # Arguments
///
/// * `provider` - Translation backend to use
/// * `input_path` - Path to input CSV file
/// * `output_path` - Path for output CSV file (will be overwritten)
/// * `config` - Translation configuration
///
/// # Errors
///
/// Returns an error if:
/// - Input file cannot be read ([`TranslateError::Io`])
/// - CSV is malformed ([`TranslateError::Csv`])
/// - Source column is not found ([`TranslateError::ColumnNotFound`])
/// - Translation fails ([`TranslateError::Provider`] or [`TranslateError::Http`])
///
/// # Example
///
/// ```no_run
/// use translator::{translate_csv, CsvTranslateConfig, provider::mock::MockProvider};
///
/// # async fn example() -> Result<(), translator::TranslateError> {
/// let config = CsvTranslateConfig {
///     source_col: "text".into(),
///     src_lang: "eng_Latn".into(),
///     targets: vec![("fr".into(), "fra_Latn".into())],
/// };
///
/// translate_csv(&MockProvider, "input.csv", "output.csv", config).await?;
/// # Ok(())
/// # }
/// ```
pub async fn translate_csv<P: TranslationProvider>(
    provider: &P,
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    config: CsvTranslateConfig,
) -> Result<(), TranslateError> {
    let input_file = File::open(input_path).await?;
    let output_file = File::create(output_path).await?;

    let mut reader = AsyncReaderBuilder::new()
        .has_headers(true)
        .create_reader(input_file);
    let mut writer = AsyncWriterBuilder::new()
        .has_headers(true)
        .create_writer(output_file);

    // Read headers and locate source column
    let headers = reader.headers().await?.clone();
    let source_idx = find_column_index(&headers, &config.source_col)?;

    // Build output headers: original columns + new translation columns
    let mut output_headers = headers.clone();
    for (col_name, _) in &config.targets {
        output_headers.push_field(col_name);
    }
    writer.write_record(&output_headers).await?;

    // Read all rows and extract source texts
    let (rows, source_texts) = collect_rows_and_sources(&mut reader, source_idx).await?;

    // Translate into each target language
    let translations = translate_to_all_targets(provider, &config, &source_texts).await?;

    // Write output rows with translations appended
    for (row_idx, mut row) in rows.into_iter().enumerate() {
        for (col_name, _) in &config.targets {
            let translated_text = &translations[col_name][row_idx];
            row.push(translated_text.clone());
        }
        writer.write_record(&row).await?;
    }

    writer.flush().await?;
    Ok(())
}

/// Find the index of a column by name, returning a descriptive error if not found.
fn find_column_index(headers: &StringRecord, column_name: &str) -> Result<usize, TranslateError> {
    headers
        .iter()
        .position(|h| h == column_name)
        .ok_or_else(|| {
            let available: Vec<String> = headers.iter().map(String::from).collect();
            TranslateError::column_not_found(column_name, available)
        })
}

/// Read all CSV rows, returning both the raw rows and extracted source texts.
///
/// This loads the entire CSV into memory. For very large files, consider
/// streaming approaches.
async fn collect_rows_and_sources<R: tokio::io::AsyncRead + Unpin + Send>(
    reader: &mut csv_async::AsyncReader<R>,
    source_idx: usize,
) -> Result<(Vec<Vec<String>>, Vec<String>), TranslateError> {
    let mut rows = Vec::new();
    let mut sources = Vec::new();
    let mut record = StringRecord::new();

    while reader.read_record(&mut record).await? {
        let row: Vec<String> = record.iter().map(str::to_owned).collect();
        let source = row.get(source_idx).cloned().unwrap_or_default();
        sources.push(source);
        rows.push(row);
    }

    Ok((rows, sources))
}

/// Translate source texts into all target languages.
///
/// Returns a map from column name to translated texts.
async fn translate_to_all_targets<P: TranslationProvider>(
    provider: &P,
    config: &CsvTranslateConfig,
    sources: &[String],
) -> Result<HashMap<String, Vec<String>>, TranslateError> {
    let mut translations = HashMap::new();

    for (col_name, tgt_lang) in &config.targets {
        let translated = provider
            .translate_batch(&config.src_lang, tgt_lang, sources)
            .await?;

        // Provider contract: output length must equal input length
        if translated.len() != sources.len() {
            return Err(TranslateError::count_mismatch(sources.len(), translated.len()));
        }

        translations.insert(col_name.clone(), translated);
    }

    Ok(translations)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::mock::MockProvider;
    use std::io::Cursor;

    /// Helper: translate CSV from an in-memory string (for testing without files).
    async fn translate_csv_in_memory<P: TranslationProvider>(
        provider: &P,
        input_csv: &str,
        config: CsvTranslateConfig,
    ) -> Result<String, TranslateError> {
        let mut reader = AsyncReaderBuilder::new()
            .has_headers(true)
            .create_reader(input_csv.as_bytes());

        let output_buffer = Vec::new();
        let mut writer = AsyncWriterBuilder::new()
            .has_headers(true)
            .create_writer(output_buffer);

        let headers = reader.headers().await?.clone();
        let source_idx = find_column_index(&headers, &config.source_col)?;

        let mut output_headers = headers.clone();
        for (col_name, _) in &config.targets {
            output_headers.push_field(col_name);
        }
        writer.write_record(&output_headers).await?;

        let (rows, sources) = collect_rows_and_sources(&mut reader, source_idx).await?;
        let translations = translate_to_all_targets(provider, &config, &sources).await?;

        for (i, mut row) in rows.into_iter().enumerate() {
            for (col_name, _) in &config.targets {
                row.push(translations[col_name][i].clone());
            }
            writer.write_record(&row).await?;
        }

        writer.flush().await?;
        let inner = writer.into_inner().await?;
        Ok(String::from_utf8(inner).expect("valid utf8"))
    }

    #[tokio::test]
    async fn translates_csv_with_multiple_target_languages() {
        let input = "source\nHello\nGoodbye\n";

        let config = CsvTranslateConfig {
            source_col: "source".into(),
            src_lang: "eng_Latn".into(),
            targets: vec![
                ("fr".into(), "fra_Latn".into()),
                ("es".into(), "spa_Latn".into()),
            ],
        };

        let output = translate_csv_in_memory(&MockProvider, input, config)
            .await
            .expect("translation should succeed");

        // Parse output to verify structure
        let mut reader = csv::Reader::from_reader(Cursor::new(output.as_bytes()));
        let headers = reader.headers().expect("should have headers");

        assert_eq!(headers.get(0), Some("source"));
        assert_eq!(headers.get(1), Some("fr"));
        assert_eq!(headers.get(2), Some("es"));

        let records: Vec<_> = reader.records().collect();
        assert_eq!(records.len(), 2, "should have 2 data rows");

        let row1 = records[0].as_ref().expect("valid record");
        assert_eq!(row1.get(0), Some("Hello"));
        assert!(row1.get(1).unwrap().contains("fra_Latn"));
        assert!(row1.get(2).unwrap().contains("spa_Latn"));
    }

    #[tokio::test]
    async fn handles_empty_csv_with_headers_only() {
        let input = "source\n";
        let config = CsvTranslateConfig {
            source_col: "source".into(),
            src_lang: "eng_Latn".into(),
            targets: vec![("fr".into(), "fra_Latn".into())],
        };

        let output = translate_csv_in_memory(&MockProvider, input, config)
            .await
            .expect("should handle empty CSV");

        let mut reader = csv::Reader::from_reader(Cursor::new(output.as_bytes()));
        let headers = reader.headers().expect("should have headers");
        assert_eq!(headers.len(), 2); // source, fr

        let records: Vec<_> = reader.records().collect();
        assert!(records.is_empty(), "should have no data rows");
    }

    #[tokio::test]
    async fn returns_actionable_error_for_missing_column() {
        let input = "other_column\nvalue\n";
        let config = CsvTranslateConfig {
            source_col: "source".into(),
            src_lang: "eng_Latn".into(),
            targets: vec![("fr".into(), "fra_Latn".into())],
        };

        let result = translate_csv_in_memory(&MockProvider, input, config).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = err.to_string();

        // Verify error is actionable
        assert!(msg.contains("source"), "should mention missing column");
        assert!(msg.contains("other_column"), "should list available columns");
        assert!(msg.contains("--source-col"), "should suggest how to fix");
    }

    #[tokio::test]
    async fn preserves_original_columns_in_output() {
        let input = "id,source,category\n1,Hello,greeting\n2,Goodbye,farewell\n";

        let config = CsvTranslateConfig {
            source_col: "source".into(),
            src_lang: "eng_Latn".into(),
            targets: vec![("fr".into(), "fra_Latn".into())],
        };

        let output = translate_csv_in_memory(&MockProvider, input, config)
            .await
            .expect("translation should succeed");

        let mut reader = csv::Reader::from_reader(Cursor::new(output.as_bytes()));
        let headers = reader.headers().expect("headers");

        assert_eq!(headers.get(0), Some("id"));
        assert_eq!(headers.get(1), Some("source"));
        assert_eq!(headers.get(2), Some("category"));
        assert_eq!(headers.get(3), Some("fr"));

        let records: Vec<_> = reader.records().collect();
        let row1 = records[0].as_ref().unwrap();
        assert_eq!(row1.get(0), Some("1"));
        assert_eq!(row1.get(2), Some("greeting"));
    }
}
