use std::pin::Pin;
use std::time::Duration;

use futures_util::future::{self, Either};
use futures_util::stream::{self, Stream, StreamExt};
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
        if !res.status().is_success() {
            return Err(api_error(res).await);
        }
        let bytes = res.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Open a Server-Sent Events stream and yield the JSON data of its `event` frames. The request
    /// is sent when the stream is first polled; dropping the stream closes the connection.
    fn sse<T: DeserializeOwned + Send + 'static>(
        &self,
        path: String,
        event: &'static str,
    ) -> impl Stream<Item = Result<T>> + Send + 'static {
        let client = self.clone();
        let open = async move {
            let mut req = client
                .http
                .get(format!("{}/v1{}", client.base_url, path))
                .header("accept", "text/event-stream");
            if let Some(key) = &client.api_key {
                req = req.header("x-api-key", key);
            }
            let res = req.send().await?;
            if !res.status().is_success() {
                return Err(api_error(res).await);
            }
            Ok(res)
        };
        stream::once(open).flat_map(move |res| match res {
            Ok(res) => Either::Left(parse_sse::<T, _, _>(res.bytes_stream(), event)),
            Err(e) => Either::Right(stream::once(future::ready(Err(e)))),
        })
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

    /// `GET /v1/orders/{quoteId}/events` — live order status (Server-Sent Events). Yields the order
    /// now and on every change, and ends when it is final (filled, refunded, failed or expired).
    /// Drop the stream to stop early. A missing order yields one `ORDER_NOT_FOUND` [`Error::Api`].
    pub fn stream_order(&self, quote_id: &str) -> impl Stream<Item = Result<Order>> + Send + 'static {
        self.sse(format!("/orders/{}/events", urlencoding::encode(quote_id)), "order")
    }

    /// Webhook developer tools with an API key (the CLI's `listen` and `trigger`).
    pub fn webhooks(&self) -> Webhooks<'_> {
        Webhooks { client: self }
    }

    /// Invoices: line items billed to a customer by email, paid through checkout. Everything needs
    /// an API key except `public_view` and `pay` (the customer's side).
    pub fn invoices(&self) -> Invoices<'_> {
        Invoices { client: self }
    }

    /// Deposit addresses: your user sends tokens to an address (no wallet connection) and the
    /// provider delivers the destination token. Everything needs an API key. Live mode may be
    /// disabled by the operator (`FORBIDDEN`); test keys always work and can `simulate` a deposit.
    pub fn deposit_addresses(&self) -> DepositAddresses<'_> {
        DepositAddresses { client: self }
    }

    /// `GET /v1/status` — public service status (status.layerhop.com): overall state, per-component
    /// status and 90-day uptime, recent incidents. ([`LayerHop::status`] tracks one order.)
    pub async fn service_status(&self) -> Result<StatusResponse> {
        self.call(Method::GET, "/status", &[], None).await
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

    /// `GET /v1/checkout/sessions/{id}/events` — public: live session status (Server-Sent Events).
    /// Yields the session now and on every change until it is `paid`, `expired` or `needs_review`.
    pub fn stream(&self, id: &str) -> impl Stream<Item = Result<CheckoutSession>> + Send + 'static {
        self.client.sse(format!("/checkout/sessions/{}/events", urlencoding::encode(id)), "session")
    }

    /// Products: a fixed price for payment buttons, `layerhop.com/buy/{id}` links and plugins.
    /// `create`, `list` and `update` need an API key; `get` and `start_session` are public.
    pub fn products(&self) -> CheckoutProducts<'_> {
        CheckoutProducts { client: self.client }
    }

    /// Refunds of paid sessions (API key). Non-custodial: you sign the refund from your wallet.
    pub fn refunds(&self) -> CheckoutRefunds<'_> {
        CheckoutRefunds { client: self.client }
    }
}

/// Checkout refund endpoints, from [`Checkout::refunds`].
#[derive(Debug, Clone, Copy)]
pub struct CheckoutRefunds<'a> {
    client: &'a LayerHop,
}

