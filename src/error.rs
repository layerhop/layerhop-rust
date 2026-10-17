use thiserror::Error;

/// Every error the client can return.
#[derive(Debug, Error)]
pub enum Error {
    /// The API answered with a non-2xx status. `code` is a stable error code
    /// (`QUOTE_EXPIRED`, `NO_ROUTE`, ...) documented at `https://docs.layerhop.com/errors/{code}`.
    #[error("{code}: {message} (HTTP {status})")]
    Api {
        code: String,
        message: String,
        status: u16,
        request_id: Option<String>,
    },
    /// Network / transport failure.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// The response body was not the JSON we expected.
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),
    /// `watch` gave up before the order reached a final state.
    #[error("timed out waiting for the order to reach a final state")]
    Timeout,
}

impl Error {
    /// The stable API error code, when this is an [`Error::Api`].
    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Api { code, .. } => Some(code),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
