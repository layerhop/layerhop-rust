use std::time::Duration;

use futures_util::stream::{self, Stream};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;

use crate::error::{Error, Result};
use crate::models::*;

/// Default API origin.
pub const DEFAULT_BASE_URL: &str = "https://api.layerhop.com";

/// Async LayerHop API client. Cheap to clone (shares one connection pool).
///
/// The client never holds private keys: it returns unsigned transactions and your wallet
/// library signs and broadcasts them.
#[derive(Debug, Clone)]
pub struct LayerHop {
    base_url: String,
    api_key: Option<String>,
    channel: String,
    integrator: Option<String>,
    integrator_fee_bps: Option<u32>,
    http: reqwest::Client,
}

/// Builder for [`LayerHop`].
#[derive(Debug, Clone, Default)]
pub struct LayerHopBuilder {
    base_url: Option<String>,
    api_key: Option<String>,
    channel: Option<String>,
    integrator: Option<String>,
    integrator_fee_bps: Option<u32>,
    http: Option<reqwest::Client>,
}

impl LayerHopBuilder {
    /// API origin without `/v1`. Defaults to `https://api.layerhop.com`.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }
    /// Sent as the `x-api-key` header.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }
    /// Fee channel: `api` (default), `web`, `widget` or `bot`.
    pub fn channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }
    /// Your partner id (`[a-z0-9-]{1,32}`), sent as `integrator` on every quote.
    pub fn integrator(mut self, id: impl Into<String>) -> Self {
        self.integrator = Some(id.into());
        self
    }
    /// Partner fee override in bps (honored only with a partner API key).
    pub fn integrator_fee_bps(mut self, bps: u32) -> Self {
        self.integrator_fee_bps = Some(bps);
        self
    }
    /// Bring your own `reqwest::Client` (proxies, timeouts, ...).
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http = Some(client);
        self
    }
    pub fn build(self) -> LayerHop {
        let base = self.base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        LayerHop {
            base_url: base.trim_end_matches('/').to_string(),
            api_key: self.api_key,
            channel: self.channel.unwrap_or_else(|| "api".to_string()),
            integrator: self.integrator,
            integrator_fee_bps: self.integrator_fee_bps,
            http: self.http.unwrap_or_default(),
        }
    }
}

/// Options for [`LayerHop::watch`].
#[derive(Debug, Clone, Copy)]
pub struct WatchOptions {
    /// Delay between polls. Default 3 s.
    pub interval: Duration,
    /// Give up after this long with [`Error::Timeout`]. Default 30 min.
    pub timeout: Duration,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self { interval: Duration::from_secs(3), timeout: Duration::from_secs(30 * 60) }
    }
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: Option<ErrorBody>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    code: Option<String>,
    message: Option<String>,
    request_id: Option<String>,
}

impl Default for LayerHop {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerHop {
    /// Client for `https://api.layerhop.com` with default options.
    pub fn new() -> Self {
        LayerHopBuilder::default().build()
    }