impl CheckoutRefunds<'_> {
    /// `POST /v1/checkout/sessions/{id}/refunds` — refund (part of) a `paid` session. Returns the
    /// refund and its quote: [`LayerHop::build_tx`] the `quote_id`, sign it from `quote.from`, then
    /// [`LayerHop::report_submitted`]. It completes when the order fills
    /// (`checkout.session.refunded`).
    pub async fn create(&self, session_id: &str, p: &CreateRefundParams) -> Result<CreateRefundResponse> {
        let path = format!("/checkout/sessions/{}/refunds", urlencoding::encode(session_id));
        self.client.call(Method::POST, &path, &[], Some(serde_json::to_value(p)?)).await
    }

    /// `GET /v1/checkout/sessions/{id}/refunds` — the session's refunds, newest first.
    pub async fn list(&self, session_id: &str) -> Result<ListRefundsResponse> {
        let path = format!("/checkout/sessions/{}/refunds", urlencoding::encode(session_id));
        self.client.call(Method::GET, &path, &[], None).await
    }
}

/// Invoice endpoints, from [`LayerHop::invoices`].
#[derive(Debug, Clone, Copy)]
pub struct Invoices<'a> {
    client: &'a LayerHop,
}

impl Invoices<'_> {
    /// `POST /v1/invoices` — create a draft (or, with `send: Some(true)`, an open invoice emailed to
    /// the customer).
    pub async fn create(&self, p: &CreateInvoiceParams) -> Result<Invoice> {
        self.client.call(Method::POST, "/invoices", &[], Some(serde_json::to_value(p)?)).await
    }

    /// `GET /v1/invoices/{id}` — one of your invoices.
    pub async fn get(&self, id: &str) -> Result<Invoice> {
        let path = format!("/invoices/{}", urlencoding::encode(id));
        self.client.call(Method::GET, &path, &[], None).await
    }

    /// `GET /v1/invoices` — your invoices of the key's mode (live or test), newest first.
    pub async fn list(&self, p: &ListInvoicesParams) -> Result<ListInvoicesResponse> {
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
        self.client.call(Method::GET, "/invoices", &q, None).await
    }

    /// `PATCH /v1/invoices/{id}` — edit a `draft` (`CONFLICT` otherwise); only `Some` fields change.
    pub async fn update(&self, id: &str, p: &UpdateInvoiceParams) -> Result<Invoice> {
        let path = format!("/invoices/{}", urlencoding::encode(id));
        self.client.call(Method::PATCH, &path, &[], Some(serde_json::to_value(p)?)).await
    }

    /// `POST /v1/invoices/{id}/send` — finalize a draft (`open`) and email it; on an open invoice,
    /// re-send the email. `CONFLICT` when paid or void.
    pub async fn send(&self, id: &str) -> Result<Invoice> {
        let path = format!("/invoices/{}/send", urlencoding::encode(id));
        self.client.call(Method::POST, &path, &[], Some(json!({}))).await
    }

    /// `POST /v1/invoices/{id}/void` — cancel a draft or open invoice. `CONFLICT` when paid or void.
    pub async fn void(&self, id: &str) -> Result<Invoice> {
        let path = format!("/invoices/{}/void", urlencoding::encode(id));
        self.client.call(Method::POST, &path, &[], Some(json!({}))).await
    }

    /// `GET /v1/invoices/{id}/public` — public: the customer's view of a sent invoice.
    pub async fn public_view(&self, id: &str) -> Result<Invoice> {
        let path = format!("/invoices/{}/public", urlencoding::encode(id));
        self.client.call(Method::GET, &path, &[], None).await
    }

    /// `POST /v1/invoices/{id}/pay` — public: start (or reuse) the checkout session that pays an
    /// open invoice. Send the payer to its `url`.
    pub async fn pay(&self, id: &str) -> Result<PayInvoiceResponse> {
        let path = format!("/invoices/{}/pay", urlencoding::encode(id));
        self.client.call(Method::POST, &path, &[], Some(json!({}))).await
    }
}

