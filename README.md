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
cargo add futures-util   # for StreamExt in the watch and stream examples
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
| `stream_order(quote_id)` | `GET /v1/orders/{quoteId}/events` (SSE), a `Stream` of `Order` until final |
| `list_orders(&ListOrdersParams)` | `GET /v1/orders?address=&status=&cursor=&limit=` |
| `checkout().create(&CreateCheckoutParams)` | `POST /v1/checkout/sessions` |
| `checkout().list(&ListCheckoutSessionsParams)` | `GET /v1/checkout/sessions?status=&cursor=&limit=` |
| `checkout().get(id)` | `GET /v1/checkout/sessions/{id}` (public) |
| `checkout().expire(id)` | `POST /v1/checkout/sessions/{id}/expire` |
| `checkout().quote(id, &CheckoutQuoteParams)` | `POST /v1/checkout/sessions/{id}/quote` (public) |
| `checkout().stream(id)` | `GET /v1/checkout/sessions/{id}/events` (SSE, public) |
| `webhooks().stream()` | `GET /v1/webhooks/stream` (SSE, API key) |
| `webhooks().trigger(event_type)` | `POST /v1/webhooks/trigger` (test keys only) |

Builder options: `base_url`, `api_key` (sent as `x-api-key`), `channel` (`api` by default),
`integrator` (your partner id), `integrator_fee_bps`, `http_client` (your own `reqwest::Client`).

## Live order status

`stream_order` follows an order over Server-Sent Events instead of polling: a `Stream` that yields
the order now and on every change, and ends once it is final (`filled`, `refunded`, `failed` or
`expired`). `checkout().stream(id)` does the same for a checkout session (until `paid`, `expired`
or `needs_review`).

```rust
use futures_util::StreamExt;

let mut orders = Box::pin(lh.stream_order(&quote_id));
while let Some(order) = orders.next().await {
    let order = order?; // Error::Api (ORDER_NOT_FOUND), Error::Http or Error::Decode, then the stream ends
    println!("{:?} {:?}", order.status, order.dst_tx_hash);
}
```

The request is sent on the first poll; drop the stream to close the connection. A client-wide
`reqwest` timeout also cuts streams, so leave it unset on a client used for them. Guide:
https://docs.layerhop.com/guides/live-status

## Checkout (pay with any token)

Get paid in USDC or USDT on the chain you choose while your customer pays with any token on any
chain. Create a session with your API key and send the customer to its hosted `url`. The session
becomes `paid` (and a `checkout.session.completed` webhook fires) once the delivery is verified on
chain.

```rust
use layerhop::{CheckoutQuoteParams, CreateCheckoutParams};

let session = lh
    .checkout()
    .create(&CreateCheckoutParams {
        amount: "49000000".into(), // 49 USDC (6 decimals)
        settle_chain: "base".into(),
        settle_token: "USDC".into(),
        recipient: "0xYourPayoutAddress".into(),
        description: Some("Pro plan, 1 month".into()),
        reference: Some("order_1042".into()),
        success_url: Some("https://shop.example.com/thanks".into()),
        ..Default::default()
    })
    .await?;
println!("{}", session.url); // redirect the customer here

let s = lh.checkout().get(&session.id).await?; // s.status: open | processing | paid | expired | needs_review
```

To build your own payment page, `get` and `quote` are public (no key needed). Pay the chosen quote
with `build_tx` and `report_submitted` as usual:

```rust
let q = lh
    .checkout()
    .quote(&session_id, &CheckoutQuoteParams {
        src_chain: "arbitrum".into(),
        src_token: "ETH".into(),
        sender: payer_address.into(),
        slippage_bps: None,
    })
    .await?;
// q.best.src_amount is what the payer sends; q.best.dst_amount_min is at least the amount due.
let tx = lh.build_tx(&q.best.quote_id).await?;
```

`checkout().list(..)` pages through your sessions and `checkout().expire(id)` cancels an open one.
See the [checkout guide](https://docs.layerhop.com/guides/checkout).

## Test mode

Use an `lh_test_` key exactly like a live key. Every request is served by the `sandbox` provider,
responses carry `livemode: false` (`QuoteResponse::livemode`, `Order::livemode`), and built
transactions have `sandbox: Some(true)`. Never broadcast them; report any hash with
`report_submitted`. The last digits of `amount` choose the outcome: `…4001` refunds, `…4002` fails
and `…4004` returns `NO_ROUTE`. Checkout sessions created with a test key (`cs_test_…`) are paid
through the sandbox too. See the [test mode guide](https://docs.layerhop.com/guides/test-mode).

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

### Webhook tools

With an API key, `webhooks().stream()` yields your account's webhook events (in the key's live or
test mode) as `WebhookEvent`s as they are created. `webhooks().trigger(event_type)` creates a
sample event in test mode (test keys only; a live key returns `Error::Api` with `FORBIDDEN`).

```rust
let lh = LayerHop::builder().api_key("lh_test_...").build();
let r = lh.webhooks().trigger("order.filled").await?; // r.event_id, r.deliveries, r.payload
let mut events = Box::pin(lh.webhooks().stream());
while let Some(event) = events.next().await {
    let event = event?;
    println!("{} {:?}", event.event_type, event.data.order);
}
```

## Development

```bash
cargo test
```

Tests use [`wiremock`](https://docs.rs/wiremock) and never touch the network.

## License

MIT
