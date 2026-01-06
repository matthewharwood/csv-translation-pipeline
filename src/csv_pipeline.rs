use crate::{error::TranslateError, Translator};
use csv_async::{AsyncReaderBuilder, AsyncWriterBuilder, StringRecord};
use futures::{stream::FuturesUnordered, StreamExt};
use std::{path::Path, sync::Arc};
use tokio::{fs::File, sync::Semaphore};

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

    // Read input headers (stable order)
    let input_headers = rdr.headers().await.map_err(TranslateError::Csv)?.clone();

    // Find source column index
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

    // Build output headers = input headers + target headers
    let mut output_headers = input_headers.clone();
    for (out_col, _) in &cfg.targets {
        output_headers.push_field(out_col);
    }

    // Write output headers
    wtr.write_record(&output_headers)
        .await
        .map_err(TranslateError::Csv)?;

    let sem = Arc::new(Semaphore::new(cfg.max_concurrency));
    let mut record = StringRecord::new();

    // Stream records
    while rdr
        .read_record(&mut record)
        .await
        .map_err(TranslateError::Csv)?
    {
        // Grab source text from the row
        let source_text = record.get(source_idx).unwrap_or("").to_string();

        // Translate into each target language concurrently (bounded)
        let mut futs = FuturesUnordered::new();
        for (out_col, tgt_lang) in cfg.targets.iter().cloned() {
            let sem = sem.clone();
            let src_lang = cfg.src_lang.clone();
            let text = source_text.clone();
            let tr = translator;

            futs.push(async move {
                let _permit = sem.acquire_owned().await.expect("semaphore closed");
                let translated = tr
                    .translate_batch(&src_lang, &tgt_lang, &[text])
                    .await?
                    .into_iter()
                    .next()
                    .unwrap_or_default();

                Ok::<(String, String), TranslateError>((out_col, translated))
            });
        }

        // Collect translations in the same order as cfg.targets
        let mut translated_map = std::collections::HashMap::new();
        while let Some(res) = futs.next().await {
            let (out_col, translated) = res?;
            translated_map.insert(out_col, translated);
        }

        // Build output row in correct header order:
        // start with original row fields
        let mut out_row: Vec<String> = record.iter().map(|s| s.to_string()).collect();

        // then append target columns in cfg.targets order
        for (out_col, _) in &cfg.targets {
            out_row.push(
                translated_map
                    .get(out_col)
                    .cloned()
                    .unwrap_or_else(String::new),
            );
        }

        wtr.write_record(&out_row)
            .await
            .map_err(TranslateError::Csv)?;
    }

    wtr.flush().await.map_err(TranslateError::Io)?;
    Ok(())
}
