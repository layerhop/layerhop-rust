use std::collections::BTreeMap;

use layerhop::{
    BuiltTx, CheckoutQuoteParams, CreateCheckoutParams, Error, LayerHop, ListCheckoutSessionsParams,
    ListOrdersParams, QuoteParams,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CS: &str = "cs_01k7t9m2c4x8q6r0v3b5n7p9d";

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).api_key("lh_test_k").build()
}

fn session_json() -> serde_json::Value {
    json!({
        "id": CS, "object": "checkout.session", "livemode": true, "status": "processing", "statusDetail": null,
        "amount": "49000000",
        "settlement": { "chainId": 8453, "token": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "symbol": "USDC", "decimals": 6 },
        "recipient": "0x2222222222222222222222222222222222222222", "merchantName": "Acme",
        "description": "Pro plan, 1 month", "url": format!("https://layerhop.com/pay/{CS}"),
        "successUrl": "https://shop.example.com/thanks", "cancelUrl": null, "expiresAt": "2026-10-18T11:00:00.000Z",
        "payment": {
            "quoteId": "q_9", "payer": "0x1111111111111111111111111111111111111111", "srcChainId": 42161,
            "srcToken": "0x0000000000000000000000000000000000000000", "srcAmount": "15623001000000000",
            "srcTxHash": "0xabc", "dstTxHash": null, "paidAt": null
        },
        "reference": "order_1042", "metadata": { "plan": "pro" },
        "createdAt": "2026-10-18T10:00:00.000Z", "updatedAt": "2026-10-18T10:02:00.000Z",
        "someFutureField": 1
    })
}

fn checkout_quote_json() -> serde_json::Value {
    let q = json!({
        "quoteId": "q_1", "provider": "sandbox", "srcAmount": "15623001000000000", "dstAmount": "49100000",
        "dstAmountMin": "49000000", "estimatedSeconds": 8, "fees": { "ourFeeBps": 20 }, "expiresAt": 1760000030u64
    });
    json!({
        "sessionId": CS, "livemode": false, "amountDue": "49000000", "best": q.clone(), "quotes": [q],
        "errors": [{ "provider": "lifi", "code": "PROVIDER_TIMEOUT", "message": "Timed out" }]
    })
}

#[tokio::test]
async fn create_sends_body_and_parses_session() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .and(header("x-api-key", "lh_test_k"))
        .and(body_json(json!({
            "amount": "49000000", "settleChain": "base", "settleToken": "USDC",
            "recipient": "0x2222222222222222222222222222222222222222", "description": "Pro plan, 1 month",
            "reference": "order_1042", "metadata": { "plan": "pro" },
            "successUrl": "https://shop.example.com/thanks", "expiresInMinutes": 30
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(session_json()))
        .expect(1)
        .mount(&server)
        .await;

    let s = client(&server)
        .checkout()
        .create(&CreateCheckoutParams {
            amount: "49000000".into(),
            settle_chain: "base".into(),
            settle_token: "USDC".into(),
            recipient: "0x2222222222222222222222222222222222222222".into(),
            description: Some("Pro plan, 1 month".into()),
            reference: Some("order_1042".into()),
            metadata: Some(BTreeMap::from([("plan".to_string(), "pro".to_string())])),
            success_url: Some("https://shop.example.com/thanks".into()),
            expires_in_minutes: Some(30),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(s.id, CS);
    assert_eq!(s.object, "checkout.session");
    assert!(s.livemode);
    assert_eq!(s.status, "processing");
    assert!(!s.is_paid());
    assert_eq!(s.status_detail, None);
    assert_eq!(s.settlement.chain_id, 8453);
    assert_eq!(s.settlement.decimals, 6);
    assert_eq!(s.url, format!("https://layerhop.com/pay/{CS}"));
    assert_eq!(s.cancel_url, None);
    assert_eq!(s.metadata.unwrap()["plan"], "pro");
    let p = s.payment.unwrap();
    assert_eq!(p.src_chain_id, Some(42161));
    assert_eq!(p.src_amount.as_deref(), Some("15623001000000000"));
    assert_eq!(p.dst_tx_hash, None);
    assert_eq!(p.paid_at, None);
}

#[tokio::test]
async fn get_list_expire() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/checkout/sessions/{CS}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": CS, "object": "checkout.session", "livemode": false, "status": "paid", "amount": "1",
            "settlement": { "chainId": 7565164, "token": "EPjF", "symbol": "USDC", "decimals": 6 },
            "recipient": "9W", "url": "u", "expiresAt": "e", "payment": null, "createdAt": "c", "updatedAt": "u"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/checkout/sessions"))
        .and(query_param("status", "paid"))
        .and(query_param("cursor", "c1"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "sessions": [session_json()], "nextCursor": "c2" })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/checkout/sessions"))
        .and(query_param_is_missing("status"))
        .and(query_param_is_missing("limit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "sessions": [], "nextCursor": null })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions/a%2Fb/expire"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut s = session_json();
            s["status"] = json!("expired");
            s["payment"] = json!(null);
            s
        }))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let s = lh.checkout().get(CS).await.unwrap();
    assert!(s.is_paid());
    assert!(!s.livemode);
    assert!(s.payment.is_none());
    assert!(s.reference.is_none());

    let page = lh
        .checkout()
        .list(&ListCheckoutSessionsParams {
            status: Some("paid".into()),
            cursor: Some("c1".into()),
            limit: Some(10),
        })
        .await
        .unwrap();
    assert_eq!(page.sessions[0].id, CS);
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));
    let empty = lh.checkout().list(&ListCheckoutSessionsParams::default()).await.unwrap();
    assert!(empty.sessions.is_empty());
    assert_eq!(empty.next_cursor, None);

    let e = lh.checkout().expire("a/b").await.unwrap();
    assert_eq!(e.status, "expired");
    assert!(e.payment.is_none());
}

