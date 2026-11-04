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
    /// `evm`, `svm` (Solana) or `tvm` (Tron, where the API runs with Tron enabled).
    pub vm: String,
    pub native_symbol: String,
    pub native_decimals: u32,
    pub explorer_tx: String,
    #[serde(default, rename = "logoURI", skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explorer_address: Option<String>,
    /// Providers that can route from or to this chain.
    #[serde(default)]
    pub providers: Vec<ProviderId>,
    /// Shown first in chain pickers.
    #[serde(default)]
    pub popular: bool,
    /// `Some(true)` for chains you can deliver to but not send from (Hyperliquid, Blast). Hide them
    /// from source pickers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_only: Option<bool>,
    /// Present when every provider of this chain serves one fixed route into it (Blast: ETH on
    /// Ethereum → ETH on Blast). Quotes into the chain from anything else are `UNSUPPORTED_CHAIN`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only_route: Option<OnlyRoute>,
    /// Result of LayerHop's periodic read-only probe quotes into and out of the chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_health: Option<ChainRouteHealth>,
}

/// The one route into a [`Chain::only_route`] chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlyRoute {
    pub src_chain_id: u64,
    /// Source token address (EVM native = `0x000…000`).
    pub src_token: String,
    /// Destination token address on this chain.
    pub dst_token: String,
}

/// [`Chain::route_health`]. Strings so new values never break decoding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainRouteHealth {
    /// Into the chain: `ok` · `failing` · `unknown`.
    #[serde(rename = "in")]
    pub inbound: String,
    /// Out of the chain: `ok` · `failing` · `unknown`.
    #[serde(rename = "out")]
    pub outbound: String,
    /// `ok` · `deposit_only` · `withdraw_only` · `unavailable` · `unknown`.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub chain_id: u64,
    /// Address or mint. EVM native coin: `0x000…000`; native SOL: the wrapped-SOL mint.
    pub address: String,
    pub symbol: String,
    #[serde(default)]
    pub name: String,
    pub decimals: u32,
    #[serde(default, rename = "logoURI", skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    /// USD price when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_usd: Option<f64>,
    /// On a provider list or LayerHop's curated list; `false` for tokens read on-chain only. Show a
    /// warning before a user trades an unverified token.
    #[serde(default)]
    pub verified: bool,
    /// Stables, wrapped native and majors (the canonical one per symbol and chain).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popular: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ChainsResponse {
    pub chains: Vec<Chain>,
}

/// One page of [`crate::LayerHop::search_tokens`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokensPage {
    #[serde(default)]
    pub tokens: Vec<Token>,
    /// Pass as [`TokenSearchParams::cursor`] for the next page; `None` on the last page.
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// Parameters for [`crate::LayerHop::search_tokens`] (`GET /v1/tokens`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TokenSearchParams {
    /// Chain id or slug.
    pub chain: String,
    /// Symbol, name or address. An unlisted address is read on-chain (`verified: false`).
    pub q: Option<String>,
    /// Page size, 1–200 (default 50).
    pub limit: Option<u32>,
    /// [`TokensPage::next_cursor`] of the previous page.
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TokenLookupResponse {
    pub token: Token,
}

/// One reason in a [`TokenRisk`] report. Strings so new codes never break decoding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRiskReason {
    /// `honeypot`, `sell_tax`, `fake_symbol`, `mintable`, ... (see the API reference).
    pub code: String,
    /// `caution` or `danger`.
    pub level: String,
    /// One plain sentence for the user.
    pub message: String,
    /// Percent for `buy_tax` / `sell_tax` / `transfer_fee`; holder count for `few_holders`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

/// A token's safety report, from [`crate::LayerHop::get_token_risk`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRisk {
    pub chain_id: u64,
    pub address: String,
    /// `ok` · `caution` · `danger` · `unknown` (no source answered; never block a swap on it).
    pub level: String,
    /// A major LayerHop lists itself; never checked upstream.
    #[serde(default)]
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default)]
    pub reasons: Vec<TokenRiskReason>,
    /// `list` · `goplus` · `none`.
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub checked_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TokenRiskResponse {
    pub risk: TokenRisk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PricesResponse {
    #[serde(default)]
    pub prices: BTreeMap<String, f64>,
}

/// Parameters for [`crate::LayerHop::balances`] (`GET /v1/balances`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BalancesParams {
    /// EVM address or Solana wallet.
    pub address: String,
    /// Chain ids or slugs (at most 25), all of the address's VM. Default: popular EVM chains, or
    /// Solana for a Solana address.
    pub chains: Option<Vec<String>>,
    /// `chainId:address` entries (at most 20) always returned, even at zero.
    pub tokens: Option<Vec<String>>,
    /// Reload balances cached more than 4 seconds ago (sent as `fresh=1`).
    pub fresh: bool,
}

/// A wallet balance: the token plus the amount held.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    #[serde(flatten)]
    pub token: Token,
    /// Base units.
    pub amount: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_usd: Option<f64>,
}

/// A chain (or requested token) that could not be read in [`BalancesResponse`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceError {
    pub chain_id: u64,
    pub message: String,
}

/// Response of [`crate::LayerHop::balances`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalancesResponse {
    pub address: String,
    pub vm: String,
    /// Highest USD value first.
    #[serde(default)]
    pub balances: Vec<Balance>,
    #[serde(default)]
    pub total_usd: f64,
    #[serde(default)]
    pub errors: Vec<BalanceError>,
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
    /// Also deliver native gas to the recipient on the destination chain. Ignored for same-chain
    /// swaps, non-EVM destinations and native outputs; providers that can't do it ignore it
    /// (check [`Quote::gas_on_arrival`]).
    pub gas_on_arrival: Option<GasOnArrivalRequest>,
    /// Gasless swap (sent as `gasless=true`): the user signs EIP-712 typed data and the provider's
    /// relayer pays the network fee out of `src_token`. ERC-20 input, same EVM chain, live mode only.
    /// Then [`crate::LayerHop::build_gasless`] → sign → [`crate::LayerHop::submit_gasless`].
    pub gasless: Option<bool>,
    /// Wallet that referred the user (attribution only; invalid values are ignored).
    pub referrer: Option<String>,
    /// Copy this trade: a trade card (`q_…`) or the copied wallet (attribution only).
    pub copy_of: Option<String>,
    /// What the payment is for: `tip` · `split` · `payout` · `gather` · `rebalance` · `fund`.
    /// Attribution only; never changes the fee or the route.
    pub purpose: Option<String>,
    /// Pay one share of a split ([`crate::Splits::create`]), with `split_share`.
    pub split_id: Option<String>,
    /// The share's `index` in the split (from 0).
    pub split_share: Option<u32>,
    /// A message to the recipient on a tip or split payment (kept to 140 characters).
    pub note: Option<String>,
}

/// One contract call on the destination chain. `msg.sender` is the provider's executor, not the
/// user: targets must credit `recipient` from a parameter.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DestinationCall {
    pub to: String,
    /// ABI-encoded calldata (`0x` hex).
    pub data: String,
    /// Wei sent with the call (default `"0"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// A preset that builds the destination calls server-side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationAction {
    /// `aave-v3-supply`.
    #[serde(rename = "type")]
    pub action_type: String,
}

impl DestinationAction {
    /// Approve the chain's Aave v3 Pool and supply `dst_amount` of `dst_token` for `recipient`.
    pub fn aave_v3_supply() -> Self {
        Self { action_type: "aave-v3-supply".to_string() }
    }
}

