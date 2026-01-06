pub mod csv_pipeline;
pub mod error;
pub mod provider;

use provider::TranslationProvider;

pub struct Translator<P: TranslationProvider> {
    provider: P,
}

impl<P: TranslationProvider> Translator<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }
    
    pub async fn translate_batch(
        &self,
        src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> Result<Vec<String>, error::TranslateError> {
        self.provider.translate_batch(src_lang, tgt_lang, texts).await
    }
}
