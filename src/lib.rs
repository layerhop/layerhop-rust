//! # layerhop
//!
//! Async Rust client for the [LayerHop](https://docs.layerhop.com) API: best-price cross-chain and
//! same-chain swaps through one non-custodial API.
//!
//! The SDK never holds private keys. It returns unsigned transactions; your wallet library
//! (alloy, ethers, solana-sdk, ...) signs and broadcasts them. Amounts are integer strings in base
//! units.
//!
//! ```no_run
//! use layerhop::{LayerHop, QuoteParams};
//!
//! # async fn run() -> layerhop::Result<()> {
//! let lh = LayerHop::new();
//! let q = lh
//!     .quote(&QuoteParams {
//!         src_chain: "base".into(),
//!         src_token: "USDC".into(),
//!         amount: "100000000".into(),
//!         dst_chain: "solana".into(),
//!         dst_token: "SOL".into(),
//!         sender: "0x...".into(),
//!         recipient: "9WzD...".into(),
//!         slippage_bps: None,
//!     })
//!     .await?;
//! let tx = lh.build_tx(&q.best.quote_id).await?;
//! # let _ = tx;
//! # Ok(())
//! # }
//! ```

mod client;
mod error;
mod models;
pub mod webhook;

pub use client::{Checkout, LayerHop, LayerHopBuilder, WatchOptions, DEFAULT_BASE_URL};
pub use error::{Error, Result};
pub use models::{
    Approval, BuiltTx, Chain, CheckoutPayment, CheckoutQuote, CheckoutQuoteParams, CheckoutQuoteResponse,
    CheckoutSession, CheckoutSettlement, CreateCheckoutParams, EvmTx, FeeBreakdown, HealthResponse,
    ListCheckoutSessionsParams, ListCheckoutSessionsResponse, ListOrdersParams, ListOrdersResponse, Order,
    OrderStatus, ProviderError, ProviderId, Quote, QuoteParams, QuoteResponse, ReportSubmittedResponse,
    SimulateError, SimulateResponse, SvmTx, Token,
};

/// LayerHop's internal chain id for Solana (EVM chains use their real chain id).
pub const SOLANA_CHAIN_ID: u64 = 7565164;
/// Native coin address on EVM chains.
pub const EVM_NATIVE: &str = "0x0000000000000000000000000000000000000000";
/// Wrapped-SOL mint, used for native SOL.
pub const SOL_NATIVE: &str = "So11111111111111111111111111111111111111112";
