use crate::{error::TranslateError, Translator};
use csv_async::{AsyncReaderBuilder, AsyncWriterBuilder, StringRecord};
use std::path::Path;
use tokio::fs::File;

const BATCH_SIZE: usize = 128;

pub struct CsvTranslateConfig {
    pub source_col: String,              // e.g. "source"
    pub src_lang: String,                // e.g. "eng_Latn"
    pub targets: Vec<(String, String)>,  // (output_col_header, target_lang_code)
    pub max_concurrency: usize,          // e.g. 8
}

pub async fn translate_csv<P: crate::provider::TranslationProvider>(
    translator: &Translator<P>,
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    cfg: CsvTranslateConfig,
) -> Result<(), TranslateError> {
    let input = File::open(input_path).await.map_err(TranslateError::Io)?;
    let output = File::create(output_path).await.map_err(TranslateError::Io)?;

    let mut rdr = AsyncReaderBuilder::new()
        .has_headers(true)
        .create_reader(input);

    let mut wtr = AsyncWriterBuilder::new()
        .has_headers(true)
        .create_writer(output);

    let input_headers = rdr.headers().await.map_err(TranslateError::Csv)?.clone();

    let source_idx = input_headers
        .iter()
        .position(|h| h == cfg.source_col)
        .ok_or_else(|| {
            TranslateError::Provider(format!(
                "source column '{}' not found in input headers {:?}",
                cfg.source_col,
                input_headers.iter().collect::<Vec<_>>()
            ))
        })?;

    let mut output_headers = input_headers.clone();
    for (out_col, _) in &cfg.targets {
        output_headers.push_field(out_col);
    }

    wtr.write_record(&output_headers)
        .await
        .map_err(TranslateError::Csv)?;

    // -----------------------------
    // 1) Read all rows into memory
    // -----------------------------
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut record = StringRecord::new();

    while rdr
        .read_record(&mut record)
        .await
        .map_err(TranslateError::Csv)?
    {
        let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
        let source_text = row.get(source_idx).cloned().unwrap_or_default();

        rows.push(row);
        sources.push(source_text);
    }

    // ---------------------------------------------------
    // 2) Translate in batches, per target language
    // ---------------------------------------------------
    let mut translated_columns: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for (out_col, tgt_lang) in &cfg.targets {
        let mut translated_all = Vec::with_capacity(sources.len());

        for chunk in sources.chunks(BATCH_SIZE) {
            let batch: Vec<String> = chunk.to_vec();

            let translated = translator
                .translate_batch(&cfg.src_lang, tgt_lang, &batch)
                .await?;

            if translated.len() != batch.len() {
                return Err(TranslateError::Provider(format!(
                    "batch size mismatch: sent {}, got {}",
                    batch.len(),
                    translated.len()
                )));
            }

            translated_all.extend(translated);
        }

        translated_columns.insert(out_col.clone(), translated_all);
    }

    // ---------------------------------------------------
    // 3) Write output rows (preserving order)
    // ---------------------------------------------------
    for (row_idx, mut row) in rows.into_iter().enumerate() {
        for (out_col, _) in &cfg.targets {
            let col_vals = translated_columns
                .get(out_col)
                .expect("missing translated column");
            row.push(col_vals[row_idx].clone());
        }

        wtr.write_record(&row)
            .await
            .map_err(TranslateError::Csv)?;
    }

    wtr.flush().await.map_err(TranslateError::Io)?;
    Ok(())
}

