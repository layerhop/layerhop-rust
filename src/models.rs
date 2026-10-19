//! Request / response models. Amounts are always integer strings in base units.
//! Unknown fields are ignored so new API fields never break older clients.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Liquidity provider id (`relay`, `debridge`, `jupiter`, ...). Kept as a string so new providers
/// never break deserialization. `sandbox` serves every request made with an `lh_test_` key;
/// `direct` is a plain token transfer, used by checkout when the payer already holds the
/// settlement token.
pub type ProviderId = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub ok: bool,
    #[serde(default)]
    pub providers: Vec<ProviderId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chain {
    /// EVM chain id, or `7565164` for Solana (LayerHop internal id).
    pub id: u64,
    pub slug: String,
    pub name: String,
    /// `evm` or `svm`.
    pub vm: String,
    pub native_symbol: String,
    pub native_decimals: u32,
    pub explorer_tx: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub chain_id: u64,
    pub address: String,
    pub symbol: String,
    pub decimals: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ChainsResponse {
    pub chains: Vec<Chain>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TokensResponse {
    pub tokens: Vec<Token>,
}

/// Parameters for [`crate::LayerHop::quote`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuoteParams {
    /// Chain id or slug (`"base"`, `"solana"`, `"8453"`).
    pub src_chain: String,
    /// Token symbol from the built-in list (`"USDC"`) or address / mint.
    pub src_token: String,
    /// Integer string in base units of `src_token`.
    pub amount: String,
    pub dst_chain: String,
    pub dst_token: String,
    pub sender: String,
    pub recipient: String,
    /// Defaults to 50.
    pub slippage_bps: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeBreakdown {
    pub our_fee_bps: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_fee_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrator_fee_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub quote_id: String,
    pub provider: ProviderId,
    /// Expected output, base units of the destination token.
    pub dst_amount: String,
    /// Minimum output after slippage.
    pub dst_amount_min: String,
    pub estimated_seconds: u64,
    pub fees: FeeBreakdown,
    /// Unix seconds. Build the order before this time.
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    pub provider: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    pub best: Quote,
    pub quotes: Vec<Quote>,
    #[serde(default)]
    pub errors: Vec<ProviderError>,
    /// `Some(false)` when the request used an `lh_test_` key (sandbox quotes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub livemode: Option<bool>,
}

/// ERC-20 `approve(spender, amount)` on `token` to be mined before the main transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub token: String,
    pub spender: String,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvmTx {
    pub chain_id: u64,
    pub to: String,
    /// Hex calldata.
    pub data: String,
    /// Native value in wei, integer string.
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<Approval>,
    /// Test mode only: a placeholder that moves no funds. Never broadcast it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmTx {
    /// Base64 serialized `VersionedTransaction` for the user's wallet to sign.
    pub serialized_tx: String,
    /// When true, only sign; send the signed tx with [`crate::LayerHop::submit`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit_via_api: Option<bool>,
    /// Test mode only: a placeholder that can never land. Never broadcast it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
}

/// The transaction for the user's wallet, discriminated by `vm`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "vm", rename_all = "lowercase")]
pub enum BuiltTx {
    Evm(EvmTx),
    Svm(SvmTx),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BuildOrderResponse {
    pub tx: BuiltTx,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmitOrderResponse {
    pub tx_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSubmittedResponse {
    pub quote_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulateError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulateResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    pub ok: bool,
    pub vm: String,
    /// EVM gas estimate (integer string; numbers are accepted too).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "string_or_number")]
    pub gas_estimate: Option<String>,
    /// Solana compute units consumed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_units: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_approval: Option<bool>,
    /// `tx` (the swap itself) or `approval` (only the approval could be simulated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<SimulateError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx: Option<BuiltTx>,
}

/// `pending` · `filled` · `refunded` · `failed` · `unknown`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderStatus {
    pub provider: ProviderId,
    pub id: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_tx_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_tx_hash: Option<String>,
}

impl OrderStatus {
    /// `filled`, `refunded` or `failed`.
    pub fn is_final(&self) -> bool {
        is_final_state(&self.state)
    }
}

pub(crate) fn is_final_state(state: &str) -> bool {
    matches!(state, "filled" | "refunded" | "failed")
}

/// Parameters for [`crate::LayerHop::list_orders`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListOrdersParams {
    /// Sender or recipient address.
    pub address: Option<String>,
    pub status: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

/// A recorded order (public fields only). Lenient: every field is optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Order {
    pub quote_id: Option<String>,
    pub provider: Option<ProviderId>,
    pub status: Option<String>,
    pub status_detail: Option<String>,
    pub channel: Option<String>,
    pub our_fee_bps: Option<u32>,
    pub partner_fee_bps: Option<u32>,
    pub src_chain_id: Option<u64>,
    pub src_token: Option<String>,
    pub dst_chain_id: Option<u64>,
    pub dst_token: Option<String>,
    pub amount: Option<String>,
    pub dst_amount: Option<String>,
    pub sender: Option<String>,
    pub recipient: Option<String>,
    pub src_tx_hash: Option<String>,
    pub dst_tx_hash: Option<String>,
    /// `Some(false)` for test-mode orders.
    pub livemode: Option<bool>,
    /// The checkout session this order pays, if any.
    pub checkout_session_id: Option<String>,
    /// The deposit address this order came from, if any.
    pub deposit_address_id: Option<String>,
    /// The checkout refund this order executes, if any.
    pub refund_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOrdersResponse {
    #[serde(default)]
    pub orders: Vec<Order>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Parameters for [`crate::Checkout::create`] (`POST /v1/checkout/sessions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCheckoutParams {
    /// Amount due, integer string in base units of the settlement token (49 USDC = `"49000000"`).
    pub amount: String,
    /// Settlement chain id or slug: ethereum, arbitrum, base, optimism, bsc, polygon or solana.
    pub settle_chain: String,
    /// `"USDC"` or `"USDT"` (or its address) on that chain.
    pub settle_token: String,
    /// Your payout address on the settlement chain.
    pub recipient: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<String>,
    /// Shown to the payer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Your own order id; never shown to the payer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Up to 20 string values; never shown to the payer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<String>,
    /// 5–10080, default 60.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in_minutes: Option<u32>,
}

/// The token a checkout session is paid out in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutSettlement {
    pub chain_id: u64,
    pub token: String,
    pub symbol: String,
    pub decimals: u32,
}

/// The current (or final) payment attempt of a checkout session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutPayment {
    pub quote_id: String,
    #[serde(default)]
    pub payer: Option<String>,
    #[serde(default)]
    pub src_chain_id: Option<u64>,
    #[serde(default)]
    pub src_token: Option<String>,
    /// What the payer sent, in base units of `src_token`.
    #[serde(default)]
    pub src_amount: Option<String>,
    #[serde(default)]
    pub src_tx_hash: Option<String>,
    /// The verified delivery transaction once paid.
    #[serde(default)]
    pub dst_tx_hash: Option<String>,
    #[serde(default)]
    pub paid_at: Option<String>,
}

/// A checkout session (`object: "checkout.session"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutSession {
    pub id: String,
    pub object: String,
    pub livemode: bool,
    /// `open` · `processing` · `paid` · `expired` · `needs_review`.
    pub status: String,
    #[serde(default)]
    pub status_detail: Option<String>,
    /// Amount due, base units of the settlement token.
    pub amount: String,
    pub settlement: CheckoutSettlement,
    pub recipient: String,
    #[serde(default)]
    pub merchant_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// The hosted checkout page: send the payer here.
    pub url: String,
    /// The product the session was started from, if any.
    #[serde(default)]
    pub product_id: Option<String>,
    /// The invoice the session pays, if any.
    #[serde(default)]
    pub invoice_id: Option<String>,
    /// Total of completed refunds, base units of the settlement token (`"0"` if none).
    #[serde(default)]
    pub refunded_amount: Option<String>,
    #[serde(default)]
    pub success_url: Option<String>,
    #[serde(default)]
    pub cancel_url: Option<String>,
    pub expires_at: String,
    /// `None` until a payment is reported.
    #[serde(default)]
    pub payment: Option<CheckoutPayment>,
    /// Only with the owning account's API key.
    #[serde(default)]
    pub reference: Option<String>,
    /// Only with the owning account's API key.
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, String>>,
    pub created_at: String,
    pub updated_at: String,
}

impl CheckoutSession {
    /// `paid`: delivery to the recipient was verified on chain.
    pub fn is_paid(&self) -> bool {
        self.status == "paid"
    }
}

/// Parameters for [`crate::Checkout::list`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListCheckoutSessionsParams {
    pub status: Option<String>,
    pub cursor: Option<String>,
    /// 1–100, default 50.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCheckoutSessionsResponse {
    #[serde(default)]
    pub sessions: Vec<CheckoutSession>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Parameters for [`crate::CheckoutProducts::create`] (`POST /v1/checkout/products`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCheckoutProductParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<String>,
    /// Price, integer string in base units of the settlement token (49 USDC = `"49000000"`).
    pub amount: String,
    /// Settlement chain id or slug: ethereum, arbitrum, base, optimism, bsc, polygon or solana.
    pub settle_chain: String,
    /// `"USDC"` or `"USDT"` (or its address) on that chain.
    pub settle_token: String,
    /// Your payout address on the settlement chain.
    pub recipient: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<String>,
    /// Lifetime of each session started from the product: 5–10080, default 60.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in_minutes: Option<u32>,
    /// https origins allowed to start sessions from a browser; empty = any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_origins: Option<Vec<String>>,
    /// Defaults to `true` on the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

/// Parameters for [`crate::CheckoutProducts::update`] (`PATCH /v1/checkout/products/{id}`).
/// Only `Some` fields are sent; `Some(None)` on a nullable field sends `null` to clear it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckoutProductParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_chain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_url: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_origins: Option<Vec<String>>,
    /// `Some(false)` stops new sessions; existing sessions keep their terms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

/// A product (`object: "checkout.product"`): a fixed price for payment buttons,
/// `layerhop.com/buy/{id}` links and store plugins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutProduct {
    pub id: String,
    pub object: String,
    pub livemode: bool,
    pub active: bool,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub merchant_name: Option<String>,
    /// Price, base units of the settlement token.
    pub amount: String,
    pub settlement: CheckoutSettlement,
    /// `https://layerhop.com/buy/{id}`: starts a session and opens the payment page.
    pub url: String,
    /// Owner only (with the account's API key).
    #[serde(default)]
    pub recipient: Option<String>,
    /// Owner only.
    #[serde(default)]
    pub success_url: Option<String>,
    /// Owner only.
    #[serde(default)]
    pub cancel_url: Option<String>,
    /// Owner only.
    #[serde(default)]
    pub expires_in_minutes: Option<u32>,
    /// Owner only.
    #[serde(default)]
    pub allowed_origins: Option<Vec<String>>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Parameters for [`crate::CheckoutProducts::list`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListCheckoutProductsParams {
    /// Filter by state when set.
    pub active: Option<bool>,
    pub cursor: Option<String>,
    /// 1–100, default 50.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCheckoutProductsResponse {
    #[serde(default)]
    pub products: Vec<CheckoutProduct>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Parameters for [`crate::CheckoutProducts::start_session`]: the only things a caller may add to
/// the product's terms.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartProductCheckoutParams {
    /// Your order id, returned in the session and its webhooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Up to 10 string values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
}

/// Parameters for [`crate::Checkout::quote`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutQuoteParams {
    /// The chain the payer pays from (id or slug).
    pub src_chain: String,
    /// Token address / mint, curated symbol or native symbol.
    pub src_token: String,
    /// The payer's address on `src_chain`.
    pub sender: String,
    /// Defaults to 50 on the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slippage_bps: Option<u32>,
}

/// A [`Quote`] for paying a checkout session, plus what the payer sends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutQuote {
    pub quote_id: String,
    pub provider: ProviderId,
    /// What the payer sends, base units of the chosen token.
    pub src_amount: String,
    pub dst_amount: String,
    /// Guaranteed delivery to the recipient; always >= the amount due.
    pub dst_amount_min: String,
    pub estimated_seconds: u64,
    pub fees: FeeBreakdown,
    /// Unix seconds.
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutQuoteResponse {
    pub session_id: String,
    pub livemode: bool,
    pub amount_due: String,
    pub best: CheckoutQuote,
    pub quotes: Vec<CheckoutQuote>,
    #[serde(default)]
    pub errors: Vec<ProviderError>,
}

/// The JSON body LayerHop POSTs to your webhook endpoints, and what
/// [`crate::Webhooks::stream`] yields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookEvent {
    pub id: String,
    /// `order.submitted` · `order.filled` · `order.refunded` · `order.failed` ·
    /// `checkout.session.processing` · `checkout.session.completed` ·
    /// `checkout.session.payment_failed` · `checkout.session.expired` ·
    /// `checkout.session.refunded` · `invoice.sent` · `invoice.paid` (or `webhook.test`).
    #[serde(rename = "type")]
    pub event_type: String,
    pub livemode: bool,
    /// `Some(true)` on samples from the dashboard or [`crate::Webhooks::trigger`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<bool>,
    pub created_at: String,
    #[serde(default)]
    pub data: WebhookEventData,
}

/// The objects a [`WebhookEvent`] is about: `order` for `order.*` events, `session` for
/// `checkout.session.*` events (plus `refund` for `checkout.session.refunded`), `invoice` for
/// `invoice.*` events (plus the paying `session` for `invoice.paid`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WebhookEventData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Order>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<CheckoutSession>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refund: Option<Refund>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice: Option<Invoice>,
}

/// Response of [`crate::Webhooks::trigger`] (`POST /v1/webhooks/trigger`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerWebhookResponse {
    pub event_id: String,
    /// Test endpoints the event was queued for.
    pub deliveries: u32,
    pub payload: WebhookEvent,
}

/// One line of an invoice; the line total is `unit_amount × quantity`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceLineItem {
    pub description: String,
    /// 1–10000.
    pub quantity: u32,
    /// Integer string in base units of the settlement token.
    pub unit_amount: String,
}

/// Parameters for [`crate::Invoices::create`] (`POST /v1/invoices`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInvoiceParams {
    /// Receives the invoice email (and the receipt once paid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// 1–50 lines; the invoice amount is their sum.
    pub line_items: Vec<InvoiceLineItem>,
    /// Settlement chain id or slug.
    pub settle_chain: String,
    /// `"USDC"` or `"USDT"` (or its address) on that chain.
    pub settle_token: String,
    /// Your payout address on the settlement chain.
    pub recipient: String,
    /// ISO 8601 date (`"2026-11-01"`) or date-time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_at: Option<String>,
    /// `Some(true)` finalizes the invoice and emails it right away (status `open`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send: Option<bool>,
}

/// Parameters for [`crate::Invoices::update`] (`PATCH /v1/invoices/{id}`, drafts only).
/// Only `Some` fields are sent; `Some(None)` on a nullable field sends `null` to clear it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInvoiceParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_email: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<Option<String>>,
    /// Replaces every line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_items: Option<Vec<InvoiceLineItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_chain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_at: Option<Option<String>>,
}

/// An invoice (`object: "invoice"`): line items billed to a customer by email and paid through
/// checkout. Owner-only fields (`customer_email`, `recipient`, `session_id`, `sent_at`, ...) are
/// `None` in [`crate::Invoices::public_view`]. The `invoice.paid` webhook carries a summary, so
/// `line_items` may be empty and `url` / `created_at` `None` there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invoice {
    pub id: String,
    pub object: String,
    pub livemode: bool,
    /// `INV-0001`, numbered per account and mode.
    pub number: String,
    /// `draft` · `open` · `paid` · `void`.
    pub status: String,
    #[serde(default)]
    pub merchant_name: Option<String>,
    #[serde(default)]
    pub customer_name: Option<String>,
    #[serde(default)]
    pub memo: Option<String>,
    #[serde(default)]
    pub line_items: Vec<InvoiceLineItem>,
    /// Total, base units of the settlement token.
    pub amount: String,
    pub settlement: CheckoutSettlement,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub paid_at: Option<String>,
    /// The hosted invoice page, `https://layerhop.com/invoice/{id}`.
    #[serde(default)]
    pub url: Option<String>,
    /// PDF receipt, once paid.
    #[serde(default)]
    pub receipt_url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    /// Owner only.
    #[serde(default)]
    pub customer_email: Option<String>,
    /// Owner only.
    #[serde(default)]
    pub recipient: Option<String>,
    /// Owner only: the checkout session that pays (or last tried to pay) the invoice.
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub sent_at: Option<String>,
    #[serde(default)]
    pub receipt_sent_at: Option<String>,
    #[serde(default)]
    pub voided_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl Invoice {
    /// `paid`: the checkout session that pays it was verified on chain.
    pub fn is_paid(&self) -> bool {
        self.status == "paid"
    }
}

/// Parameters for [`crate::Invoices::list`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListInvoicesParams {
    /// `draft` · `open` · `paid` · `void`.
    pub status: Option<String>,
    pub cursor: Option<String>,
    /// 1–100.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInvoicesResponse {
    #[serde(default)]
    pub invoices: Vec<Invoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Response of [`crate::Invoices::pay`]: the checkout session that pays the invoice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayInvoiceResponse {
    pub session_id: String,
    /// The hosted checkout page: send the payer here.
    pub url: String,
}

/// Parameters for [`crate::DepositAddresses::create`] (`POST /v1/deposit-addresses`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDepositAddressParams {
    /// Chain id or slug the user sends on (`"base"`, `"8453"`).
    pub src_chain: String,
    /// Token the user sends (symbol or address on `src_chain`).
    pub src_token: String,
    pub dst_chain: String,
    pub dst_token: String,
    /// Receives `dst_token` on `dst_chain`.
    pub recipient: String,
    /// The user's address on `src_chain` for refunds. Never a CEX deposit address.
    pub refund_to: String,
    /// `None` / `Some(true)`: reusable (any amount, forever, for this route). `Some(false)`: a
    /// one-time address for exactly `amount`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reusable: Option<bool>,
    /// Base units of `src_token`: required for a one-time address; otherwise it sizes the estimate
    /// (API default: about $100 of `src_token`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Your own tag (1–64 chars), filterable in [`crate::DepositAddresses::list`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Up to 20 string values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    /// Defaults to the client's [`crate::LayerHopBuilder::integrator`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrator: Option<String>,
    /// Defaults to the client's [`crate::LayerHopBuilder::integrator_fee_bps`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrator_fee_bps: Option<u32>,
}

