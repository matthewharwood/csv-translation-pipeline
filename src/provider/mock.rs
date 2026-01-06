use async_trait::async_trait;

use crate::error::TranslateError;
use crate::provider::TranslationProvider;

/// A dev/test provider that fakes translations.
/// Output format: "[mock::<tgt_lang>] <original text>"
#[derive(Clone, Default)]
pub struct MockProvider;

#[async_trait]
impl TranslationProvider for MockProvider {
    async fn translate_batch(
        &self,
        _src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> Result<Vec<String>, TranslateError> {
        Ok(texts
            .iter()
            .map(|t| format!("[mock::{}] {}", tgt_lang, t))
            .collect())
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}