/// Parameters for [`crate::LayerHop::quote_calls`] (`POST /v1/quote`): a quote by exact output
/// that runs destination calls with it. Send `calls` or `action`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CallsQuoteParams {
    pub src_chain: String,
    pub src_token: String,
    pub dst_chain: String,
    pub dst_token: String,
    /// Exact base units of `dst_token` the calls receive. Each quote reports `src_amount`.
    pub dst_amount: String,
    pub sender: String,
    pub recipient: String,
    /// Defaults to 50.
    pub slippage_bps: Option<u32>,
    /// At most 8, run in order by the provider's executor.
    pub calls: Option<Vec<DestinationCall>>,
    pub action: Option<DestinationAction>,
    pub referrer: Option<String>,
    pub copy_of: Option<String>,
}

/// Parameters for [`crate::LayerHop::quote_exact_output`]: receive exactly `dst_amount`. Each
/// quote reports `src_amount` (what `sender` pays) and `src_amount_max` (the most the transaction
/// can take, for balance and approval checks). Only providers that quote a fixed output answer;
/// not with gas on arrival or gasless.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExactOutputQuoteParams {
    pub src_chain: String,
    pub src_token: String,
    pub dst_chain: String,
    pub dst_token: String,
    /// The exact base units of `dst_token` the recipient receives.
    pub dst_amount: String,
    pub sender: String,
    pub recipient: String,
    /// Defaults to 50.
    pub slippage_bps: Option<u32>,
    pub referrer: Option<String>,
    pub copy_of: Option<String>,
    /// As [`QuoteParams::purpose`].
    pub purpose: Option<String>,
    pub split_id: Option<String>,
    pub split_share: Option<u32>,
    pub note: Option<String>,
}

/// Options for [`crate::LayerHop::quote_stream`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuoteStreamOptions {
    /// Refresh interval in ms (server default 10000, minimum 5000). Each refresh counts as one
    /// request against your rate limit.
    pub interval_ms: Option<u32>,
}

/// One `quote` event of a live quote stream: a full [`QuoteResponse`] plus its sequence number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuoteStreamUpdate {
    pub seq: u64,
    #[serde(flatten)]
    pub response: QuoteResponse,
}

/// The destination calls of a quote, as the provider committed to them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationCallsPlan {
    #[serde(default)]
    pub items: Vec<DestinationCall>,
    /// The contract that receives the output and makes the calls (their `msg.sender`).
    pub executor: String,
    #[serde(default)]
    pub executed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation: Option<CallsSimulation>,
}

/// The mandatory pre-quote simulation of a [`DestinationCallsPlan`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallsSimulation {
    pub ok: bool,
    /// Gas the calls used together (decimal string; numbers are accepted too).
    #[serde(default, deserialize_with = "string_or_number")]
    pub gas_used: Option<String>,
    /// ERC-20 output left in the executor after the calls; anything above 0 is lost to you.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leftover: Option<String>,
}

/// Network fee a gasless relayer takes from the input token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GasFee {
    /// Base units of `token`.
    pub amount: String,
    pub token: String,
}

/// [`Quote::gasless`]: on gasless quotes only. `dst_amount` already accounts for the gas fee.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GaslessQuoteInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_fee: Option<GasFee>,
    /// `true` when the wallet signs a gasless approval (permit) before the trade: two signatures.
    #[serde(default)]
    pub approval: bool,
}

/// EIP-712 typed data exactly as the provider returned it. Sign it unchanged
/// (`eth_signTypedData_v4`); `types`, `domain` and `message` are kept as raw JSON for that reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Eip712TypedData {
    pub types: serde_json::Value,
    pub domain: serde_json::Value,
    pub message: serde_json::Value,
    pub primary_type: String,
}

/// One thing to sign for a gasless order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GaslessSignable {
    /// The provider's kind, e.g. `permit` (approval) or `settler_metatransaction` (trade).
    #[serde(rename = "type")]
    pub kind: String,
    pub eip712: Eip712TypedData,
}

/// What the wallet signs for a gasless quote, from [`crate::LayerHop::build_gasless`]: `approval`
/// (when present), then `trade`, on `chain_id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GaslessSignRequest {
    pub chain_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<GaslessSignable>,
    pub trade: GaslessSignable,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_fee: Option<GasFee>,
}

/// State of a submitted gasless order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GaslessOrderStatus {
    pub quote_id: String,
    pub provider: ProviderId,
    /// Provider order id (0x: the trade's EIP-712 hash).
    pub order_id: String,
    /// An order state: `pending` · `filled` · `failed` · ...
    pub state: String,
    /// The relayer's transaction, once mined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_tx_hash: Option<String>,
    /// Provider failure reason when `failed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// How much native gas [`QuoteParams::gas_on_arrival`] asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GasOnArrivalRequest {
    /// The default, $2 (sent as `gasOnArrival=true`).
    Default,
    /// A USD amount, 0.5–20 (sent as `gasOnArrivalUsd`).
    Usd(f64),
}

/// Native gas a route delivers to the recipient on top of `dst_amount` (its cost is already taken
/// out of `dst_amount`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasOnArrival {
    /// Base units of the destination chain's native token.
    pub amount: String,
    /// Native token address on the destination chain (`0x000…000`).
    pub token: String,
    pub symbol: String,
    pub decimals: u32,
    /// USD value at quote time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd: Option<f64>,
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
    /// Native gas delivered to the recipient on top of `dst_amount` (when requested and included).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_on_arrival: Option<GasOnArrival>,
    /// What `sender` pays, fees included (exact-output quotes: [`crate::LayerHop::quote_exact_output`]
    /// and destination calls).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_amount: Option<String>,
    /// Exact-output quotes: the most the transaction can take from `sender`, slippage included
    /// (>= `src_amount`). Check balances and approvals against this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_amount_max: Option<String>,
    /// The destination calls and their simulation ([`crate::LayerHop::quote_calls`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<DestinationCallsPlan>,
    /// On gasless quotes only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gasless: Option<GaslessQuoteInfo>,
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
    /// Present only when the request was served, but not exactly as asked (e.g.
    /// `GAS_ON_ARRIVAL_UNAVAILABLE`). Ignore codes you don't know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<QuoteWarning>>,
    /// `Some(false)` when the request used an `lh_test_` key (sandbox quotes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub livemode: Option<bool>,
    /// Address screening (D-069): which of this request's addresses are on the OFAC SDN list.
    /// Present once the API has a list loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screening: Option<ScreeningAnnotation>,
}

/// A note on a quote response that was served, but not exactly as asked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteWarning {
    pub code: String,
    pub message: String,
}

/// ERC-20 `approve(spender, amount)` on `token` to be mined before the main transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub token: String,
    pub spender: String,
    pub amount: String,
}

/// A plain EVM call: target, hex calldata and native value in wei.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvmCall {
    pub to: String,
    pub data: String,
    pub value: String,
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
    /// Further calls to send after the main transaction, in order (e.g. LayerHop's fee transfer on a
    /// same-token checkout payment). Report the main transaction's hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_txs: Option<Vec<EvmCall>>,
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

