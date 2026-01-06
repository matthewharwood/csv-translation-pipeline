//! Integration tests for the MarianMT translation provider.
//!
//! These tests are ignored by default because they:
//! 1. Require libtorch to be installed
//! 2. Download ~300MB of model weights on first run
//! 3. Take several seconds to execute on CPU
//!
//! # Running the Tests
//!
//! ```bash
//! # Install libtorch first (see architecture.md for instructions)
//!
//! # Run a single test
//! cargo test --features marian test_marian_english_to_french -- --ignored
//!
//! # Run all MarianMT tests
//! cargo test --features marian marian -- --ignored
//!
//! # Run with output visible
//! cargo test --features marian marian -- --ignored --nocapture
//! ```
//!
//! # Expected First Run
//!
//! The first run will download model weights from Hugging Face:
//! ```
//! Downloading https://huggingface.co/Helsinki-NLP/opus-mt-en-fr/...
//! ```
//! This is cached in `~/.cache/huggingface/` for subsequent runs.

#[cfg(feature = "marian")]
mod marian_tests {
    use translator::provider::marian::MarianProvider;
    use translator::provider::TranslationProvider;

    /// Test basic English to French translation.
    ///
    /// This is the primary smoke test for the MarianMT provider.
    /// It verifies that:
    /// 1. The provider can be created successfully
    /// 2. Translation produces non-empty output
    /// 3. Output differs from input (i.e., translation occurred)
    #[tokio::test]
    #[ignore = "Requires libtorch and downloads ~300MB model"]
    async fn test_marian_english_to_french() {
        // Create provider with default English->French model
        let provider = MarianProvider::new()
            .expect("Failed to create MarianProvider - is libtorch installed?");

        assert_eq!(provider.name(), "marian");

        // Test simple translation
        let texts = vec!["Hello world".to_string()];
        let translations = provider
            .translate_batch("eng_Latn", "fra_Latn", &texts)
            .await
            .expect("Translation failed");

        // Basic assertions
        assert_eq!(translations.len(), 1, "Expected exactly one translation");
        assert!(!translations[0].is_empty(), "Translation should not be empty");
        assert_ne!(
            translations[0], texts[0],
            "Translation should differ from input"
        );

        // Print result for manual verification
        println!("Input:  {}", texts[0]);
        println!("Output: {}", translations[0]);

        // The French translation should contain "Bonjour" or similar
        let lower = translations[0].to_lowercase();
        assert!(
            lower.contains("bonjour") || lower.contains("monde") || lower.contains("salut"),
            "Expected French text, got: {}",
            translations[0]
        );
    }

    /// Test batch translation with multiple sentences.
    ///
    /// Verifies that the provider correctly handles batch input
    /// and maintains the ordering of translations.
    #[tokio::test]
    #[ignore = "Requires libtorch and downloads ~300MB model"]
    async fn test_marian_batch_translation() {
        let provider = MarianProvider::new().expect("Failed to create MarianProvider");

        let texts = vec![
            "Hello".to_string(),
            "How are you?".to_string(),
            "Goodbye".to_string(),
        ];

        let translations = provider
            .translate_batch("eng_Latn", "fra_Latn", &texts)
            .await
            .expect("Batch translation failed");

        // Verify output length matches input
        assert_eq!(
            translations.len(),
            texts.len(),
            "Output count must match input count"
        );

        // Verify each translation is non-empty and different from input
        for (i, (src, tgt)) in texts.iter().zip(translations.iter()).enumerate() {
            assert!(!tgt.is_empty(), "Translation {} should not be empty", i);
            assert_ne!(src, tgt, "Translation {} should differ from input", i);
            println!("[{}] {} -> {}", i, src, tgt);
        }
    }

    /// Test empty input handling.
    ///
    /// Per the TranslationProvider contract, empty input must return
    /// empty output without calling the model.
    #[tokio::test]
    #[ignore = "Requires libtorch and downloads ~300MB model"]
    async fn test_marian_empty_input() {
        let provider = MarianProvider::new().expect("Failed to create MarianProvider");

        let translations = provider
            .translate_batch("eng_Latn", "fra_Latn", &[])
            .await
            .expect("Empty translation should succeed");

        assert!(translations.is_empty(), "Empty input should produce empty output");
    }

    /// Test that unsupported language codes produce clear errors.
    #[tokio::test]
    #[ignore = "Requires libtorch and downloads ~300MB model"]
    async fn test_marian_unsupported_language() {
        let provider = MarianProvider::new().expect("Failed to create MarianProvider");

        let texts = vec!["Hello".to_string()];

        // Use a made-up language code
        let result = provider
            .translate_batch("eng_Latn", "xxx_Xxxx", &texts)
            .await;

        assert!(result.is_err(), "Unsupported language should produce error");

        let error = result.unwrap_err();
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("Unsupported language code"),
            "Error should mention unsupported language: {}",
            error_msg
        );
    }

    /// Test translation of longer text.
    ///
    /// Verifies the model handles multi-sentence input correctly.
    #[tokio::test]
    #[ignore = "Requires libtorch and downloads ~300MB model"]
    async fn test_marian_longer_text() {
        let provider = MarianProvider::new().expect("Failed to create MarianProvider");

        let texts = vec![
            "The quick brown fox jumps over the lazy dog. This is a longer sentence to test the translation capability.".to_string(),
        ];

        let translations = provider
            .translate_batch("eng_Latn", "fra_Latn", &texts)
            .await
            .expect("Translation of longer text failed");

        assert_eq!(translations.len(), 1);
        assert!(!translations[0].is_empty());

        // Longer input should produce reasonably long output
        assert!(
            translations[0].len() > 20,
            "Translation seems too short: {}",
            translations[0]
        );

        println!("Long text translation:");
        println!("  In:  {}", texts[0]);
        println!("  Out: {}", translations[0]);
    }
}
