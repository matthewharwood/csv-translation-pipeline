//! # CSV Translation Pipeline
//!
//! An async Rust library for translating CSV files using pluggable providers.
//!
//! This crate provides three ways to use the translation functionality:
//!
//! 1. **Direct file translation** via [`translate_csv`]
//! 2. **Async job API** via [`JobManager`] for web service integration
//! 3. **CLI tool** (`translate_csv` binary) for command-line usage
//!
//! ## Quick Start
//!
//! ```no_run
//! use translator::{translate_csv, CsvTranslateConfig, provider::mock::MockProvider};
//!
//! # async fn example() -> Result<(), translator::TranslateError> {
//! let config = CsvTranslateConfig {
//!     source_col: "text".into(),
//!     src_lang: "eng_Latn".into(),
//!     targets: vec![
//!         ("fr".into(), "fra_Latn".into()),
//!         ("es".into(), "spa_Latn".into()),
//!     ],
//! };
//!
//! translate_csv(&MockProvider, "input.csv", "output.csv", config).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Choosing a Provider
//!
//! | Provider | When to Use |
//! |----------|-------------|
//! | [`provider::mock::MockProvider`] | Testing, development, CI pipelines |
//! | [`provider::nllb_rest::NllbRestProvider`] | Production with 200+ languages |
//! | [`provider::marian::MarianProvider`] | Offline, pure Rust deployments |
//!
//! ## Job API for Web Services
//!
//! For web frameworks like Axum, use the [`JobManager`] to run translations
//! in the background:
//!
//! ```no_run
//! use translator::{JobManager, JobStatus, TranslationRequest};
//! use translator::provider::mock::MockProvider;
//!
//! # async fn example() {
//! let manager = JobManager::new(MockProvider);
//!
//! // Submit returns immediately with a job ID
//! let job_id = manager.submit(TranslationRequest {
//!     texts: vec!["Hello".into()],
//!     src_lang: "eng_Latn".into(),
//!     targets: vec![("fr".into(), "fra_Latn".into())],
//! });
//!
//! // Poll for completion
//! match manager.get(&job_id) {
//!     Some(JobStatus::Pending) => println!("Still working..."),
//!     Some(JobStatus::Complete { translations }) => {
//!         println!("French: {:?}", translations.get("fr"));
//!     }
//!     Some(JobStatus::Failed { error }) => println!("Error: {error}"),
//!     None => println!("Job not found"),
//! }
//! # }
//! ```
//!
//! ## Error Handling
//!
//! All operations return [`TranslateError`], which provides actionable context:
//!
//! ```rust
//! use translator::TranslateError;
//!
//! fn handle_error(err: TranslateError) {
//!     if err.is_retryable() {
//!         println!("Transient error, consider retrying: {err}");
//!     } else {
//!         println!("Permanent error: {err}");
//!     }
//! }
//! ```
//!
//! ## Adding Custom Providers
//!
//! Implement [`TranslationProvider`] to add support for new translation backends:
//!
//! ```rust
//! use translator::{TranslationProvider, TranslateError};
//!
//! struct MyProvider;
//!
//! impl TranslationProvider for MyProvider {
//!     async fn translate_batch(
//!         &self,
//!         src_lang: &str,
//!         tgt_lang: &str,
//!         texts: &[String],
//!     ) -> Result<Vec<String>, TranslateError> {
//!         // Return exactly texts.len() translations
//!         Ok(texts.iter().map(|t| format!("[{tgt_lang}] {t}")).collect())
//!     }
//!
//!     fn name(&self) -> &'static str { "my-provider" }
//! }
//! ```
//!
//! ## Feature Flags
//!
//! - `marian`: Enable the MarianMT provider for pure Rust translation.
//!   Requires libtorch to be installed. See README for setup instructions.

pub mod csv_pipeline;
pub mod error;
pub mod jobs;
pub mod provider;

// Re-export the primary public API at the crate root for convenience.
// Users can write `translator::translate_csv` instead of `translator::csv_pipeline::translate_csv`.

pub use csv_pipeline::{translate_csv, CsvTranslateConfig};
pub use error::TranslateError;
pub use jobs::{JobId, JobManager, JobStatus, TranslationRequest};
pub use provider::TranslationProvider;