/// One Tron smart-contract call (`TriggerSmartContract`). Addresses are base58 T-addresses;
/// amounts are sun (1 TRX = 1,000,000 sun).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TronCall {
    pub to: String,
    /// ABI calldata, `0x` hex.
    pub data: String,
    /// TRX sent with the call, sun.
    pub value: String,
    /// `fee_limit` in sun: the most TRX the network may burn for this call's energy.
    pub fee_limit: String,
}

/// An unsigned Tron transaction exactly as the node built it (TronWeb's transaction object). Sign
/// it unchanged and broadcast it before `raw_data.expiration`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TronUnsignedTx {
    /// The tx hash once broadcast: bare 64-hex, no `0x`.
    #[serde(rename = "txID")]
    pub tx_id: String,
    /// Protobuf-encoded raw transaction (what gets signed).
    pub raw_data_hex: String,
    /// Decoded `raw_data` (contract, ref block, expiration, fee_limit), kept as raw JSON so it
    /// goes to your Tron wallet unchanged.
    pub raw_data: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

/// TRC-20 approval to send before [`TronTx::call`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TronApproval {
    pub token: String,
    pub spender: String,
    /// Base units.
    pub amount: String,
    pub call: TronCall,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsigned: Option<TronUnsignedTx>,
}

/// Tron transaction(s) for a quote (D-048). The SDK never signs: sign `unsigned` unchanged with a
/// Tron wallet (TronWeb / TronLink), send `approval` first when present, broadcast each before
/// `raw_data.expiration`, then [`crate::LayerHop::report_submitted`] with the main transaction's
/// `tx_id` (bare 64-hex).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TronTx {
    /// Always [`crate::TRON_CHAIN_ID`].
    pub chain_id: u64,
    /// Owner (signer) of every call, T-address.
    pub from: String,
    pub call: TronCall,
    /// The main call prebuilt (absent in test mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsigned: Option<TronUnsignedTx>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<TronApproval>,
    /// Test mode only: a placeholder that moves no funds. Never broadcast it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
}

/// The transaction for the user's wallet, discriminated by `vm`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "vm", rename_all = "lowercase")]
#[allow(clippy::large_enum_variant)]
pub enum BuiltTx {
    Evm(EvmTx),
    Svm(SvmTx),
    /// Tron (`vm: "tvm"`).
    Tvm(TronTx),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BuildOrderResponse {
    #[serde(default)]
    pub tx: Option<BuiltTx>,
    #[serde(default)]
    pub gasless: Option<GaslessSignRequest>,
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
    /// `evm` · `svm` · `tvm`.
    pub vm: String,
    /// EVM gas estimate (integer string; numbers are accepted too).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "string_or_number")]
    pub gas_estimate: Option<String>,
    /// Solana compute units consumed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_units: Option<u64>,
    /// Tron energy the simulated call used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy_used: Option<u64>,
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

/// `pending` · `filled` · `refunded` · `failed` · `unknown` · `dropped` (the source tx never
/// reached the network, no funds moved; final, re-quote).
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
    /// `filled`, `refunded`, `failed` or `dropped`.
    pub fn is_final(&self) -> bool {
        is_final_state(&self.state)
    }
}

pub(crate) fn is_final_state(state: &str) -> bool {
    matches!(state, "filled" | "refunded" | "failed" | "dropped")
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
    /// The limit order this order is a fill of, if any (`quote_id` then starts with `lof_`).
    pub limit_order_id: Option<String>,
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
    /// Embedded checkout: the only sites (exact origins) that may show the session in an iframe.
    /// Empty = any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed_origins: Option<Vec<String>>,
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
    /// `wallet`: paid through a quote on the page; `transfer`: a scan-to-pay transfer (D-064), no
    /// quote.
    #[serde(default)]
    pub method: Option<String>,
    /// `None` for a scan-to-pay transfer.
    #[serde(default)]
    pub quote_id: Option<String>,
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
    /// Only when paid plans are enabled: `false` when the merchant's plan is white-label.
    #[serde(default)]
    pub branding: Option<bool>,
    /// The payer can be shown a "Send to address" QR ([`crate::Checkout::start_transfer`]).
    #[serde(default)]
    pub transfer_available: Option<bool>,
    /// Scan-to-pay state once the payer asked for a QR, else `None`.
    #[serde(default)]
    pub transfer: Option<CheckoutTransfer>,
    /// Sites allowed to embed this checkout (empty = any).
    #[serde(default)]
    pub embed_origins: Option<Vec<String>>,
    /// "Pay with Bitcoin" (D-067; only where the API enabled it): can this session show a BTC
    /// deposit address.
    #[serde(default)]
    pub bitcoin_available: Option<bool>,
    /// The Bitcoin payment once the payer asked for it ([`crate::Checkout::start_bitcoin`]).
    #[serde(default)]
    pub bitcoin: Option<CheckoutBitcoin>,
    pub created_at: String,
    pub updated_at: String,
}

/// Scan-to-pay ("Send to address", D-064). `evm_transfer`: send exactly `amount` of `token` to
/// `address` on `chain_id` (the amount carries a tag in its sub-cent digits). `solana_pay`: scan
/// `uri` (a Solana Pay transaction request) with a Solana wallet. `evm_splitter` (D-065): send
/// exactly `amount` to `address`, the session's own fee-splitting deposit address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutTransfer {
    /// `evm_transfer` · `solana_pay` · `evm_splitter`.
    pub kind: String,
    pub chain_id: u64,
    pub token: String,
    pub symbol: String,
    pub decimals: u32,
    pub address: String,
    /// What to send now, base units (EVM: the exact tagged amount; after a short payment, the rest).
    pub amount: String,
    pub amount_due: String,
    /// LayerHop's fee on top, inside the same Solana Pay transaction.
    #[serde(default)]
    pub fee: Option<CheckoutTransferFee>,
    /// QR payload: EIP-681 (EVM) or `solana:https://…/solana-pay` (Solana Pay transaction request).
    pub uri: String,
    /// Solana Pay transfer request (plain transfer), only when enabled.
    #[serde(default)]
    pub transfer_uri: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    /// Confirmed so far, base units.
    pub received: String,
    /// Seen but still confirming, base units.
    pub seen: String,
    /// `waiting` · `partial` · `paid` · `overpaid`.
    pub state: String,
    #[serde(default)]
    pub txs: Vec<CheckoutTransferTx>,
    pub started_at: String,
    /// `evm_splitter` only: the sweep that paid the deposit out to the merchant.
    #[serde(default)]
    pub sweep_tx_hash: Option<String>,
}

/// LayerHop's fee on a [`CheckoutTransfer`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckoutTransferFee {
    pub bps: u32,
    pub amount: String,
}

/// One transaction that paid a [`CheckoutTransfer`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckoutTransferTx {
    pub hash: String,
    pub amount: String,
    #[serde(default)]
    pub at: Option<String>,
}

/// "Pay with Bitcoin" (D-067): send exactly `amount_sats` (`amount_btc`) to `address`, a deposit
/// address bound to this payment; the merchant receives `amount_out` of the settlement token. `uri`
/// is the BIP21 payment request for the QR. After `expires_at` call
/// [`crate::Checkout::start_bitcoin`] again for a fresh quote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutBitcoin {
    pub address: String,
    pub amount_sats: String,
    pub amount_btc: String,
    pub uri: String,
    pub amount_out: String,
    pub expires_at: String,
    #[serde(default)]
    pub expired: bool,
    /// `waiting` · `depositing` · `pending` · `submitted` · `success` · `failure` · `refund`.
    pub status: String,
    /// Satoshis seen at `address` (mempool included).
    #[serde(default)]
    pub seen_sats: String,
    pub request_id: String,
    pub fee_bps: u32,
    #[serde(default)]
    pub refund_address: Option<String>,
    /// Every deposit quote of this payment.
    #[serde(default)]
    pub quotes: Vec<CheckoutBitcoinQuote>,
}

