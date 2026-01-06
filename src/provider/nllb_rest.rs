use crate::error::TranslateError;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone)]
pub struct NllbRestProvider {
    client: Client,
    base_url: String,
    timeout: Duration,
}

impl NllbRestProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[derive(Serialize)]
struct NllbReq<'a> {
    src_lang: &'a str,
    tgt_lang: &'a str,
    text: &'a [String],
}

#[derive(Deserialize)]
struct NllbResp {
    translations: Vec<String>,
}

#[async_trait]
impl crate::provider::TranslationProvider for NllbRestProvider {
    async fn translate_batch(
        &self,
        src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> Result<Vec<String>, TranslateError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("{}/translate", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(url)
            .timeout(self.timeout)
            .json(&NllbReq { src_lang, tgt_lang, text: texts })
            .send()
            .await
            .map_err(TranslateError::Http)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(TranslateError::Provider(format!(
                "NLLB REST error: status={} body={}",
                status, body
            )));
        }

        let parsed: NllbResp = resp.json().await.map_err(TranslateError::Http)?;
        if parsed.translations.len() != texts.len() {
            return Err(TranslateError::Provider(format!(
                "NLLB returned {} translations for {} inputs",
                parsed.translations.len(),
                texts.len()
            )));
        }

        Ok(parsed.translations)
    }

    fn name(&self) -> &'static str {
        "nllb-rest"
    }
}
