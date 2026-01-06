//! Mock translation provider for testing and development.
//!
//! This provider doesn't actually translate - it wraps each input string
//! with a marker showing the target language. Useful for:
//!
//! - Unit and integration tests
//! - Development without a translation server
//! - Validating CSV pipeline logic

use crate::error::TranslateError;
use crate::provider::TranslationProvider;

/// A fake translation provider that returns marked-up input text.
///
/// # Example
///
/// ```
/// use translator::provider::mock::MockProvider;
/// use translator::provider::TranslationProvider;
///
/// # tokio_test::block_on(async {
/// let provider = MockProvider;
/// let result = provider
///     .translate_batch("eng_Latn", "fra_Latn", &["Hello".into()])
///     .await
///     .unwrap();
///
/// assert_eq!(result, vec!["[mock::fra_Latn] Hello"]);
/// # });
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct MockProvider;

impl TranslationProvider for MockProvider {
    async fn translate_batch(
        &self,
        _src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> Result<Vec<String>, TranslateError> {
        // Simply wrap each text with a marker - no actual translation
        let translated = texts
            .iter()
            .map(|text| format!("[mock::{tgt_lang}] {text}"))
            .collect();

        Ok(translated)
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_provider_preserves_order() {
        let provider = MockProvider;
        let input = vec!["one".into(), "two".into(), "three".into()];

        let result = provider
            .translate_batch("en", "fr", &input)
            .await
            .expect("mock should not fail");

        assert_eq!(result.len(), input.len());
        assert!(result[0].contains("one"));
        assert!(result[1].contains("two"));
        assert!(result[2].contains("three"));
    }

    #[tokio::test]
    async fn mock_provider_handles_empty_input() {
        let provider = MockProvider;
        let result = provider
            .translate_batch("en", "fr", &[])
            .await
            .expect("mock should not fail");

        assert!(result.is_empty());
    }
}