/// One Bitcoin deposit quote of a [`CheckoutBitcoin`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutBitcoinQuote {
    pub address: String,
    pub amount_sats: String,
    pub status: String,
    pub request_id: String,
    pub created_at: String,
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
    /// Address screening (D-069) of the payer and payout addresses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screening: Option<ScreeningAnnotation>,
}

/// The JSON body LayerHop POSTs to your webhook endpoints, and what
/// [`crate::Webhooks::stream`] yields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookEvent {
    pub id: String,
    /// `order.submitted` · `order.filled` · `order.refunded` · `order.failed` · `order.dropped` ·
    /// `checkout.session.processing` · `checkout.session.completed` ·
    /// `checkout.session.payment_failed` · `checkout.session.expired` ·
    /// `checkout.session.refunded` · `invoice.sent` · `invoice.paid` · `subscription.created` ·
    /// `subscription.renewed` · `subscription.past_due` · `subscription.canceled` ·
    /// `limit_order.opened` · `limit_order.filled` · `limit_order.cancelled` ·
    /// `limit_order.expired` · `limit_order.failed` · `x402.payment.settled` (or `webhook.test`).
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
/// `invoice.*` events (plus the paying `session` for `invoice.paid`), `subscription` for
/// `subscription.*` events (plus `invoice` for `subscription.renewed` / `subscription.past_due`),
/// `limit_order` for `limit_order.*` events (plus `fill`, the fill's order, for every
/// `limit_order.filled`, partial fills included).
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<Subscription>,
    #[serde(default, rename = "limitOrder", skip_serializing_if = "Option::is_none")]
    pub limit_order: Option<LimitOrder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Order>,
    /// `x402.payment.settled`: the settled x402 payment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment: Option<X402Payment>,
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
    /// Owner only: the subscription this invoice bills a period of, if any.
    #[serde(default)]
    pub subscription_id: Option<String>,
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

/// Parameters for [`crate::Subscriptions::create`] (`POST /v1/subscriptions`). Bill either a
/// product (`product_id`) or your own price (`amount`, `settle_chain`, `settle_token`, `recipient`
/// and `description`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubscriptionParams {
    /// Where each period's invoice and the reminders are emailed.
    pub customer_email: String,
    /// `day` · `week` · `month` · `year`.
    pub interval: String,
    /// Bill every `interval_count` intervals (1–365, API default 1; 3 + `month` = quarterly).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_count: Option<u32>,
    /// Bill this product's price, settlement token and payout address (instead of the four fields
    /// below).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    /// Price per period, base units of the settlement token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Settlement chain id or slug: ethereum, arbitrum, base, optimism, bsc, polygon or solana.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settle_chain: Option<String>,
    /// `"USDC"` or `"USDT"` (or its address) on that chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settle_token: Option<String>,
    /// Your payout address on the settlement chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    /// The invoice line item, e.g. `"Pro plan"` (required without `product_id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<String>,
    /// A note on every invoice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// Days after an invoice is issued that it is due (0–60, API default 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_until_due: Option<u32>,
    /// ISO date-time of the first period and invoice (default now, at most 366 days ahead).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_at: Option<String>,
    /// Up to 20 string values, returned in the API and webhooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
}

/// Parameters for [`crate::Subscriptions::update`] (`PATCH /v1/subscriptions/{id}`). Changes apply
/// from the next invoice. Only `Some` fields are sent; `Some(None)` on a nullable field sends
/// `null` to clear it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubscriptionParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_until_due: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Option<BTreeMap<String, String>>>,
    /// `Some(true)` schedules a cancel at the end of the current period; `Some(false)` undoes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_at_period_end: Option<bool>,
}

/// A subscription (`object: "subscription"`): recurring billing as one invoice per period. Crypto
/// can't be pulled from a wallet, so LayerHop issues and emails an [`Invoice`] every period and the
/// customer pays it with any token. Unpaid past the due date → `past_due` with reminders, canceled
/// after 7 days.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub object: String,
    pub livemode: bool,
    /// `active` · `past_due` · `paused` · `canceled`.
    pub status: String,
    pub customer_email: String,
    #[serde(default)]
    pub customer_name: Option<String>,
    #[serde(default)]
    pub merchant_name: Option<String>,
    pub description: String,
    #[serde(default)]
    pub memo: Option<String>,
    #[serde(default)]
    pub product_id: Option<String>,
    /// Price per period, base units of the settlement token.
    pub amount: String,
    pub settlement: CheckoutSettlement,
    /// Your payout address.
    pub recipient: String,
    /// `day` · `week` · `month` · `year`.
    pub interval: String,
    pub interval_count: u32,
    pub days_until_due: u32,
    /// Start of the period last invoiced (`None` before the first invoice).
    #[serde(default)]
    pub current_period_start: Option<String>,
    #[serde(default)]
    pub current_period_end: Option<String>,
    /// When the next invoice is issued (`None` when paused or canceled).
    #[serde(default)]
    pub next_invoice_at: Option<String>,
    /// Scheduled cancel (the end of the current period).
    #[serde(default)]
    pub cancel_at: Option<String>,
    #[serde(default)]
    pub cancel_at_period_end: bool,
    #[serde(default)]
    pub canceled_at: Option<String>,
    /// `requested` (you canceled) or `unpaid` (7 days past due).
    #[serde(default)]
    pub cancellation_reason: Option<String>,
    #[serde(default)]
    pub paused_at: Option<String>,
    /// The invoice of the current period.
    #[serde(default)]
    pub latest_invoice_id: Option<String>,
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, String>>,
    pub created_at: String,
    pub updated_at: String,
}

impl Subscription {
    /// `active`: an invoice is issued every period.
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }
}

/// Parameters for [`crate::Subscriptions::list`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListSubscriptionsParams {
    /// `active` · `past_due` · `paused` · `canceled`.
    pub status: Option<String>,
    pub customer_email: Option<String>,
    pub cursor: Option<String>,
    /// 1–100.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSubscriptionsResponse {
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct SubscriptionInvoicesResponse {
    #[serde(default)]
    pub invoices: Vec<Invoice>,
}

/// Parameters for [`crate::LimitOrders::create`] (`POST /v1/limit-orders`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLimitOrderParams {
    /// The maker wallet (base58): signs the order and receives the output.
    pub wallet: String,
    /// Token to sell: mint, curated symbol (`"USDC"`) or `"SOL"`.
    pub src_token: String,
    /// Token to buy. LayerHop's fee applies when it is SOL, USDC or USDT.
    pub dst_token: String,
    /// Base units of `src_token` to sell (worth at least $5).
    pub amount: String,
    /// Minimum base units of `dst_token` for the whole `amount`: the limit price is
    /// `dst_amount / amount`.
    pub dst_amount: String,
    /// Unix seconds, at least 60 s ahead. `None` for good-till-cancelled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Only `"solana"` (or `"7565164"`) for now, the API default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    /// Defaults to the client's [`crate::LayerHopBuilder::channel`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
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