#[tokio::test]
async fn quote_sends_body_and_parses() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/checkout/sessions/{CS}/quote")))
        .and(body_json(json!({ "srcChain": "arbitrum", "srcToken": "ETH", "sender": "0x11" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(checkout_quote_json()))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/checkout/sessions/{CS}/quote")))
        .and(body_json(json!({ "srcChain": "solana", "srcToken": "SOL", "sender": "9W", "slippageBps": 100 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(checkout_quote_json()))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let r = lh
        .checkout()
        .quote(
            CS,
            &CheckoutQuoteParams {
                src_chain: "arbitrum".into(),
                src_token: "ETH".into(),
                sender: "0x11".into(),
                slippage_bps: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(r.session_id, CS);
    assert!(!r.livemode);
    assert_eq!(r.amount_due, "49000000");
    assert_eq!(r.best.quote_id, "q_1");
    assert_eq!(r.best.provider, "sandbox");
    assert_eq!(r.best.src_amount, "15623001000000000");
    assert_eq!(r.best.dst_amount_min, "49000000");
    assert_eq!(r.best.fees.our_fee_bps, 20);
    assert_eq!(r.quotes.len(), 1);
    assert_eq!(r.errors[0].code, "PROVIDER_TIMEOUT");

    lh.checkout()
        .quote(
            CS,
            &CheckoutQuoteParams {
                src_chain: "solana".into(),
                src_token: "SOL".into(),
                sender: "9W".into(),
                slippage_bps: Some(100),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn checkout_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/checkout/sessions/{CS}/expire")))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({ "error": {
            "code": "CONFLICT", "message": "Session is paid", "requestId": "r1" } })))
        .mount(&server)
        .await;
    match client(&server).checkout().expire(CS).await.unwrap_err() {
        Error::Api { code, status, request_id, .. } => {
            assert_eq!(code, "CONFLICT");
            assert_eq!(status, 409);
            assert_eq!(request_id.as_deref(), Some("r1"));
        }
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn test_mode_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "best": { "quoteId": "q_1", "provider": "sandbox", "dstAmount": "1", "dstAmountMin": "1",
                      "estimatedSeconds": 1, "fees": { "ourFeeBps": 20 }, "expiresAt": 1 },
            "quotes": [], "errors": [], "livemode": false
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tx": { "vm": "svm", "serializedTx": "AQID", "sandbox": true }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "orders": [{ "quoteId": "q_1", "livemode": false, "checkoutSessionId": CS }], "nextCursor": null
        })))
        .mount(&server)
        .await;

    let lh = client(&server);
    let q = lh
        .quote(&QuoteParams {
            src_chain: "base".into(),
            src_token: "USDC".into(),
            amount: "100004001".into(),
            dst_chain: "solana".into(),
            dst_token: "SOL".into(),
            sender: "a".into(),
            recipient: "b".into(),
            slippage_bps: None,
        })
        .await
        .unwrap();
    assert_eq!(q.livemode, Some(false));
    assert_eq!(q.best.provider, "sandbox");
    match lh.build_tx("q_1").await.unwrap() {
        BuiltTx::Svm(tx) => assert_eq!(tx.sandbox, Some(true)),
        other => panic!("expected svm, got {other:?}"),
    }
    let o = lh.list_orders(&ListOrdersParams::default()).await.unwrap();
    assert_eq!(o.orders[0].livemode, Some(false));
    assert_eq!(o.orders[0].checkout_session_id.as_deref(), Some(CS));
}