/// Fees applied to every deposit of a [`DepositAddress`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositAddressFees {
    pub our_fee_bps: u32,
    pub integrator_fee_bps: u32,
    /// The partner id, if a partner fee applies.
    #[serde(default)]
    pub integrator: Option<String>,
}

/// Price of one deposit of `amount`, returned by [`crate::DepositAddresses::create`] only.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositAddressEstimate {
    pub amount: String,
    pub dst_amount: String,
    pub dst_amount_min: String,
    pub estimated_seconds: u64,
}

/// A deposit address (`object: "deposit_address"`): the end user sends `token` to `address` on
/// `chain_id`, and the provider delivers `dst_token` to `recipient` on `dst_chain_id`. LayerHop
/// never holds the funds; each deposit becomes an [`Order`] with `deposit_address_id` set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositAddress {
    pub id: String,
    pub object: String,
    pub livemode: bool,
    /// `active` · `disabled`.
    pub status: String,
    /// `relay` (live) or `sandbox` (test mode).
    pub provider: ProviderId,
    /// Show this to your user: they send `token` here on `chain_id`.
    pub address: String,
    pub chain_id: u64,
    pub token: String,
    pub dst_chain_id: u64,
    pub dst_token: String,
    pub recipient: String,
    pub refund_to: String,
    pub reusable: bool,
    /// One-time addresses only, base units of `token`.
    #[serde(default)]
    pub amount: Option<String>,
    pub fees: DepositAddressFees,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub deposit_count: u64,
    #[serde(default)]
    pub last_deposit_at: Option<String>,
    pub created_at: String,
    /// Set by [`crate::DepositAddresses::create`] only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<DepositAddressEstimate>,
    /// The 20 most recent deposits, newest first ([`crate::DepositAddresses::get`] only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deposits: Option<Vec<Order>>,
}

