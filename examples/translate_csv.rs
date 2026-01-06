use tokio::fs;

use translator::{
    csv_pipeline::{translate_csv, CsvTranslateConfig},
    provider::nllb_rest::NllbRestProvider,
    Translator,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a sample CSV if it doesn't exist
    let sample = "source\nBook a ride\nYour driver has arrived\n";

    if fs::metadata("input.csv").await.is_err() {
        fs::write("input.csv", sample).await?;
        println!("Created sample input.csv");
    }

    // Point this at your local FastAPI server
    let nllb = NllbRestProvider::new("http://localhost:8080");
    let translator = Translator::new(nllb);

    let cfg = CsvTranslateConfig {
        source_col: "source".to_string(),
        src_lang: "eng_Latn".to_string(),
        targets: vec![
            ("fr".to_string(), "fra_Latn".to_string()),
            ("es".to_string(), "spa_Latn".to_string()),
            ("de".to_string(), "deu_Latn".to_string()),
        ],
        max_concurrency: 8,
    };

    translate_csv(&translator, "input.csv", "output.csv", cfg).await?;
    println!("Wrote output.csv");

    Ok(())
}