/// The fee written into an on-chain [`LimitOrder`] (0 when `dst_token` carries no fee).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitOrderFees {
    pub our_fee_bps: u32,
    pub integrator_fee_bps: u32,
    /// The partner id, if a partner fee applies.
    #[serde(default)]
    pub integrator: Option<String>,
}

/// A Solana limit order (`object: "limit_order"`, Jupiter Trigger). The order lives on chain and
/// the wallet signs every transaction; each fill becomes an [`Order`] with `limit_order_id` set
/// (its `quote_id` starts with `lof_`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitOrder {
    pub id: String,
    pub object: String,
    pub livemode: bool,
    /// `built` · `open` · `filled` · `cancelled` · `expired` · `failed`.
    pub status: String,
    #[serde(default)]
    pub status_detail: Option<String>,
    /// `jupiter` (live) or `sandbox` (test mode).
    pub provider: ProviderId,
    /// Always `7565164` (Solana) for now.
    pub chain_id: u64,
    pub wallet: String,
    /// Mint sold.
    pub src_token: String,
    /// Mint bought.
    pub dst_token: String,
    /// Input, base units.
    pub amount: String,
    /// Minimum output for the whole order, base units.
    pub dst_amount: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    pub fees: LimitOrderFees,
    /// The provider's order id (Jupiter: the order account).
    pub provider_order_id: String,
    pub filled_amount: String,
    pub filled_dst_amount: String,
    #[serde(default)]
    pub fill_count: u64,
    /// The transaction that opened the order.
    #[serde(default)]
    pub open_tx_hash: Option<String>,
    /// Set once a cancel tx was submitted; the status turns `cancelled` when the provider confirms.
    #[serde(default)]
    pub cancel_tx_hash: Option<String>,
    pub channel: String,
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, String>>,
    pub created_at: String,
    pub updated_at: String,
    /// The unsigned tx for `wallet` to sign, then [`crate::LimitOrders::submit`]
    /// ([`crate::LimitOrders::create`] only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx: Option<SvmTx>,
    /// The fills, newest first ([`crate::LimitOrders::get`] only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fills: Option<Vec<Order>>,
}

impl LimitOrder {
    /// `filled`, `cancelled`, `expired` or `failed`: the order will not change again.
    pub fn is_final(&self) -> bool {
        matches!(self.status.as_str(), "filled" | "cancelled" | "expired" | "failed")
    }
}

/// Parameters for [`crate::LimitOrders::list`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListLimitOrdersParams {
    /// Orders of this wallet; `None` lists your API key's account (a key is then required).
    pub wallet: Option<String>,
    /// `built` · `open` · `filled` · `cancelled` · `expired` · `failed`.
    pub status: Option<String>,
    pub cursor: Option<String>,
    /// 1–100.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLimitOrdersResponse {
    #[serde(default)]
    pub limit_orders: Vec<LimitOrder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Response of [`crate::LimitOrders::submit`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitLimitOrderResponse {
    pub limit_order: LimitOrder,
    /// The landed transaction's signature.
    pub tx_hash: String,
}

/// Response of [`crate::LimitOrders::cancel`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelLimitOrderResponse {
    pub limit_order: LimitOrder,
    /// The unsigned cancel tx to sign and [`crate::LimitOrders::submit`]; `None` when a `built`
    /// (never submitted) order was cancelled directly.
    #[serde(default)]
    pub tx: Option<SvmTx>,
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

/// One quotable leg, in the same terms as a quote request (chain ids, token addresses, base units).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteLeg {
    pub src_chain: u64,
    pub src_token: String,
    /// Base units of `src_token`.
    pub amount: String,
    pub dst_chain: u64,
    pub dst_token: String,
    /// Estimated output, base units of `dst_token`. Quote the leg again before building it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_amount: Option<String>,
}

/// The hub token between the two steps of a `via_hub` [`RouteSuggestion`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteHub {
    pub chain_id: u64,
    pub token: String,
    pub symbol: String,
    pub decimals: u32,
}

/// An alternative to a route that returned no quote (D-053), from [`crate::Error::suggestions`].
/// The leg fields describe the whole trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteSuggestion {
    /// `dst_token` (another destination token) · `src_token` (pay with another source token) ·
    /// `via_hub` (two signed steps through a hub chain).
    pub kind: String,
    /// Short text for a button, e.g. "Receive USDC on Ethereum instead".
    pub message: String,
    pub src_chain: u64,
    pub src_token: String,
    pub amount: String,
    pub dst_chain: u64,
    pub dst_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_amount: Option<String>,
    /// The token that changed (new source or destination token, or the hub token).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// `via_hub` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hub: Option<RouteHub>,
    /// `via_hub` only: the two legs in order, each its own quote, order and signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<RouteLeg>>,
}

impl RouteSuggestion {
    /// The whole trip as one leg.
    pub fn leg(&self) -> RouteLeg {
        RouteLeg {
            src_chain: self.src_chain,
            src_token: self.src_token.clone(),
            amount: self.amount.clone(),
            dst_chain: self.dst_chain,
            dst_token: self.dst_token.clone(),
            dst_amount: self.dst_amount.clone(),
        }
    }
}

/// At most this many route suggestions per error.
pub const MAX_ROUTE_SUGGESTIONS: usize = 4;

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.len() <= 78 && s.bytes().all(|b| b.is_ascii_digit())
}

fn parse_leg(v: &serde_json::Value) -> Option<RouteLeg> {
    let o = v.as_object()?;
    let amount = o.get("amount")?.as_str()?;
    if !is_digits(amount) || amount.bytes().all(|b| b == b'0') {
        return None;
    }
    let dst_amount = match o.get("dstAmount") {
        None => None,
        Some(d) => Some(d.as_str().filter(|s| is_digits(s))?.to_string()),
    };
    Some(RouteLeg {
        src_chain: o.get("srcChain")?.as_u64()?,
        src_token: o.get("srcToken")?.as_str()?.to_string(),
        amount: amount.to_string(),
        dst_chain: o.get("dstChain")?.as_u64()?,
        dst_token: o.get("dstToken")?.as_str()?.to_string(),
        dst_amount,
    })
}

fn parse_suggestion(v: &serde_json::Value) -> Option<RouteSuggestion> {
    let leg = parse_leg(v)?;
    let kind = v.get("kind")?.as_str()?;
    if !matches!(kind, "dst_token" | "src_token" | "via_hub") {
        return None;
    }
    let message = v.get("message")?.as_str()?.to_string();
    let (hub, steps) = if kind == "via_hub" {
        let steps: Vec<RouteLeg> = v.get("steps")?.as_array()?.iter().map(parse_leg).collect::<Option<_>>()?;
        if steps.len() != 2 {
            return None;
        }
        let h = v.get("hub")?;
        let hub = RouteHub {
            chain_id: h.get("chainId")?.as_u64()?,
            token: h.get("token")?.as_str()?.to_string(),
            symbol: h.get("symbol")?.as_str()?.to_string(),
            decimals: u32::try_from(h.get("decimals")?.as_u64()?).ok()?,
        };
        (Some(hub), Some(steps))
    } else {
        (None, None)
    };
    Some(RouteSuggestion {
        kind: kind.to_string(),
        message,
        src_chain: leg.src_chain,
        src_token: leg.src_token,
        amount: leg.amount,
        dst_chain: leg.dst_chain,
        dst_token: leg.dst_token,
        dst_amount: leg.dst_amount,
        symbol: v.get("symbol").and_then(|s| s.as_str()).map(str::to_string),
        hub,
        steps,
    })
}