impl DepositAddress {
    /// `active`: deposits sent to it are tracked.
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }
}

/// Parameters for [`crate::DepositAddresses::list`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListDepositAddressesParams {
    /// `active` · `disabled`.
    pub status: Option<String>,
    pub label: Option<String>,
    pub cursor: Option<String>,
    /// 1–100.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDepositAddressesResponse {
    #[serde(default)]
    pub deposit_addresses: Vec<DepositAddress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Response of [`crate::DepositAddresses::simulate`]: the test deposit order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulateDepositResponse {
    pub order: Order,
}

/// Parameters for [`crate::CheckoutRefunds::create`]. All optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRefundParams {
    /// Base units of the settlement token; default: everything not yet refunded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The wallet that signs the refund (default: the session's payout address).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// `original` (default): the payer's chain and token. `settlement`: the settlement token, to
    /// the payer's address on the settlement chain (same chain family only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// Where a refund is delivered.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundDestination {
    pub chain_id: u64,
    pub token: String,
    pub address: String,
}

/// A refund of a paid checkout session (`object: "refund"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Refund {
    pub id: String,
    pub object: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub livemode: Option<bool>,
    /// `pending` · `submitted` · `completed` · `failed`.
    pub status: String,
    #[serde(default)]
    pub status_detail: Option<String>,
    /// Base units of the settlement token.
    pub amount: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub quote_id: Option<String>,
    pub to: RefundDestination,
    #[serde(default)]
    pub tx_hash: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// The route that executes a refund: [`crate::LayerHop::build_tx`] it, sign it from `from`, then
