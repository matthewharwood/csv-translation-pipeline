//! Example: Translate a CSV file using the NLLB provider.
//!
//! This example demonstrates how to use the translator library programmatically.
//!
//! # Running
//!
//! 1. Start your NLLB server on port 8080
//! 2. Run: `cargo run --example translate_csv`
//!
//! # What This Example Does
//!
//! 1. Creates a sample input.csv if it doesn't exist
//! 2. Configures translation from English to French, Spanish, and German
//! 3. Runs the translation pipeline
//! 4. Writes results to output.csv

use translator::{provider::nllb_rest::NllbRestProvider, translate_csv, CsvTranslateConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a sample CSV file for demonstration
    create_sample_input().await?;

    // Configure the NLLB translation provider
    // Point this at your local NLLB REST server
    let provider = NllbRestProvider::new("http://localhost:8080");

    // Define which languages to translate into
    let config = CsvTranslateConfig {
        source_col: "source".into(),
        src_lang: "eng_Latn".into(),
        targets: vec![
            ("fr".into(), "fra_Latn".into()),
            ("es".into(), "spa_Latn".into()),
            ("de".into(), "deu_Latn".into()),
        ],
    };

    // Run the translation pipeline
    translate_csv(&provider, "input.csv", "output.csv", config).await?;

    println!("Success! Check output.csv for translated content.");
    Ok(())
}

/// Create a sample input.csv if it doesn't exist.
async fn create_sample_input() -> Result<(), std::io::Error> {
    use tokio::fs;

    if fs::metadata("input.csv").await.is_ok() {
        println!("Using existing input.csv");
        return Ok(());
    }

    let sample_csv = "source\n\
        Book a ride\n\
        Your driver has arrived\n\
        Payment successful\n";

    fs::write("input.csv", sample_csv).await?;
    println!("Created sample input.csv");
    Ok(())
}
