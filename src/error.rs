use thiserror::Error;

use crate::models::{parse_route_suggestions, AmountTooSmallDetails, ProviderError, RouteSuggestion};

/// Every error the client can return.
#[derive(Debug, Error)]
pub enum Error {
    /// The API answered with a non-2xx status. `code` is a stable error code
    /// (`QUOTE_EXPIRED`, `NO_ROUTE`, ...) documented at `https://docs.layerhop.com/errors/{code}`.
    /// `details` is the raw `error.details` object, present on some errors only; read it with
    /// [`Error::suggestions`], [`Error::amount_too_small`] and [`Error::provider_errors`].
    #[error("{code}: {message} (HTTP {status})")]
    Api {
        code: String,
        message: String,
        status: u16,
        request_id: Option<String>,
        details: Option<serde_json::Value>,
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
    /// The call does not fit this quote, e.g. [`crate::LayerHop::build_tx`] on a gasless quote
    /// (use [`crate::LayerHop::build_gasless`]) or the other way round.
    #[error("{0}")]
    Usage(String),
}

impl Error {
    /// The stable API error code, when this is an [`Error::Api`].
    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Api { code, .. } => Some(code),
            _ => None,
        }
    }

    /// The raw `error.details` object of an [`Error::Api`], when the API sent one.
    pub fn details(&self) -> Option<&serde_json::Value> {
        match self {
            Error::Api { details, .. } => details.as_ref(),
            _ => None,
        }
    }

    /// Alternatives that quoted, best first (D-053): `details.suggestions` of a live `NO_ROUTE`
    /// (or `UNSUPPORTED_CHAIN` into a route-restricted chain such as Blast). Validated leniently:
    /// malformed entries are dropped, at most 4. Empty when there are none.
    pub fn suggestions(&self) -> Vec<RouteSuggestion> {
        self.details().map(parse_route_suggestions).unwrap_or_default()
    }

    /// The route's minimum on an `AMOUNT_TOO_SMALL` quote error (D-036), when the API could tell it.
    /// `None` for other errors or without details.
    pub fn amount_too_small(&self) -> Option<AmountTooSmallDetails> {
        if self.code() != Some("AMOUNT_TOO_SMALL") {
            return None;
        }
        let d = self.details()?;
        Some(AmountTooSmallDetails {
            min_amount: d
                .get("minAmount")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                .map(str::to_string),
            min_amount_usd: d.get("minAmountUsd").and_then(|v| v.as_f64()),
            min_amount_approx: d.get("minAmountApprox").and_then(|v| v.as_bool()),
        })
    }

    /// Why each provider returned no quote (`details.errors` of `NO_ROUTE` and of
    /// `AMOUNT_TOO_SMALL` / `AMOUNT_TOO_LARGE` decided from the providers' answers). Malformed
    /// entries are skipped.
    pub fn provider_errors(&self) -> Vec<ProviderError> {
        self.details()
            .and_then(|d| d.get("errors"))
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|e| serde_json::from_value(e.clone()).ok()).collect())
            .unwrap_or_default()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