/// Deposit address endpoints, from [`LayerHop::deposit_addresses`]. All need an API key.
#[derive(Debug, Clone, Copy)]
pub struct DepositAddresses<'a> {
    client: &'a LayerHop,
}

impl DepositAddresses<'_> {
    /// `POST /v1/deposit-addresses` — create an address; show `address` to your user. The response
    /// carries an `estimate`. `integrator` / `integrator_fee_bps` default to the client's. Live mode
    /// may be disabled by the operator: a live key then gets a `FORBIDDEN` [`Error::Api`].
    pub async fn create(&self, p: &CreateDepositAddressParams) -> Result<DepositAddress> {
        let mut p = p.clone();
        if p.integrator.is_none() {
            p.integrator = self.client.integrator.clone();
        }
        if p.integrator_fee_bps.is_none() {
            p.integrator_fee_bps = self.client.integrator_fee_bps;
        }
        self.client.call(Method::POST, "/deposit-addresses", &[], Some(serde_json::to_value(&p)?)).await
    }

    /// `GET /v1/deposit-addresses/{id}` — the address with its 20 most recent `deposits` (orders).
    pub async fn get(&self, id: &str) -> Result<DepositAddress> {
        let path = format!("/deposit-addresses/{}", urlencoding::encode(id));
        self.client.call(Method::GET, &path, &[], None).await
    }

    /// `GET /v1/deposit-addresses` — your addresses of the key's mode (live or test), newest first.
    pub async fn list(&self, p: &ListDepositAddressesParams) -> Result<ListDepositAddressesResponse> {
        let mut q: Vec<(&str, String)> = Vec::new();
        if let Some(v) = &p.status {
            q.push(("status", v.clone()));
        }
        if let Some(v) = &p.label {
            q.push(("label", v.clone()));
        }
        if let Some(v) = &p.cursor {
            q.push(("cursor", v.clone()));
        }
        if let Some(v) = p.limit {
            q.push(("limit", v.to_string()));
        }
        self.client.call(Method::GET, "/deposit-addresses", &q, None).await
    }

    /// `POST /v1/deposit-addresses/{id}/refresh` — check for deposits now (call it when your user
    /// says they sent). `CONFLICT` when disabled.
    pub async fn refresh(&self, id: &str) -> Result<DepositAddress> {
        let path = format!("/deposit-addresses/{}/refresh", urlencoding::encode(id));
        self.client.call(Method::POST, &path, &[], Some(json!({}))).await
    }

    /// `POST /v1/deposit-addresses/{id}/disable` — stop tracking the address; tell your user not to
    /// send to it again.
    pub async fn disable(&self, id: &str) -> Result<DepositAddress> {
        let path = format!("/deposit-addresses/{}/disable", urlencoding::encode(id));
        self.client.call(Method::POST, &path, &[], Some(json!({}))).await
    }

    /// `POST /v1/deposit-addresses/{id}/simulate` — test keys only (`FORBIDDEN` otherwise): record a
    /// deposit of `amount` (base units of the source token). It settles like any test order
    /// (amounts ending in 4001 / 4002 end `refunded` / `failed`).
    pub async fn simulate(&self, id: &str, amount: &str) -> Result<SimulateDepositResponse> {
        let path = format!("/deposit-addresses/{}/simulate", urlencoding::encode(id));
        self.client.call(Method::POST, &path, &[], Some(json!({ "amount": amount }))).await
    }
}

/// Checkout product endpoints, from [`Checkout::products`].
#[derive(Debug, Clone, Copy)]
pub struct CheckoutProducts<'a> {
    client: &'a LayerHop,
}