/// `details.suggestions` of an API error, validated: malformed entries (and a `via_hub` entry
/// without a valid `hub` and exactly two valid `steps`) are dropped; at most
/// [`MAX_ROUTE_SUGGESTIONS`]. Anything that isn't an array gives an empty list.
pub fn parse_route_suggestions(details: &serde_json::Value) -> Vec<RouteSuggestion> {
    details
        .get("suggestions")
        .and_then(|s| s.as_array())
        .map(|a| a.iter().filter_map(parse_suggestion).take(MAX_ROUTE_SUGGESTIONS).collect())
        .unwrap_or_default()
}

/// `error.details` of `AMOUNT_TOO_SMALL` on a quote (D-036), from [`crate::Error::amount_too_small`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AmountTooSmallDetails {
    /// Smallest input known to quote on this route, base units of `src_token`.
    pub min_amount: Option<String>,
    pub min_amount_usd: Option<f64>,
    /// `true` when found by probing $10 / $25 / $50 (the true minimum is at most this).
    pub min_amount_approx: Option<bool>,
}

/// One share of a [`CreateSplitParams`]: a label, and an amount for custom shares (all or none).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SplitShareInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Base units; the amounts must add up to `total`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
}

/// Parameters for [`crate::Splits::create`] (`POST /v1/splits`). Send `participants` or `shares`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSplitParams {
    /// Address, ENS name or `.sol` name of whoever is owed.
    pub recipient: String,
    /// Chain slug or id they receive on.
    pub chain: String,
    /// Listed token symbol or address on that chain.
    pub token: String,
    /// Base units of `token`.
    pub total: String,
    /// Equal shares for this many people (2–50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participants: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares: Option<Vec<SplitShareInput>>,
    /// Plain text, kept to 140 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Parameters for [`crate::Splits::update`]. Only `Some` fields are sent; `note: Some(None)` sends
/// `null` to clear the note.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSplitParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<Option<String>>,
    /// One label per share.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    /// `open` or `closed` (no more payments).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// The token of a [`Split`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SplitToken {
    pub address: String,
    pub symbol: String,
    pub decimals: u32,
}

/// One share of a [`Split`] and who has paid it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SplitShare {
    /// `split_share` on quotes.
    pub index: u32,
    pub label: String,
    /// Base units.
    pub amount: String,
    /// Base units delivered by filled orders for this share.
    pub received: String,
    /// `unpaid` · `partial` · `paid`.
    pub status: String,
    pub payments: u32,
    pub pending: u32,
}

/// A split bill: `recipient` is owed `total` of `token`, in shares paid by quotes with
/// [`QuoteParams::split_id`] / [`QuoteParams::split_share`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Split {
    pub id: String,
    /// The split's page.
    pub url: String,
    /// `open` · `closed`.
    pub status: String,
    #[serde(default)]
    pub note: Option<String>,
    pub recipient: String,
    #[serde(default)]
    pub recipient_name: Option<String>,
    pub chain_id: u64,
    pub chain: String,
    pub token: SplitToken,
    pub total: String,
    pub collected: String,
    #[serde(default)]
    pub paid_count: u32,
    #[serde(default)]
    pub shares: Vec<SplitShare>,
    pub created_at: String,
    pub updated_at: String,
}

/// Response of [`crate::Splits::create`]: the split plus its one-time edit secret.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitCreated {
    #[serde(flatten)]
    pub split: Split,
    /// The edit secret, shown only once. Keep it to [`crate::Splits::update`] the split.
    pub edit_token: String,
    /// The split's page with the secret in the URL fragment.
    pub edit_url: String,
}

/// A tipper's message in a [`TipJar`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TipJarMessage {
    pub text: String,
    pub at: String,
}

/// Tip jar counters of an address, from [`crate::LayerHop::get_tip_jar`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TipJar {
    /// EVM lowercased.
    pub address: String,
    /// Distinct wallets with a filled tip.
    pub tippers: u64,
    /// Filled tips.
    pub tips: u64,
    /// The latest 10 messages.
    pub messages: Vec<TipJarMessage>,
}

/// One payout row for [`PayoutPlanParams::rows`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PayoutRow {
    /// Address, ENS name or `.sol` name.
    pub recipient: String,
    /// Human units of `token` (`"25"`, `"0.5"`), not base units.
    pub amount: String,
    /// Symbol or address.
    pub token: String,
    /// Chain slug or id.
    pub chain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Parameters for [`crate::Payouts::plan`] (`POST /v1/payouts/plan`). Send `rows` or `csv`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayoutPlanParams {
    /// The chain the payer pays from.
    pub src_chain: String,
    /// The token the payer pays with (listed).
    pub src_token: String,
    /// The payer's address on `src_chain`.
    pub sender: String,
    /// At most 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<Vec<PayoutRow>>,
    /// `recipient,amount,token,chain[,label]` lines, header optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
}

/// A token in a payout plan.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayoutToken {
    pub chain_id: u64,
    pub address: String,
    pub symbol: String,
    pub decimals: u32,
    /// USD price when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd: Option<f64>,
}

/// The payer's side of a [`PayoutPlan`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayoutPlanSource {
    pub chain_id: u64,
    pub chain: String,
    pub sender: String,
    pub token: PayoutToken,
}

/// Why a [`PayoutPlanRow`] is invalid.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PayoutRowError {
    pub code: String,
    pub message: String,
}

/// A resolved payout recipient.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PayoutRecipient {
    pub address: String,
    /// The ENS / `.sol` name it came from.
    #[serde(default)]
    pub name: Option<String>,
}

/// One checked row of a [`PayoutPlan`]: `ok` with the resolved details, or an `error`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PayoutPlanRow {
    /// Row number from 0 (header excluded).
    pub index: u32,
    /// 1-based CSV line (0 for JSON rows).
    pub line: u32,
    /// The row as sent (raw JSON).
    pub input: serde_json::Value,
    pub ok: bool,
    pub error: Option<PayoutRowError>,
    pub recipient: Option<PayoutRecipient>,
    pub chain_id: Option<u64>,
    pub chain: Option<String>,
    pub token: Option<PayoutToken>,
    /// Base units of `token`.
    pub amount: Option<String>,
    /// `transfer` (same chain and token as the payer) or `route` (quote it with `purpose: payout`).
    pub kind: Option<String>,
}

/// A batch of plain transfers in a [`PayoutPlan`]; build it with [`crate::Payouts::transfers`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayoutTransferBatch {
    pub chain_id: u64,
    pub token: String,
    /// Row indices.
    pub rows: Vec<u32>,
    /// Base units of the payer's token.
    pub total: String,
}

/// Routed rows of a [`PayoutPlan`] for one destination chain (one quote and signature per row).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayoutRouteGroup {
    pub chain_id: u64,
    pub rows: Vec<u32>,
}

/// What one destination token adds up to in a [`PayoutPlan`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayoutTotal {
    pub chain_id: u64,
    pub token: String,
    pub symbol: String,
    pub decimals: u32,
    pub amount: String,
    pub formatted: String,
    pub rows: u32,
}

