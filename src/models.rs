//! Request / response models. Amounts are always integer strings in base units.
//! Unknown fields are ignored so new API fields never break older clients.

use serde::{Deserialize, Serialize};

/// Liquidity provider id (`relay`, `debridge`, `jupiter`, ...). Kept as a string so new providers
/// never break deserialization.
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvmTx {
    /// Base64 serialized `VersionedTransaction` for the user's wallet to sign.
    pub serialized_tx: String,
    /// When true, only sign; send the signed tx with [`crate::LayerHop::submit`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit_via_api: Option<bool>,
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
