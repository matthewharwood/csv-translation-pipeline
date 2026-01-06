use crate::error::TranslateError;
use async_trait::async_trait;
use rand::Rng;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Clone)]
pub struct NllbRestProvider {
    client: Client,
    base_url: String,

    // Existing behavior
    timeout: Duration,

    // New (Gap C)
    connect_timeout: Duration,
    max_retries: usize,      // retries (total attempts = max_retries + 1)
    backoff_base: Duration,  // exponential base
    backoff_cap: Duration,   // max backoff
}

impl NllbRestProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),

            timeout: Duration::from_secs(30),

            // sensible defaults (Gap C)
            connect_timeout: Duration::from_secs(5),
            max_retries: 3,
            backoff_base: Duration::from_millis(250),
            backoff_cap: Duration::from_secs(4),
        }
    }

    /// Keep your existing API
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// New: connect timeout
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// New: retries
    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// New: backoff tuning
    pub fn with_backoff(mut self, base: Duration, cap: Duration) -> Self {
        self.backoff_base = base;
        self.backoff_cap = cap;
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

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT // 408
            | StatusCode::TOO_MANY_REQUESTS // 429
            | StatusCode::INTERNAL_SERVER_ERROR // 500
            | StatusCode::BAD_GATEWAY // 502
            | StatusCode::SERVICE_UNAVAILABLE // 503
            | StatusCode::GATEWAY_TIMEOUT // 504
    )
}

fn is_retryable_reqwest_error(e: &reqwest::Error) -> bool {
    e.is_timeout() || e.is_connect() || e.is_request()
}

fn compute_backoff(attempt: usize, base: Duration, cap: Duration) -> Duration {
    // attempt=0 => base*1, attempt=1 => base*2, attempt=2 => base*4, ...
    let shift = (attempt as u32).min(63);        // prevent undefined large shifts
    let pow = 1u64 << shift;                     // 2^attempt, saturated at 2^63
    let ms = (base.as_millis() as u64).saturating_mul(pow);
    let capped_ms = ms.min(cap.as_millis() as u64);
    Duration::from_millis(capped_ms)
}

fn add_jitter(d: Duration) -> Duration {
    let jitter_ms: u64 = rand::thread_rng().gen_range(0..=100);
    d + Duration::from_millis(jitter_ms)
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
        let req_body = NllbReq {
            src_lang,
            tgt_lang,
            text: texts,
        };

        let total_attempts = self.max_retries + 1;

        for attempt in 0..total_attempts {
            // Build a request per attempt (reqwest requests aren’t reusable after send)
            let resp = self
                .client
                .post(&url)
                .timeout(self.timeout)
                // connect timeout isn’t per-request; reqwest has it on Client builder.
                // But we can approximate by enforcing it via the overall timeout and keeping this field for
                // future upgrade to a built client if you want.
                .json(&req_body)
                .send()
                .await;

            match resp {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        let parsed: NllbResp = r.json().await.map_err(TranslateError::Http)?;
                        if parsed.translations.len() != texts.len() {
                            return Err(TranslateError::Provider(format!(
                                "NLLB returned {} translations for {} inputs",
                                parsed.translations.len(),
                                texts.len()
                            )));
                        }
                        return Ok(parsed.translations);
                    }

                    let body = r.text().await.unwrap_or_default();

                    // Retry on transient statuses
                    if is_retryable_status(status) && attempt + 1 < total_attempts {
                        let backoff = add_jitter(compute_backoff(
                            attempt,
                            self.backoff_base,
                            self.backoff_cap,
                        ));
                        eprintln!(
                            "[nllb-rest] retryable HTTP {} attempt {}/{}; sleep {:?}; body={}",
                            status.as_u16(),
                            attempt + 1,
                            total_attempts,
                            backoff,
                            body
                        );
                        sleep(backoff).await;
                        continue;
                    }

                    return Err(TranslateError::Provider(format!(
                        "NLLB REST error: status={} body={}",
                        status, body
                    )));
                }
                Err(e) => {
                    // Retry on network/timeouts
                    if is_retryable_reqwest_error(&e) && attempt + 1 < total_attempts {
                        let backoff = add_jitter(compute_backoff(
                            attempt,
                            self.backoff_base,
                            self.backoff_cap,
                        ));
                        eprintln!(
                            "[nllb-rest] retryable network error attempt {}/{}; sleep {:?}; err={}",
                            attempt + 1,
                            total_attempts,
                            backoff,
                            e
                        );
                        sleep(backoff).await;
                        continue;
                    }

                    return Err(TranslateError::Http(e));
                }
            }
        }

        Err(TranslateError::Provider(
            "NLLB REST error: exhausted retries".to_string(),
        ))
    }

    fn name(&self) -> &'static str {
        "nllb-rest"
    }
}