    pub fn builder() -> LayerHopBuilder {
        LayerHopBuilder::default()
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let mut req = self.http.request(method, format!("{}/v1{}", self.base_url, path));
        if !query.is_empty() {
            req = req.query(query);
        }
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let res = req.send().await?;
        let status = res.status();
        let header_request_id = res
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = res.bytes().await?;
        if !status.is_success() {
            let parsed = serde_json::from_slice::<ErrorEnvelope>(&bytes).ok().and_then(|e| e.error);
            let (code, message, request_id) = match parsed {
                Some(e) => (e.code, e.message, e.request_id),
                None => (None, None, None),
            };
            return Err(Error::Api {
                code: code.unwrap_or_else(|| "INTERNAL".to_string()),
                message: message.unwrap_or_else(|| {
                    status.canonical_reason().unwrap_or("request failed").to_string()
                }),
                status: status.as_u16(),
                request_id: request_id.or(header_request_id),
            });
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// `GET /v1/health`.
    pub async fn health(&self) -> Result<HealthResponse> {
        self.call(Method::GET, "/health", &[], None).await
    }

    /// `GET /v1/chains` — every chain LayerHop can route from or to.
    pub async fn chains(&self) -> Result<Vec<Chain>> {
        let r: ChainsResponse = self.call(Method::GET, "/chains", &[], None).await?;
        Ok(r.chains)
    }

    /// `GET /v1/tokens?chain=` — the built-in token list for one chain (id or slug).
    pub async fn tokens(&self, chain: &str) -> Result<Vec<Token>> {
        let r: TokensResponse =
            self.call(Method::GET, "/tokens", &[("chain", chain.to_string())], None).await?;
        Ok(r.tokens)
    }

    /// `GET /v1/quote` — asks every provider; `best` is the highest output for the user.
    pub async fn quote(&self, p: &QuoteParams) -> Result<QuoteResponse> {
        let mut q: Vec<(&str, String)> = vec![
            ("srcChain", p.src_chain.clone()),
            ("srcToken", p.src_token.clone()),
            ("amount", p.amount.clone()),
            ("dstChain", p.dst_chain.clone()),
            ("dstToken", p.dst_token.clone()),
            ("sender", p.sender.clone()),
            ("recipient", p.recipient.clone()),
            ("slippageBps", p.slippage_bps.unwrap_or(50).to_string()),
            ("channel", self.channel.clone()),
        ];
        if let Some(i) = &self.integrator {
            q.push(("integrator", i.clone()));
        }
        if let Some(bps) = self.integrator_fee_bps {
            q.push(("integratorFeeBps", bps.to_string()));
        }
        self.call(Method::GET, "/quote", &q, None).await
    }

    /// `POST /v1/orders` — the transaction to sign for a quote. EVM txs may carry an `approval`.
    pub async fn build_tx(&self, quote_id: &str) -> Result<BuiltTx> {
        let r: BuildOrderResponse =
            self.call(Method::POST, "/orders", &[], Some(json!({ "quoteId": quote_id }))).await?;
        Ok(r.tx)
    }

    /// `POST /v1/orders/submit` — for [`SvmTx`] with `submit_via_api`: forward the wallet-signed
    /// tx (base64). Returns the transaction signature.
    pub async fn submit(&self, quote_id: &str, signed_transaction: &str) -> Result<String> {
        let r: SubmitOrderResponse = self
            .call(
                Method::POST,
                "/orders/submit",
                &[],
                Some(json!({ "quoteId": quote_id, "signedTransaction": signed_transaction })),
            )
            .await?;
        Ok(r.tx_hash)
    }

    /// `POST /v1/orders/{quoteId}/submitted` — tell LayerHop you broadcast the tx so it can track
    /// the order (and fire webhooks). Idempotent.
    pub async fn report_submitted(&self, quote_id: &str, tx_hash: &str) -> Result<ReportSubmittedResponse> {
        let path = format!("/orders/{}/submitted", urlencoding::encode(quote_id));
        self.call(Method::POST, &path, &[], Some(json!({ "txHash": tx_hash }))).await
    }

    /// `POST /v1/simulate` — build the tx for a quote and dry-run it without recording an order.
    pub async fn simulate(&self, quote_id: &str) -> Result<SimulateResponse> {
        self.call(Method::POST, "/simulate", &[], Some(json!({ "quoteId": quote_id }))).await
    }

    /// `GET /v1/orders/{provider}/{id}` — `id` is the provider order id or source tx hash.
    pub async fn status(&self, provider: &str, id: &str) -> Result<OrderStatus> {
        let path = format!("/orders/{}/{}", urlencoding::encode(provider), urlencoding::encode(id));
        self.call(Method::GET, &path, &[], None).await
    }

    /// `GET /v1/orders` — orders by sender or recipient address.
    pub async fn list_orders(&self, p: &ListOrdersParams) -> Result<ListOrdersResponse> {
        let mut q: Vec<(&str, String)> = Vec::new();
        if let Some(v) = &p.address {
            q.push(("address", v.clone()));
        }
        if let Some(v) = &p.status {
            q.push(("status", v.clone()));
        }
        if let Some(v) = &p.cursor {
            q.push(("cursor", v.clone()));
        }
        if let Some(v) = p.limit {
            q.push(("limit", v.to_string()));
        }
        self.call(Method::GET, "/orders", &q, None).await
    }

    /// Checkout ("pay with any token"). `create`, `list` and `expire` need an API key; `get` and
    /// `quote` are public (the payer's side). Pay a quote with [`LayerHop::build_tx`] +
    /// [`LayerHop::report_submitted`] as usual.
    pub fn checkout(&self) -> Checkout<'_> {
        Checkout { client: self }
    }

    /// Poll [`LayerHop::status`] until the order is final (`filled` / `refunded` / `failed`),
    /// yielding each state change. Yields [`Error::Timeout`] (then ends) after `opts.timeout`,
    /// and ends after the first request error.
    pub fn watch(
        &self,
        provider: &str,
        id: &str,
        opts: WatchOptions,
    ) -> impl Stream<Item = Result<OrderStatus>> + Send + 'static {
        struct State {
            client: LayerHop,
            provider: String,
            id: String,
            opts: WatchOptions,
            deadline: tokio::time::Instant,
            last: Option<String>,
            first: bool,
            done: bool,
        }
        let init = State {
            client: self.clone(),
            provider: provider.to_string(),
            id: id.to_string(),
            opts,
            deadline: tokio::time::Instant::now() + opts.timeout,
            last: None,
            first: true,
            done: false,
        };
        stream::unfold(init, |mut st| async move {
            if st.done {
                return None;
            }
            loop {
                if !st.first {
                    tokio::time::sleep(st.opts.interval).await;
                }
                st.first = false;
                if tokio::time::Instant::now() >= st.deadline {
                    st.done = true;
                    return Some((Err(Error::Timeout), st));
                }
                let res = st.client.status(&st.provider, &st.id).await;
                match res {
                    Err(e) => {
                        st.done = true;
                        return Some((Err(e), st));
                    }
                    Ok(s) => {
                        let fin = s.is_final();
                        if st.last.as_deref() != Some(s.state.as_str()) {
                            st.last = Some(s.state.clone());
                            st.done = fin;
                            return Some((Ok(s), st));
                        }
                        if fin {
                            return None;
                        }
                    }
                }
            }
        })
    }
}

/// Checkout session endpoints, from [`LayerHop::checkout`].
#[derive(Debug, Clone, Copy)]
pub struct Checkout<'a> {
    client: &'a LayerHop,
}

