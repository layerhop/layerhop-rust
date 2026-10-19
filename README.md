# layerhop (Rust)

[![crates.io](https://img.shields.io/crates/v/layerhop?color=c3f06b&labelColor=090d15&cacheSeconds=3600)](https://crates.io/crates/layerhop) [![docs.rs](https://img.shields.io/docsrs/layerhop?labelColor=090d15)](https://docs.rs/layerhop) [![license](https://img.shields.io/crates/l/layerhop?color=7ddec8&labelColor=090d15)](https://opensource.org/license/mit)

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
| `checkout().products().create(&CreateCheckoutProductParams)` | `POST /v1/checkout/products` |
| `checkout().products().list(&ListCheckoutProductsParams)` | `GET /v1/checkout/products?active=&cursor=&limit=` |
| `checkout().products().get(id)` | `GET /v1/checkout/products/{id}` (public) |
| `checkout().products().update(id, &UpdateCheckoutProductParams)` | `PATCH /v1/checkout/products/{id}` (only `Some` fields) |
| `checkout().products().start_session(id, &StartProductCheckoutParams)` | `POST /v1/checkout/products/{id}/sessions` (public) |
| `checkout().refunds().create(session_id, &CreateRefundParams)` | `POST /v1/checkout/sessions/{id}/refunds` |
| `checkout().refunds().list(session_id)` | `GET /v1/checkout/sessions/{id}/refunds` |
| `invoices().create(&CreateInvoiceParams)` | `POST /v1/invoices` |
| `invoices().list(&ListInvoicesParams)` | `GET /v1/invoices?status=&cursor=&limit=` |
| `invoices().get(id)` | `GET /v1/invoices/{id}` |
| `invoices().update(id, &UpdateInvoiceParams)` | `PATCH /v1/invoices/{id}` (drafts only, only `Some` fields) |
| `invoices().send(id)` | `POST /v1/invoices/{id}/send` |
| `invoices().void(id)` | `POST /v1/invoices/{id}/void` |
| `invoices().public_view(id)` | `GET /v1/invoices/{id}/public` (public) |
| `invoices().pay(id)` | `POST /v1/invoices/{id}/pay` (public) |
| `deposit_addresses().create(&CreateDepositAddressParams)` | `POST /v1/deposit-addresses` |
| `deposit_addresses().list(&ListDepositAddressesParams)` | `GET /v1/deposit-addresses?status=&label=&cursor=&limit=` |
| `deposit_addresses().get(id)` | `GET /v1/deposit-addresses/{id}` (with its recent `deposits`) |
| `deposit_addresses().refresh(id)` | `POST /v1/deposit-addresses/{id}/refresh` |
| `deposit_addresses().disable(id)` | `POST /v1/deposit-addresses/{id}/disable` |
| `deposit_addresses().simulate(id, amount)` | `POST /v1/deposit-addresses/{id}/simulate` (test keys only) |
| `service_status()` | `GET /v1/status` (public) |
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

## Products and payment buttons

A product is a fixed price (amount, settlement token, payout address and redirect URLs) that
anyone may start a session from without being able to change it: the backing for payment buttons,
`https://layerhop.com/buy/{id}` links and store plugins. `create`, `list` and `update` need your
API key; `get` and `start_session` are public.

```rust
use layerhop::{CreateCheckoutProductParams, StartProductCheckoutParams, UpdateCheckoutProductParams};

let product = lh
    .checkout()
    .products()
    .create(&CreateCheckoutProductParams {
        name: "Pro plan".into(),
        description: Some("1 month".into()),
        amount: "49000000".into(), // 49 USDC
        settle_chain: "base".into(),
        settle_token: "USDC".into(),
        recipient: "0xYourPayoutAddress".into(),
        success_url: Some("https://shop.example.com/thanks".into()),
        allowed_origins: Some(vec!["https://shop.example.com".into()]),
        ..Default::default()
    })
    .await?;
println!("{}", product.url); // https://layerhop.com/buy/prod_...: share it or put it behind a button

// Only `Some` fields change; `Some(None)` clears a nullable field such as `description`.
lh.checkout()
    .products()
    .update(&product.id, &UpdateCheckoutProductParams { active: Some(false), ..Default::default() })
    .await?; // stops new sessions

// Public (no key): start a session on the product's terms, adding only your order id.
let session = LayerHop::new()
    .checkout()
    .products()
    .start_session(&product.id, &StartProductCheckoutParams { reference: Some("order_1042".into()), ..Default::default() })
    .await?;
println!("{} {:?}", session.url, session.product_id);
```

See the [payment buttons guide](https://docs.layerhop.com/guides/payment-buttons).

## Invoices

Bill a customer by email: line items, a due date and a memo, paid through checkout with any
token. `send` (or `send: Some(true)` on create) finalizes the draft and emails the hosted page
`https://layerhop.com/invoice/{id}`. Once paid, the invoice is `paid`, the customer gets a receipt
with a PDF link (`receipt_url`) and an `invoice.paid` webhook fires.

```rust
use layerhop::{CreateInvoiceParams, InvoiceLineItem, UpdateInvoiceParams};

let inv = lh
    .invoices()
    .create(&CreateInvoiceParams {
        customer_email: Some("jane@example.com".into()),
        customer_name: Some("Jane Doe".into()),
        line_items: vec![InvoiceLineItem {
            description: "Design work".into(),
            quantity: 2,
            unit_amount: "100000000".into(), // 100 USDC each
        }],
        settle_chain: "base".into(),
        settle_token: "USDC".into(),
        recipient: "0xYourPayoutAddress".into(),
        due_at: Some("2026-11-01".into()),
        send: Some(true), // email it now; omit to keep a draft
        ..Default::default()
    })
    .await?;
println!("{} {} {:?}", inv.number, inv.status, inv.url); // INV-0001 open Some("https://layerhop.com/invoice/inv_...")

// Drafts can be edited (`Some(None)` clears a field); open ones can be re-sent or voided.
lh.invoices()
    .update(&draft_id, &UpdateInvoiceParams { memo: Some(Some("Net 14".into())), ..Default::default() })
    .await?;
lh.invoices().send(&draft_id).await?;
lh.invoices().void(&inv.id).await?;

// Public (no key): the customer's side.
let pay = LayerHop::new().invoices().pay(&inv.id).await?; // send the payer to pay.url
```

Statuses: `draft` → `open` → `paid`, or `void`. See the
[invoices guide](https://docs.layerhop.com/guides/invoices).

## Deposit addresses

Let a user swap or bridge by just **sending** tokens to an address: no wallet connection, no
signing in your app. The provider (Relay) owns the address and delivers `dst_token` to `recipient`;
LayerHop never holds the funds. Each detected deposit becomes an order with `deposit_address_id`
set and the usual `order.*` webhooks. Everything needs an API key.

```rust
use layerhop::{CreateDepositAddressParams, ListDepositAddressesParams};

let da = lh
    .deposit_addresses()
    .create(&CreateDepositAddressParams {
        src_chain: "base".into(),
        src_token: "USDC".into(),
        dst_chain: "arbitrum".into(),
        dst_token: "USDC".into(),
        recipient: "0xRecipientOnArbitrum".into(),
        refund_to: "0xUserRefundAddressOnBase".into(), // never a CEX deposit address
        label: Some("user_42".into()),
        // reusable: None = reusable (any amount); Some(false) + amount = one-time address
        ..Default::default()
    })
    .await?;
println!("{} {:?}", da.address, da.estimate); // show the address to your user

lh.deposit_addresses().refresh(&da.id).await?; // "I sent it": check for deposits now
let got = lh.deposit_addresses().get(&da.id).await?; // got.deposits: its 20 most recent orders
lh.deposit_addresses()
    .list(&ListDepositAddressesParams { label: Some("user_42".into()), ..Default::default() })
    .await?;
lh.deposit_addresses().disable(&da.id).await?;
```

Live mode may be disabled by the operator: a live key then gets an `Error::Api` with code
`FORBIDDEN`. Test keys always work, and `lh.deposit_addresses().simulate(&da.id, "25000000")`
records a test deposit that settles like any test order. Deposits under $1 may not be processed.

## Refunds

Refunds are non-custodial: you sign them from your own wallet (by default the session's payout
address). LayerHop routes the settlement token back to the payer's original chain and token
(`to: "original"`, the default) or sends the settlement token itself (`to: "settlement"`). Leave
`amount` unset to refund everything not yet refunded.

```rust
use layerhop::CreateRefundParams;

let r = lh
    .checkout()
    .refunds()
    .create(&session_id, &CreateRefundParams {
        amount: Some("10000000".into()), // 10 USDC of the 49 paid
        reason: Some("Damaged item".into()),
        ..Default::default()
    })
    .await?;
// 1. build, 2. sign from r.quote.from with your wallet, 3. report the hash
let tx = lh.build_tx(&r.quote.quote_id).await?;
// ... sign and broadcast tx ...
lh.report_submitted(&r.quote.quote_id, &tx_hash).await?;

let refunds = lh.checkout().refunds().list(&session_id).await?; // pending → submitted → completed | failed
```

When the refund order fills, the refund is `completed`, the session's `refunded_amount` grows and
a `checkout.session.refunded` webhook fires (with `data.session` and `data.refund`). See the
[refunds guide](https://docs.layerhop.com/guides/refunds).

## Service status

`service_status()` returns the data behind [status.layerhop.com](https://status.layerhop.com): the
overall `status` (`operational`, `degraded` or `outage`), every component with its 90-day uptime,
and incidents from the last 14 days. It is public. (`status(provider, id)` tracks one order.)

```rust
let st = lh.service_status().await?;
for c in &st.components {
    println!("{} {}", c.name, c.status);
}
```

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
