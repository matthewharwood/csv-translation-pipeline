use async_trait::async_trait;

#[async_trait]
pub trait TranslationProvider: Send + Sync + 'static {
    /// Translate a batch of texts from src_lang -> tgt_lang.
    async fn translate_batch(
        &self,
        src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> Result<Vec<String>, crate::error::TranslateError>;

    /// Human readable provider name (helps telemetry/logging).
    fn name(&self) -> &'static str;
}

pub mod nllb_rest;
pub mod mock;
#[cfg(feature = "gemini")]
pub mod gemini;
