use thiserror::Error;

#[derive(Debug, Error)]
pub enum TranslateError {
    #[error("io error: {0}")]
    Io(std::io::Error),

    #[error("http error: {0}")]
    Http(reqwest::Error),

    #[error("csv error: {0}")]
    Csv(csv_async::Error),

    #[error("provider error: {0}")]
    Provider(String),
}
