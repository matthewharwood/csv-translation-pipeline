// provider/gemini.rs
use crate::error::TranslateError;
use async_trait::async_trait;

#[derive(Clone)]
pub struct GeminiProvider {
    // later: api_key, model, client...
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl crate::provider::TranslationProvider for GeminiProvider {
    async fn translate_batch(
        &self,
        _src_lang: &str,
        _tgt_lang: &str,
        _texts: &[String],
    ) -> Result<Vec<String>, TranslateError> {
        Err(TranslateError::Provider(
            "Gemini provider is not wired yet (feature stub)".to_string(),
        ))
    }

    fn name(&self) -> &'static str {
        "gemini"
    }
}