impl CheckoutProducts<'_> {
    /// `POST /v1/checkout/products` — create a product; share its `url` or put it behind a button.
    pub async fn create(&self, p: &CreateCheckoutProductParams) -> Result<CheckoutProduct> {
        self.client.call(Method::POST, "/checkout/products", &[], Some(serde_json::to_value(p)?)).await
    }

    /// `GET /v1/checkout/products/{id}` — public: name, price and settlement; every field with the
    /// owner's key.
    pub async fn get(&self, id: &str) -> Result<CheckoutProduct> {
        let path = format!("/checkout/products/{}", urlencoding::encode(id));
        self.client.call(Method::GET, &path, &[], None).await
    }

    /// `GET /v1/checkout/products` — your products of the key's mode (live or test).
    pub async fn list(&self, p: &ListCheckoutProductsParams) -> Result<ListCheckoutProductsResponse> {
        let mut q: Vec<(&str, String)> = Vec::new();
        if let Some(v) = p.active {
            q.push(("active", v.to_string()));
        }
        if let Some(v) = &p.cursor {
            q.push(("cursor", v.clone()));
        }
        if let Some(v) = p.limit {
            q.push(("limit", v.to_string()));
        }
        self.client.call(Method::GET, "/checkout/products", &q, None).await
    }

    /// `PATCH /v1/checkout/products/{id}` — change only the fields that are `Some`.
    pub async fn update(&self, id: &str, p: &UpdateCheckoutProductParams) -> Result<CheckoutProduct> {
        let path = format!("/checkout/products/{}", urlencoding::encode(id));
        self.client.call(Method::PATCH, &path, &[], Some(serde_json::to_value(p)?)).await
    }

    /// `POST /v1/checkout/products/{id}/sessions` — public: start a session on the product's terms
    /// (`CONFLICT` if inactive). Send the payer to its `url`.
    pub async fn start_session(&self, id: &str, p: &StartProductCheckoutParams) -> Result<CheckoutSession> {
        let path = format!("/checkout/products/{}/sessions", urlencoding::encode(id));
        self.client.call(Method::POST, &path, &[], Some(serde_json::to_value(p)?)).await
    }
}

/// Webhook developer tools, from [`LayerHop::webhooks`]. Both need an API key.
#[derive(Debug, Clone, Copy)]
pub struct Webhooks<'a> {
    client: &'a LayerHop,
}

impl Webhooks<'_> {
    /// `GET /v1/webhooks/stream` — your account's webhook events of the key's mode (live or test)
    /// as they are created, with the exact payloads your endpoints receive. The server closes the
    /// stream after at most 30 minutes; open a new one to continue.
    pub fn stream(&self) -> impl Stream<Item = Result<WebhookEvent>> + Send + 'static {
        self.client.sse("/webhooks/stream".to_string(), "event")
    }

    /// `POST /v1/webhooks/trigger` — a sample event of `event_type` (e.g. `order.filled`) in test
    /// mode, sent to your test endpoints and open streams. Test keys only (`FORBIDDEN` otherwise).
    pub async fn trigger(&self, event_type: &str) -> Result<TriggerWebhookResponse> {
        self.client
            .call(Method::POST, "/webhooks/trigger", &[], Some(json!({ "type": event_type })))
            .await
    }
}

/// Build an [`Error::Api`] from a non-2xx response (JSON error body, else the status text).
async fn api_error(res: reqwest::Response) -> Error {
    let status = res.status();
    let header_request_id = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = match res.bytes().await {
        Ok(b) => b,
        Err(e) => return Error::Http(e),
    };
    let parsed = serde_json::from_slice::<ErrorEnvelope>(&bytes).ok().and_then(|e| e.error);
    let (code, message, request_id) = match parsed {
        Some(e) => (e.code, e.message, e.request_id),
        None => (None, None, None),
    };
    Error::Api {
        code: code.unwrap_or_else(|| "INTERNAL".to_string()),
        message: message
            .unwrap_or_else(|| status.canonical_reason().unwrap_or("request failed").to_string()),
        status: status.as_u16(),
        request_id: request_id.or(header_request_id),
    }
}

/// Incremental Server-Sent Events parser: one line (without its terminator) at a time.
struct SseParser {
    event: &'static str,
    name: String,
    data: Vec<String>,
}

impl SseParser {
    fn new(event: &'static str) -> Self {
        Self { event, name: "message".to_string(), data: Vec::new() }
    }