/// Response of [`crate::Payouts::plan`]: every row checked, and the valid ones grouped.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PayoutPlan {
    pub source: PayoutPlanSource,
    pub rows: Vec<PayoutPlanRow>,
    pub transfers: Vec<PayoutTransferBatch>,
    pub routes: Vec<PayoutRouteGroup>,
    /// Base units of the payer's token sent as transfers.
    pub transfer_total: String,
    pub totals: Vec<PayoutTotal>,
    pub valid: u32,
    pub invalid: u32,
    pub warnings: Vec<String>,
}

/// One transfer for [`PayoutTransfersParams`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PayoutTransfer {
    /// An address (resolve names with the plan first).
    pub recipient: String,
    /// Base units.
    pub amount: String,
}

/// Parameters for [`crate::Payouts::transfers`] (`POST /v1/payouts/transfers`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PayoutTransfersParams {
    pub chain: String,
    pub token: String,
    pub sender: String,
    /// At most 20 on EVM chains and 8 on Solana.
    pub transfers: Vec<PayoutTransfer>,
}

/// Unsigned transfers from [`crate::Payouts::transfers`]: EVM `calls` (one per transfer, send them
/// in order) or a Solana `serialized_tx` (base64, the sender's signature slot). The SDK never signs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayoutTransfers {
    /// `evm` or `svm`.
    pub vm: String,
    pub chain_id: u64,
    pub token: PayoutToken,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<Vec<EvmCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serialized_tx: Option<String>,
}

/// Parameters for [`crate::RecurringOrders::create`] (`POST /v1/recurring-orders`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRecurringOrderParams {
    /// The Solana wallet that funds, signs and owns the order.
    pub wallet: String,
    /// Token bought: mint or symbol.
    pub dst_token: String,
    /// The whole budget, base units of `src_token`, deposited when the order opens.
    pub amount: String,
    /// Number of buys (2–1000); each spends `amount / orders` (at least $50).
    pub orders: u32,
    /// Seconds between buys (86400 = daily).
    pub interval_seconds: u64,
    /// Token spent: mint or symbol. Default `USDC`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_token: Option<String>,
    /// Only `solana` (the default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
}

/// Fees of a recurring order: always 0.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecurringOrderFees {
    pub our_fee_bps: u32,
    pub integrator_fee_bps: u32,
}

/// Response of [`crate::RecurringOrders::create`]: sign `tx` with `wallet` (sign only) and
/// [`crate::RecurringOrders::submit`] it with `request_id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringOrderBuild {
    pub object: String,
    pub livemode: bool,
    /// `jupiter` or `sandbox`.
    pub provider: ProviderId,
    pub chain_id: u64,
    pub wallet: String,
    /// Mint spent.
    pub src_token: String,
    /// Mint bought.
    pub dst_token: String,
    pub amount: String,
    pub amount_per_order: String,
    pub orders: u32,
    pub interval_seconds: u64,
    #[serde(default)]
    pub fees: RecurringOrderFees,
    pub request_id: String,
    pub tx: SvmTx,
}

/// One buy of a [`RecurringOrder`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecurringOrderFill {
    pub tx_hash: String,
    pub amount: String,
    pub dst_amount: String,
    /// Jupiter's fee, base units of `fee_token`.
    pub fee_amount: String,
    pub fee_token: Option<String>,
    pub filled_at: Option<String>,
}

/// A Solana recurring order (Jupiter Recurring), read live from Jupiter. Base units throughout.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecurringOrder {
    /// The order account.
    pub id: String,
    pub object: String,
    pub provider: ProviderId,
    pub chain_id: u64,
    /// `active` · `completed` · `cancelled`.
    pub status: String,
    pub wallet: String,
    pub src_token: String,
    pub dst_token: String,
    pub amount: String,
    pub amount_per_order: String,
    pub orders: u32,
    pub orders_filled: u32,
    pub interval_seconds: u64,
    pub spent_amount: String,
    pub withdrawn_amount: String,
    pub received_amount: String,
    pub open_tx_hash: Option<String>,
    pub close_tx_hash: Option<String>,
    pub created_at: Option<String>,
    /// Up to 50 most recent buys.
    pub fills: Vec<RecurringOrderFill>,
}

/// Parameters for [`crate::RecurringOrders::list`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListRecurringOrdersParams {
    pub wallet: String,
    /// `active` (default) or `history`.
    pub status: Option<String>,
    /// From 1.
    pub page: Option<u32>,
}

/// Response of [`crate::RecurringOrders::list`] (10 per page).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ListRecurringOrdersResponse {
    pub recurring_orders: Vec<RecurringOrder>,
    pub page: u32,
    pub total_pages: u32,
}

/// Response of [`crate::RecurringOrders::submit`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitRecurringOrderResponse {
    pub tx_hash: String,
    /// The order account (create); `None` for a cancel.
    #[serde(default)]
    pub order: Option<String>,
}

/// Response of [`crate::RecurringOrders::cancel`]: sign `tx` and submit it with `request_id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRecurringOrderResponse {
    pub request_id: String,
    pub tx: SvmTx,
}

/// Address screening (D-069) on quotes, orders and checkout quotes: which addresses of the
/// request are on the OFAC SDN list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreeningAnnotation {
    /// `ofac_sdn`.
    pub source: String,
    /// Publish date of the list checked.
    #[serde(default)]
    pub list_updated_at: Option<String>,
    /// Empty when no address is listed.
    #[serde(default)]
    pub sanctioned: Vec<SanctionedAddress>,
}

/// One listed address in a [`ScreeningAnnotation`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SanctionedAddress {
    pub address: String,
    /// `sender` · `recipient` · `refundTo` · `payer` · `payout`.
    pub role: String,
    #[serde(default)]
    pub lists: Vec<String>,
}

/// One address's result from [`crate::LayerHop::screen`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreeningResult {
    pub address: String,
    pub sanctioned: bool,
    /// Lists the address is on (`"OFAC SDN"`), empty when not listed.
    #[serde(default)]
    pub lists: Vec<String>,
    /// `ofac_sdn`.
    pub source: String,
    /// Publish date of the list checked; `None` while the API has no list loaded.
    #[serde(default)]
    pub list_updated_at: Option<String>,
}

/// The sanctions list version loaded by the API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreeningList {
    pub source: String,
    pub name: String,
    #[serde(default)]
    pub published_at: Option<String>,
    pub fetched_at: String,
    pub address_count: u64,
}

/// Response of [`crate::LayerHop::screen`] (`POST /v1/screen`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenResponse {
    /// One result per address, in order.
    pub results: Vec<ScreeningResult>,
    /// `None` until the API has downloaded the list (every result is then `sanctioned: false`).
    #[serde(default)]
    pub list: Option<ScreeningList>,
    /// The API refuses listed addresses for everyone (`ADDRESS_SANCTIONED`, HTTP 451).
    pub enforced: bool,
}

/// Result of [`crate::Webhooks::resend`]: the deliveries queued (empty when only open streams get it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResendResult {
    pub event_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub livemode: bool,
    #[serde(default)]
    pub deliveries: Vec<WebhookResendDelivery>,
}

/// One delivery queued by [`crate::Webhooks::resend`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResendDelivery {
    pub id: String,
    pub endpoint_id: String,
}

/// Parameters for [`crate::Usage::get`]. Times are ISO 8601 or unix seconds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageParams {
    /// `hour` or `day`.
    pub bucket: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    /// Only this API key of the account.
    pub key_id: Option<String>,
}

