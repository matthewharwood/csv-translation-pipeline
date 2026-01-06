//! Translation provider trait and implementations.
//!
//! This module defines the [`TranslationProvider`] trait that all translation
//! backends must implement. The trait uses Rust's native async fn in traits
//! (stabilized in Rust 1.75) for a clean, zero-cost abstraction.
//!
//! # Available Providers
//!
//! | Provider | Description | Requirements |
//! |----------|-------------|--------------|
//! | [`mock::MockProvider`] | Fake translations for testing | None |
//! | [`nllb_rest::NllbRestProvider`] | NLLB REST API with retry logic | NLLB server |
//! | [`marian::MarianProvider`] | Pure Rust translation | libtorch (`marian` feature) |
//!
//! # Implementing a New Provider
//!
//! To add a new translation backend:
//!
//! 1. Create a struct with your provider's configuration
//! 2. Implement [`TranslationProvider`] following the contract below
//! 3. Export from this module
//!
//! ```rust
//! use translator::{TranslationProvider, TranslateError};
//!
//! pub struct MyProvider {
//!     api_key: String,
//! }
//!
//! impl TranslationProvider for MyProvider {
//!     async fn translate_batch(
//!         &self,
//!         src_lang: &str,
//!         tgt_lang: &str,
//!         texts: &[String],
//!     ) -> Result<Vec<String>, TranslateError> {
//!         // CRITICAL: Return exactly texts.len() translations
//!         if texts.is_empty() {
//!             return Ok(Vec::new());
//!         }
//!
//!         // Your translation logic here...
//!         Ok(texts.iter()
//!             .map(|t| format!("[{tgt_lang}] {t}"))
//!             .collect())
//!     }
//!
//!     fn name(&self) -> &'static str {
//!         "my-provider"
//!     }
//! }
//! ```

use crate::error::TranslateError;
use std::future::Future;

/// A pluggable translation backend.
///
/// This trait defines the contract that all translation providers must follow.
/// It is designed to be simple to implement while supporting diverse backends
/// (REST APIs, local models, mock implementations, etc.).
///
/// # Contract
///
/// Implementations **MUST**:
///
/// 1. **Preserve Input Order**: Return translations in the same order as input texts.
///    The translation at index `i` corresponds to the input at index `i`.
///
/// 2. **Match Input Length**: Return exactly `texts.len()` translations.
///    Violating this causes [`TranslateError::TranslationCountMismatch`].
///
/// 3. **Handle Empty Input**: Return an empty `Vec` when `texts` is empty.
///    Do not make network calls or allocate for empty input.
///
/// 4. **Be Thread-Safe**: Support concurrent calls from multiple async tasks.
///    The `Send + Sync` bounds enforce this at compile time.
///
/// # Language Codes
///
/// This crate uses NLLB-style language codes (BCP-47 with script tags):
///
/// - English: `eng_Latn`
/// - French: `fra_Latn`
/// - Spanish: `spa_Latn`
/// - German: `deu_Latn`
/// - Chinese (Simplified): `zho_Hans`
///
/// Providers may accept other formats but should document their requirements.
///
/// # Error Handling
///
/// Providers should return appropriate [`TranslateError`] variants:
///
/// - [`TranslateError::Http`] for network failures
/// - [`TranslateError::UnsupportedLanguage`] for unknown language codes
/// - [`TranslateError::Provider`] for provider-specific errors
/// - [`TranslateError::RetriesExhausted`] after retry logic is exhausted
pub trait TranslationProvider: Send + Sync + 'static {
    /// Translate a batch of texts from source to target language.
    ///
    /// # Arguments
    ///
    /// * `src_lang` - Source language code (e.g., "eng_Latn")
    /// * `tgt_lang` - Target language code (e.g., "fra_Latn")
    /// * `texts` - Texts to translate (may be empty)
    ///
    /// # Returns
    ///
    /// A vector of translated strings with the same length and order as `texts`.
    ///
    /// # Errors
    ///
    /// - [`TranslateError::Http`] - Network request failed
    /// - [`TranslateError::UnsupportedLanguage`] - Language code not supported
    /// - [`TranslateError::Provider`] - Provider-specific error
    /// - [`TranslateError::RetriesExhausted`] - All retry attempts failed
    ///
    /// # Example
    ///
    /// ```rust
    /// use translator::{TranslationProvider, provider::mock::MockProvider};
    ///
    /// # tokio_test::block_on(async {
    /// let provider = MockProvider;
    /// let texts = vec!["Hello".to_string(), "World".to_string()];
    ///
    /// let translations = provider
    ///     .translate_batch("eng_Latn", "fra_Latn", &texts)
    ///     .await
    ///     .expect("translation should succeed");
    ///
    /// assert_eq!(translations.len(), texts.len());
    /// # });
    /// ```
    fn translate_batch(
        &self,
        src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> impl Future<Output = Result<Vec<String>, TranslateError>> + Send;

    /// Human-readable provider name for logging and diagnostics.
    ///
    /// This should be a short, lowercase identifier like "nllb-rest" or "mock".
    fn name(&self) -> &'static str;
}

pub mod mock;
pub mod nllb_rest;

#[cfg(feature = "marian")]
pub mod marian;
