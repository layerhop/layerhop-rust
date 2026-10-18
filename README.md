# layerhop (Rust)

[![crates.io](https://img.shields.io/crates/v/layerhop?color=c3f06b&labelColor=090d15)](https://crates.io/crates/layerhop) [![docs.rs](https://img.shields.io/docsrs/layerhop?labelColor=090d15)](https://docs.rs/layerhop) [![license](https://img.shields.io/crates/l/layerhop?color=7ddec8&labelColor=090d15)](https://opensource.org/license/mit)

Async Rust client for the [LayerHop](https://docs.layerhop.com) API: best-price cross-chain and
same-chain swaps (EVM ↔ Solana) through one non-custodial API. It mirrors the TypeScript SDK
(`@layerhop/sdk`).

- Built on `reqwest` (rustls, no OpenSSL) and `tokio`. Models use `serde`.
- **Amounts are integer strings in base units** (`"100000000"` = 100 USDC). Never floats.
- **The SDK never holds private keys.** It returns unsigned transactions. Your own wallet library
  (alloy, ethers-rs, solana-sdk, a KMS, a hardware wallet...) signs and broadcasts them.

## Install

```bash
cargo add layerhop
cargo add tokio --features macros,rt-multi-thread
cargo add futures-util   # for StreamExt in the watch example
```


## Quickstart

Quote → build → sign with your wallet → report the tx hash → watch until it is final.

```rust
use futures_util::StreamExt;
use layerhop::{BuiltTx, LayerHop, QuoteParams, WatchOptions};

#[tokio::main]
async fn main() -> layerhop::Result<()> {
    let lh = LayerHop::builder()
        .api_key(std::env::var("LAYERHOP_API_KEY").unwrap_or_default())
        .build(); // https://api.layerhop.com; use .base_url("http://localhost:8787") locally

    // 1. Quote: 100 USDC on Base -> SOL on Solana.
    let quotes = lh
        .quote(&QuoteParams {
            src_chain: "base".into(),
            src_token: "USDC".into(),
            amount: "100000000".into(), // base units (6 decimals)
            dst_chain: "solana".into(),
            dst_token: "SOL".into(),
            sender: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".into(),
            recipient: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".into(),
            slippage_bps: Some(50),
        })
        .await?;
    let best = &quotes.best;
    println!("{} gives {} lamports via {}", best.quote_id, best.dst_amount, best.provider);

    // 2. Build the transaction (optionally dry-run it first with lh.simulate(&best.quote_id)).
    let tx = lh.build_tx(&best.quote_id).await?;

    // 3. Sign and broadcast with YOUR wallet library. LayerHop never sees your key.
    let tx_hash: String = match tx {
        BuiltTx::Evm(evm) => {
            // If evm.approval is Some, send ERC-20 approve(spender, amount) first and wait for it.
            // Then send { to: evm.to, data: evm.data, value: evm.value } on evm.chain_id
            // with alloy / ethers-rs and return the tx hash.
            todo!("sign {evm:?} with your EVM wallet")
        }
        BuiltTx::Svm(svm) => {
            // Deserialize svm.serialized_tx (base64 VersionedTransaction) with solana-sdk and sign it.
            // If svm.submit_via_api == Some(true): only sign, then
            //   lh.submit(&best.quote_id, &signed_base64).await?
            // Otherwise send it through your RPC.
            todo!("sign {svm:?} with your Solana wallet")
        }
    };

    // 4. Tell LayerHop you broadcast it (enables order tracking and webhooks). Idempotent.
    lh.report_submitted(&best.quote_id, &tx_hash).await?;

    // 5. Watch until filled / refunded / failed.
    let mut updates = Box::pin(lh.watch(&best.provider, &tx_hash, WatchOptions::default()));
    while let Some(status) = updates.next().await {
        println!("{}", status?.state);
    }
    Ok(())
}
```

## API

| Method | Endpoint |
|---|---|
| `health()` | `GET /v1/health` |
| `chains()` | `GET /v1/chains` |
| `tokens(chain)` | `GET /v1/tokens?chain=` |
| `quote(&QuoteParams)` | `GET /v1/quote` |
| `build_tx(quote_id)` | `POST /v1/orders` |
| `submit(quote_id, signed_base64)` | `POST /v1/orders/submit` (Solana `submit_via_api` only) |
| `report_submitted(quote_id, tx_hash)` | `POST /v1/orders/{quoteId}/submitted` |
| `simulate(quote_id)` | `POST /v1/simulate` |
| `status(provider, id)` | `GET /v1/orders/{provider}/{id}` |
| `watch(provider, id, WatchOptions)` | polls `status` and yields a `Stream` of state changes |
| `list_orders(&ListOrdersParams)` | `GET /v1/orders?address=&status=&cursor=&limit=` |

Builder options: `base_url`, `api_key` (sent as `x-api-key`), `channel` (`api` by default),
`integrator` (your partner id), `integrator_fee_bps`, `http_client` (your own `reqwest::Client`).

## Errors

Every non-2xx response becomes `layerhop::Error::Api { code, message, status, request_id }`, where
`code` is a stable error code (`QUOTE_EXPIRED`, `NO_ROUTE`, ...) documented at
`https://docs.layerhop.com/errors/{code}`. `err.code()` returns it. Transport failures are
`Error::Http`, unexpected bodies `Error::Decode`, and `watch` timeouts `Error::Timeout`.

```rust
match lh.build_tx(&quote_id).await {
    Err(e) if e.code() == Some("QUOTE_EXPIRED") => { /* re-quote */ }
    other => { other?; }
}
```

## Webhooks

Verify the `LayerHop-Signature` header (`t=<unix>,v1=<hex hmac_sha256(secret, "<t>.<raw body>")>`)
against the **raw** request body. Deliveries older than 300 s are rejected.

```rust
use layerhop::webhook;

webhook::verify(&secret, &raw_body, signature_header)?; // default 300 s tolerance
// or: webhook::verify_signature(&secret, &raw_body, header, tolerance, std::time::SystemTime::now())
```

## Development

```bash
cargo test
```

Tests use [`wiremock`](https://docs.rs/wiremock) and never touch the network.

## License

MIT
