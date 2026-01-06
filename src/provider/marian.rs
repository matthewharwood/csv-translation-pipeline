//! MarianMT translation provider using rust-bert.
//!
//! This provider uses the MarianMT architecture for pure Rust translation
//! without Python dependencies at runtime. It leverages pre-trained
//! Opus-MT models from the University of Helsinki.
//!
//! # When to Use This Provider
//!
//! - **Offline deployments**: No network access to translation servers
//! - **Pure Rust builds**: Avoid Python runtime dependencies
//! - **Edge deployments**: Single-binary distribution
//! - **European languages**: Best coverage for EU language pairs
//!
//! For 200+ language support, use [`super::nllb_rest::NllbRestProvider`] instead.
//!
//! # Requirements
//!
//! This provider requires libtorch (PyTorch C++ API) to be installed.
//!
//! ## macOS Installation (Homebrew)
//!
//! ```bash
//! brew install pytorch jq
//! export LIBTORCH=$(brew --cellar pytorch)/$(brew info --json pytorch | jq -r '.[0].installed[0].version')
//! export LD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH
//! ```
//!
//! ## Linux Installation
//!
//! ```bash
//! wget https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.4.0+cpu.zip
//! unzip libtorch-cxx11-abi-shared-with-deps-2.4.0+cpu.zip
//! export LIBTORCH=/path/to/libtorch
//! export LD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH
//! ```
//!
//! ## Automatic Download
//!
//! Set `LIBTORCH_USE_PYTORCH=1` to let the build script download libtorch
//! automatically (this may take several minutes on first build).
//!
//! # First Run
//!
//! The first time you use this provider, it downloads model weights (~300MB)
//! from Hugging Face. These are cached in `~/.cache/huggingface/`.
//!
//! # Example
//!
//! ```rust,ignore
//! use translator::provider::marian::MarianProvider;
//! use translator::TranslationProvider;
//!
//! # async fn example() -> Result<(), translator::TranslateError> {
//! let provider = MarianProvider::new()?;
//!
//! let translations = provider
//!     .translate_batch("eng_Latn", "fra_Latn", &["Hello world".to_string()])
//!     .await?;
//!
//! println!("French: {}", translations[0]);
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Mutex;

use rust_bert::pipelines::translation::{Language, TranslationModel, TranslationModelBuilder};
use tch::Device;

use crate::error::TranslateError;
use crate::provider::TranslationProvider;

/// Supported language codes mapped to rust-bert Language enum.
///
/// This constant defines which language codes are recognized by this provider.
const SUPPORTED_LANGUAGES: &[(&str, Language)] = &[
    // Common European languages
    ("eng_Latn", Language::English),
    ("fra_Latn", Language::French),
    ("deu_Latn", Language::German),
    ("spa_Latn", Language::Spanish),
    ("ita_Latn", Language::Italian),
    ("por_Latn", Language::Portuguese),
    ("nld_Latn", Language::Dutch),
    ("pol_Latn", Language::Polish),
    ("rus_Cyrl", Language::Russian),
    // Nordic languages
    ("swe_Latn", Language::Swedish),
    ("dan_Latn", Language::Danish),
    ("nob_Latn", Language::Norwegian),
    ("fin_Latn", Language::Finnish),
    // Asian languages
    ("zho_Hans", Language::ChineseMandarin),
    ("jpn_Jpan", Language::Japanese),
    ("kor_Hang", Language::Korean),
    ("hin_Deva", Language::Hindi),
    // Other major languages
    ("ara_Arab", Language::Arabic),
    ("heb_Hebr", Language::Hebrew),
    ("tur_Latn", Language::Turkish),
    ("vie_Latn", Language::Vietnamese),
];

/// A pure Rust translation provider using MarianMT models.
///
/// This provider wraps rust-bert's TranslationModel for machine translation
/// without requiring Python or external REST services.
///
/// # Thread Safety
///
/// The underlying TranslationModel is wrapped in a Mutex to ensure safe
/// concurrent access. While this serializes translations, it avoids issues
/// with libtorch's threading model.
///
/// # Supported Language Pairs
///
/// The default model supports English <-> French translation. For other
/// language pairs, use [`MarianProvider::with_languages`] to configure
/// the appropriate model.
///
/// # Performance
///
/// - First creation downloads ~300MB of model weights (cached for future use)
/// - CPU inference: ~50-200ms per batch depending on text length
/// - GPU inference (with CUDA): significantly faster
pub struct MarianProvider {
    model: Mutex<TranslationModel>,
    lang_map: HashMap<&'static str, Language>,
}

impl MarianProvider {
    /// Create a new MarianProvider with the default English-French model.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - libtorch is not properly installed
    /// - Model weights cannot be downloaded
    /// - Model initialization fails
    ///
    /// # Performance Note
    ///
    /// The first call downloads model weights (~300MB) from Hugging Face.
    /// Subsequent calls use cached weights from `~/.cache/huggingface/`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use translator::provider::marian::MarianProvider;
    ///
    /// let provider = MarianProvider::new()?;
    /// ```
    pub fn new() -> Result<Self, TranslateError> {
        Self::with_languages(vec![Language::English], vec![Language::French])
    }