/// [`crate::LayerHop::report_submitted`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundQuote {
    pub quote_id: String,
    pub provider: ProviderId,
    /// Settlement-token base units sent by the merchant.
    pub src_amount: String,
    /// What the payer receives, base units of the refund token.
    pub dst_amount: String,
    pub dst_amount_min: String,
    pub estimated_seconds: u64,
    /// Unix seconds.
    pub expires_at: u64,
    /// The wallet that must sign the refund transaction.
    pub from: String,
}

/// Response of [`crate::CheckoutRefunds::create`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRefundResponse {
    pub refund: Refund,
    pub quote: RefundQuote,
    /// The remaining steps (build, sign, report).
    #[serde(default)]
    pub next: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRefundsResponse {
    #[serde(default)]
    pub refunds: Vec<Refund>,
}

/// One day of a component's uptime history.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDay {
    /// `YYYY-MM-DD` (UTC).
    pub date: String,
    /// Percent of passing checks; `None` without data.
    #[serde(default)]
    pub uptime: Option<f64>,
}

/// A monitored component in [`StatusResponse`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusComponent {
    pub id: String,
    pub name: String,
    pub group: String,
    /// `operational` · `degraded` · `down`.
    pub status: String,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    /// Percent over the last 90 days; `None` without data.
    #[serde(default, rename = "uptime90d")]
    pub uptime_90d: Option<f64>,
    /// The last 90 days, oldest first.
    #[serde(default)]
    pub days: Vec<StatusDay>,
}

/// An incident from the last 14 days.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusIncident {
    pub id: String,
    pub component_id: String,
    pub component: String,
    pub title: String,
    pub started_at: String,
    /// `None` while ongoing.
    #[serde(default)]
    pub resolved_at: Option<String>,
}

/// Response of [`crate::LayerHop::service_status`] (`GET /v1/status`, shown at
/// status.layerhop.com).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    /// `operational` · `degraded` · `outage`.
    pub status: String,
    pub updated_at: String,
    #[serde(default)]
    pub components: Vec<StatusComponent>,
    #[serde(default)]
    pub incidents: Vec<StatusIncident>,
    pub url: String,
}

/// Accepts `"21000"` or `21000` and yields `Some("21000")`.
fn string_or_number<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(other) => Some(other.to_string()),
    })
}