    /// The joined `data` of a completed frame named `event`. Comments (`:`) and other frames
    /// (`ready`) are skipped.
    fn line(&mut self, line: &str) -> Option<String> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            let name = std::mem::replace(&mut self.name, "message".to_string());
            let data = std::mem::take(&mut self.data);
            return (name == self.event && !data.is_empty()).then(|| data.join("\n"));
        }
        if let Some(v) = line.strip_prefix("event:") {
            self.name = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("data:") {
            self.data.push(v.strip_prefix(' ').unwrap_or(v).to_string());
        }
        None
    }
}

/// Turn a response body into a stream of decoded `event` frames. Ends when the body ends (a
/// partial trailing frame is dropped) or after the first error.
fn parse_sse<T, S, B>(body: S, event: &'static str) -> impl Stream<Item = Result<T>> + Send + 'static
where
    T: DeserializeOwned + Send + 'static,
    S: Stream<Item = reqwest::Result<B>> + Send + 'static,
    B: AsRef<[u8]> + Send + 'static,
{
    struct State<S> {
        body: Pin<Box<S>>,
        buf: Vec<u8>,
        parser: SseParser,
        done: bool,
    }
    let init = State { body: Box::pin(body), buf: Vec::new(), parser: SseParser::new(event), done: false };
    stream::unfold(init, |mut st| async move {
        if st.done {
            return None;
        }
        loop {
            while let Some(pos) = st.buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = st.buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line[..pos]);
                if let Some(data) = st.parser.line(&line) {
                    return match serde_json::from_str::<T>(&data) {
                        Ok(v) => Some((Ok(v), st)),
                        Err(e) => {
                            st.done = true;
                            Some((Err(Error::Decode(e)), st))
                        }
                    };
                }
            }
            match st.body.next().await {
                Some(Ok(chunk)) => st.buf.extend_from_slice(chunk.as_ref()),
                Some(Err(e)) => {
                    st.done = true;
                    return Some((Err(Error::Http(e)), st));
                }
                None => return None,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Ev {
        n: u32,
        s: Option<String>,
    }

    fn body(chunks: &[&'static str]) -> impl Stream<Item = reqwest::Result<&'static [u8]>> + Send + 'static {
        stream::iter(chunks.iter().map(|c| Ok(c.as_bytes())).collect::<Vec<_>>())
    }

    #[tokio::test]
    async fn parses_frames_split_across_chunks() {
        let chunks = [
            ": connected\r\n\r\nevent: ready\r\ndata: {}\r\n\r\nevent: order\r\ndata: {\"n\":",
            "1}\r\n\r",
            "\n: ping\n\nevent: other\ndata: {\"n\":9}\n\nevent: order\ndata: {\"n\":2,\ndata: \"s\":\"a\\nb\"}\n",
            "\nevent: order\ndata: {\"n\":3}\n", // no blank line: incomplete, dropped
        ];
        let got: Vec<Ev> = parse_sse::<Ev, _, _>(body(&chunks), "order").map(|r| r.unwrap()).collect().await;
        assert_eq!(got, vec![Ev { n: 1, s: None }, Ev { n: 2, s: Some("a\nb".into()) }]);
    }

    #[tokio::test]
    async fn bad_json_yields_one_decode_error() {
        let items: Vec<Result<Ev>> =
            parse_sse(body(&["event: order\ndata: {nope\n\nevent: order\ndata: {\"n\":1}\n\n"]), "order").collect().await;
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], Err(Error::Decode(_))));
    }

    #[test]
    fn parser_handles_data_prefix_variants() {
        let mut p = SseParser::new("event");
        assert_eq!(p.line("event:event"), None);
        assert_eq!(p.line("data:x"), None);
        assert_eq!(p.line("data:  y"), None);
        assert_eq!(p.line(""), Some("x\n y".to_string()));
        assert_eq!(p.line("data: z"), None);
        assert_eq!(p.line("\r"), None); // a frame with the default name "message" is ignored
    }
}