    /// Create a MarianProvider with custom source and target languages.
    ///
    /// The builder automatically selects an appropriate model that supports
    /// the requested language pairs.
    ///
    /// # Arguments
    ///
    /// * `source_langs` - Languages to translate FROM
    /// * `target_langs` - Languages to translate TO
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use rust_bert::pipelines::translation::Language;
    /// use translator::provider::marian::MarianProvider;
    ///
    /// // Create a provider for English -> German translation
    /// let provider = MarianProvider::with_languages(
    ///     vec![Language::English],
    ///     vec![Language::German],
    /// )?;
    /// ```
    pub fn with_languages(
        source_langs: Vec<Language>,
        target_langs: Vec<Language>,
    ) -> Result<Self, TranslateError> {
        // Use CPU by default for broader compatibility.
        // For GPU acceleration, use Device::Cuda(0) if available.
        let device = Device::Cpu;

        let model = TranslationModelBuilder::new()
            .with_device(device)
            .with_source_languages(source_langs)
            .with_target_languages(target_langs)
            .create_model()
            .map_err(|e| {
                TranslateError::Provider(format!(
                    "Failed to create MarianMT model. Is libtorch installed? Error: {e}"
                ))
            })?;

        let lang_map = Self::build_language_map();

        Ok(Self {
            model: Mutex::new(model),
            lang_map,
        })
    }

    /// Build mapping from NLLB language codes to rust-bert Language enum.
    fn build_language_map() -> HashMap<&'static str, Language> {
        SUPPORTED_LANGUAGES.iter().copied().collect()
    }

    /// Convert an NLLB language code to rust-bert Language.
    fn parse_language(&self, code: &str) -> Result<Language, TranslateError> {
        self.lang_map.get(code).copied().ok_or_else(|| {
            let supported: Vec<String> = self.lang_map.keys().map(|s| (*s).to_string()).collect();
            TranslateError::unsupported_language(code, supported)
        })
    }
}

impl TranslationProvider for MarianProvider {
    async fn translate_batch(
        &self,
        _src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> Result<Vec<String>, TranslateError> {
        // Handle empty input per the provider contract
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Parse and validate target language
        let target = self.parse_language(tgt_lang)?;

        // Convert to string slices for the model
        let text_refs: Vec<&str> = texts.iter().map(String::as_str).collect();

        // Lock the model and perform translation
        let model = self.model.lock().map_err(|e| {
            TranslateError::Provider(format!("Failed to acquire model lock: {e}"))
        })?;

        // Perform translation
        // Note: rust-bert's translate() handles batching internally
        let translations = model.translate(&text_refs, None, target).map_err(|e| {
            TranslateError::Provider(format!("Translation failed: {e}"))
        })?;

        // Verify output length matches input (provider contract)
        if translations.len() != texts.len() {
            return Err(TranslateError::count_mismatch(texts.len(), translations.len()));
        }

        Ok(translations)
    }

    fn name(&self) -> &'static str {
        "marian"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_map_contains_common_languages() {
        let map = MarianProvider::build_language_map();

        assert!(map.contains_key("eng_Latn"), "Should support English");
        assert!(map.contains_key("fra_Latn"), "Should support French");
        assert!(map.contains_key("deu_Latn"), "Should support German");
        assert!(map.contains_key("spa_Latn"), "Should support Spanish");
        assert!(map.contains_key("zho_Hans"), "Should support Chinese");
    }

    #[test]
    fn supported_languages_constant_is_valid() {
        // Verify the constant is properly formed
        assert!(
            SUPPORTED_LANGUAGES.len() >= 20,
            "Should have at least 20 supported languages"
        );

        // Verify all entries have valid format
        for (code, _lang) in SUPPORTED_LANGUAGES {
            assert!(
                code.contains('_'),
                "Language code '{}' should have underscore separator",
                code
            );
        }
    }

    /// Integration test for MarianMT translation.
    ///
    /// This test is ignored by default because:
    /// 1. It requires libtorch to be installed
    /// 2. First run downloads ~300MB of model weights
    /// 3. Translation takes several seconds on CPU
    ///
    /// Run manually: `cargo test --features marian test_marian_translation -- --ignored`
    #[tokio::test]
    #[ignore = "Requires libtorch and downloads ~300MB model"]
    async fn test_marian_translation() {
        let provider = MarianProvider::new().expect("Failed to create MarianProvider");

        let texts = vec!["Hello world".to_string(), "How are you?".to_string()];

        let translations = provider
            .translate_batch("eng_Latn", "fra_Latn", &texts)
            .await
            .expect("Translation failed");

        assert_eq!(translations.len(), 2, "Should return same number of translations");

        // Translations should differ from input
        assert_ne!(translations[0], texts[0]);
        assert_ne!(translations[1], texts[1]);

        // Should contain French text
        let first_lower = translations[0].to_lowercase();
        assert!(
            first_lower.contains("bonjour") || first_lower.contains("monde") || first_lower.contains("salut"),
            "Expected French translation, got: {}",
            translations[0]
        );

        println!("Translation results:");
        for (src, tgt) in texts.iter().zip(translations.iter()) {
            println!("  {} -> {}", src, tgt);
        }
    }

    #[tokio::test]
    #[ignore = "Requires libtorch and downloads ~300MB model"]
    async fn test_empty_input_returns_empty_output() {
        let provider = MarianProvider::new().expect("Failed to create MarianProvider");

        let translations = provider
            .translate_batch("eng_Latn", "fra_Latn", &[])
            .await
            .expect("Empty translation should succeed");

        assert!(translations.is_empty());
    }
}