/// Requests per HTTP status class.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StatusClassCounts {
    #[serde(rename = "2xx", default)]
    pub s2xx: u64,
    #[serde(rename = "3xx", default)]
    pub s3xx: u64,
    #[serde(rename = "4xx", default)]
    pub s4xx: u64,
    #[serde(rename = "5xx", default)]
    pub s5xx: u64,
}

/// Totals of a [`UsageReport`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    pub requests: u64,
    pub errors: u64,
    #[serde(default)]
    pub by_status_class: StatusClassCounts,
    #[serde(default)]
    pub avg_ms: Option<f64>,
    #[serde(default)]
    pub p50_ms: Option<f64>,
    #[serde(default)]
    pub p95_ms: Option<f64>,
}

/// One bucket of [`UsageReport::series`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsagePoint {
    pub ts: String,
    pub requests: u64,
    pub errors: u64,
    #[serde(default)]
    pub by_status_class: StatusClassCounts,
    #[serde(default)]
    pub p50_ms: Option<f64>,
    #[serde(default)]
    pub p95_ms: Option<f64>,
}

/// One endpoint group of [`UsageReport::by_endpoint`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEndpoint {
    pub group: String,
    pub requests: u64,
    pub errors: u64,
    #[serde(default)]
    pub by_status_class: StatusClassCounts,
    #[serde(default)]
    pub avg_ms: Option<f64>,
    #[serde(default)]
    pub p50_ms: Option<f64>,
    #[serde(default)]
    pub p95_ms: Option<f64>,
}

/// One error code of [`UsageReport::error_codes`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageErrorCode {
    pub code: String,
    pub count: u64,
    /// Count per endpoint group.
    #[serde(default)]
    pub by_endpoint: BTreeMap<String, u64>,
}

/// `GET /v1/usage`: the key's account and mode. Latency percentiles are estimates (`None` without
/// traffic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageReport {
    /// `hour` or `day`.
    pub bucket: String,
    pub from: String,
    pub to: String,
    pub livemode: bool,
    #[serde(default)]
    pub key_id: Option<String>,
    pub totals: UsageTotals,
    #[serde(default)]
    pub series: Vec<UsagePoint>,
    #[serde(default)]
    pub by_endpoint: Vec<UsageEndpoint>,
    #[serde(default)]
    pub error_codes: Vec<UsageErrorCode>,
}

/// Parameters for [`crate::Usage::errors`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageErrorsParams {
    /// Only this error code (e.g. `RATE_LIMITED`).
    pub code: Option<String>,
    pub key_id: Option<String>,
    pub limit: Option<u32>,
}

/// One failed request from [`crate::Usage::errors`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageError {
    pub ts: String,
    #[serde(default)]
    pub key_id: Option<String>,
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub endpoint_group: String,
    pub status: u16,
    pub code: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UsageErrorsResponse {
    #[serde(default)]
    pub errors: Vec<UsageError>,
}

/// One scheme / network pair of [`X402Supported`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402Kind {
    /// 1 or 2.
    pub x402_version: u32,
    /// `exact`.
    pub scheme: String,
    /// x402 v2 CAIP-2 id (`eip155:8453`) or v1 name (`base`).
    pub network: String,
}

/// Response of [`crate::X402::supported`] (`GET /v1/x402/supported`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402Supported {
    pub kinds: Vec<X402Kind>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub signers: BTreeMap<String, Vec<String>>,
    /// Whether `POST /v1/x402/settle` is enabled.
    pub settle: bool,
}

/// Parameters for [`crate::X402::requirements`] (`POST /v1/x402/requirements`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402RequirementsParams {
    /// `base` (server default), `optimism`, `arbitrum`, `polygon`, or a CAIP-2 id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    /// The price you net per request, USDC base units (`"10000"` = 0.01 USDC).
    pub amount: String,
    /// Your payout address on that network.
    pub pay_to: String,
    /// The resource URL (query ignored for the deposit address) or any stable id.
    pub resource: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// 10–3600, default 60.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_timeout_seconds: Option<u32>,
}

/// Your fee-splitting deposit address for one resource and payout address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402Payee {
    pub id: String,
    pub network: String,
    pub chain_id: u64,
    pub asset: String,
    /// The requirements' `payTo`: it can only pay you and the fee.
    pub deposit_address: String,
    pub merchant: String,
    pub resource: String,
    pub fee_bps: u32,
    pub factory: String,
    #[serde(default)]
    pub last_sweep_tx: Option<String>,
    #[serde(default)]
    pub last_swept_at: Option<String>,
}

/// LayerHop's fee on top of an x402 price.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct X402Fee {
    pub bps: u32,
    pub amount: String,
}

/// Response of [`crate::X402::requirements`]: answer `402 Payment Required` with these.
/// `accepts` is the x402 v2 form, `accepts_v1` the v1 form, `payment_required` the whole v2
/// `PaymentRequired` object (base64 it into the `PAYMENT-REQUIRED` header). Kept as raw JSON so
/// they reach the payer unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402Requirements {
    pub payee: X402Payee,
    /// What you net, USDC base units.
    pub price: String,
    /// What the payer signs (price + fee).
    pub amount: String,
    pub fee: X402Fee,
    pub accepts: Vec<serde_json::Value>,
    #[serde(rename = "acceptsV1")]
    pub accepts_v1: Vec<serde_json::Value>,
    pub payment_required: serde_json::Value,
}

/// Body of [`crate::X402::verify`] and [`crate::X402::settle`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402FacilitatorRequest {
    /// 1 or 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x402_version: Option<u32>,
    /// The decoded `X-PAYMENT` (v1) or `PAYMENT-SIGNATURE` (v2) header.
    pub payment_payload: serde_json::Value,
    /// The requirements you answered the 402 with (v1 or v2 form).
    pub payment_requirements: serde_json::Value,
}

/// Response of [`crate::X402::verify`]. `is_valid: false` carries an x402 `invalid_reason`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402VerifyResponse {
    pub is_valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalid_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Response of [`crate::X402::settle`]. `error_reason: settlement_pending` = sent, receipt not in
/// yet (LayerHop finishes it and `x402.payment.settled` fires then).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402SettleResponse {
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    /// Transaction hash (empty when nothing was sent).
    pub transaction: String,
    pub network: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// An x402 payment settled through LayerHop (`object: "x402.payment"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402Payment {
    pub id: String,
    pub object: String,
    /// `submitted` · `settled` · `failed`.
    pub status: String,
    #[serde(default)]
    pub status_detail: Option<String>,
    pub network: String,
    pub chain_id: u64,
    pub asset: String,
    pub payer: String,
    /// The deposit address paid.
    pub pay_to: String,
    /// What the payer paid, USDC base units.
    pub amount: String,
    /// What you net after the deposit contract's fee.
    pub merchant_amount: String,
    pub fee_bps: u32,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub transaction: Option<String>,
    /// The sweep that paid it out to you.
    #[serde(default)]
    pub sweep_tx: Option<String>,
    #[serde(default)]
    pub settled_at: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Parameters for [`crate::X402Payments::list`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListX402PaymentsParams {
    /// `submitted` · `settled` · `failed`.
    pub status: Option<String>,
    pub cursor: Option<String>,
    /// 1–200, default 50.
    pub limit: Option<u32>,
}

/// Response of [`crate::X402Payments::list`], newest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListX402PaymentsResponse {
    #[serde(default)]
    pub payments: Vec<X402Payment>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}