impl Checkout<'_> {
    /// `POST /v1/checkout/sessions` — create a session; send the payer to its `url`.
    pub async fn create(&self, p: &CreateCheckoutParams) -> Result<CheckoutSession> {
        self.client.call(Method::POST, "/checkout/sessions", &[], Some(serde_json::to_value(p)?)).await
    }

    /// `GET /v1/checkout/sessions/{id}` — public; `reference` and `metadata` need the owner's key.
    pub async fn get(&self, id: &str) -> Result<CheckoutSession> {
        let path = format!("/checkout/sessions/{}", urlencoding::encode(id));
        self.client.call(Method::GET, &path, &[], None).await
    }

    /// `GET /v1/checkout/sessions` — your sessions of the key's mode (live or test), newest first.
    pub async fn list(&self, p: &ListCheckoutSessionsParams) -> Result<ListCheckoutSessionsResponse> {
        let mut q: Vec<(&str, String)> = Vec::new();
        if let Some(v) = &p.status {
            q.push(("status", v.clone()));
        }
        if let Some(v) = &p.cursor {
            q.push(("cursor", v.clone()));
        }
        if let Some(v) = p.limit {
            q.push(("limit", v.to_string()));
        }
        self.client.call(Method::GET, "/checkout/sessions", &q, None).await
    }

    /// `POST /v1/checkout/sessions/{id}/expire` — cancel an `open` session (`CONFLICT` otherwise).
    pub async fn expire(&self, id: &str) -> Result<CheckoutSession> {
        let path = format!("/checkout/sessions/{}/expire", urlencoding::encode(id));
        self.client.call(Method::POST, &path, &[], Some(json!({}))).await
    }

    /// `POST /v1/checkout/sessions/{id}/quote` — public: price paying the session with
    /// `src_token` on `src_chain` from `sender`.
    pub async fn quote(&self, id: &str, p: &CheckoutQuoteParams) -> Result<CheckoutQuoteResponse> {
        let path = format!("/checkout/sessions/{}/quote", urlencoding::encode(id));
        self.client.call(Method::POST, &path, &[], Some(serde_json::to_value(p)?)).await
    }
}
