//! x402, usage, webhook resend / replay, live quote stream, address screening, checkout transfer /
//! Bitcoin fields and exact-output quotes.

use futures_util::StreamExt;
use layerhop::{
    CreateCheckoutParams, Error, ExactOutputQuoteParams, LayerHop, ListX402PaymentsParams, QuoteParams,
    QuoteStreamOptions, UsageErrorsParams, UsageParams, X402FacilitatorRequest, X402RequirementsParams,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CS: &str = "cs_01k7t9m2c4x8q6r0v3b5n7p9d";
const SENDER: &str = "0x1111111111111111111111111111111111111111";
const PAYOUT: &str = "0x2222222222222222222222222222222222222222";

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).api_key("lh_live_k").build()
}

fn sse(body: String) -> ResponseTemplate {
    ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(body)
}

fn quote_json(id: &str) -> serde_json::Value {
    json!({ "quoteId": id, "provider": "lifi", "dstAmount": "10000000", "dstAmountMin": "10000000",
            "estimatedSeconds": 20, "fees": { "ourFeeBps": 25 }, "expiresAt": 1760000030u64,
            "srcAmount": "10048000", "srcAmountMax": "10098240" })
}

fn screening_json() -> serde_json::Value {
    json!({ "source": "ofac_sdn", "listUpdatedAt": "2026-10-28T00:00:00.000Z",
            "sanctioned": [{ "address": "0x098B716B8Aaf21512996dC57EB0615e2383E2f96", "role": "recipient", "lists": ["OFAC SDN"] }] })
}

fn payment_json() -> serde_json::Value {
    json!({ "id": "x402_01k7t9m2c4x8q6r0v3b5n7p9d", "object": "x402.payment", "status": "settled", "statusDetail": null,
            "network": "eip155:8453", "chainId": 8453, "asset": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            "payer": SENDER, "payTo": "0x3333333333333333333333333333333333333333", "amount": "10025",
            "merchantAmount": "10000", "feeBps": 25, "resource": "https://api.example.com/v1/weather",
            "transaction": "0xabc", "sweepTx": null, "settledAt": "2026-11-04T10:00:20.000Z",
            "createdAt": "2026-11-04T10:00:00.000Z", "updatedAt": "2026-11-04T10:00:20.000Z" })
}

#[tokio::test]
async fn x402_endpoints() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/x402/supported"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "kinds": [{ "x402Version": 2, "scheme": "exact", "network": "eip155:8453" }, { "x402Version": 1, "scheme": "exact", "network": "base" }],
            "extensions": [], "signers": { "eip155:8453": ["0x44"] }, "settle": false
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/x402/requirements"))
        .and(header("x-api-key", "lh_live_k"))
        .and(body_json(json!({ "network": "base", "amount": "10000", "payTo": PAYOUT, "resource": "https://api.example.com/v1/weather", "description": "Current weather" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "payee": { "id": "x4p_1", "network": "base", "chainId": 8453, "asset": "0x83", "depositAddress": "0x33",
                       "merchant": PAYOUT, "resource": "https://api.example.com/v1/weather", "feeBps": 25, "factory": "0x55",
                       "lastSweepTx": null, "lastSweptAt": null },
            "price": "10000", "amount": "10025", "fee": { "bps": 25, "amount": "25" },
            "accepts": [{ "scheme": "exact", "network": "eip155:8453", "amount": "10025" }],
            "acceptsV1": [{ "scheme": "exact", "network": "base", "maxAmountRequired": "10025" }],
            "paymentRequired": { "x402Version": 2 }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let fac = json!({ "x402Version": 2, "paymentPayload": { "payload": { "signature": "0xsig" } }, "paymentRequirements": { "scheme": "exact" } });
    Mock::given(method("POST"))
        .and(path("/v1/x402/verify"))
        .and(body_json(fac.clone()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "isValid": false, "invalidReason": "insufficient_funds", "payer": SENDER })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/x402/settle"))
        .and(body_json(fac))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": false, "errorReason": "settlement_pending", "payer": SENDER, "transaction": "0xabc", "network": "eip155:8453" })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/x402/payments"))
        .and(query_param("status", "settled"))
        .and(query_param("limit", "10"))
        .and(query_param_is_missing("cursor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "payments": [payment_json()], "nextCursor": "c2" })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/x402/payments/x402_01k7t9m2c4x8q6r0v3b5n7p9d"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payment_json()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/x402/payments/x402_missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "error": { "code": "NOT_FOUND", "message": "Payment not found" } })))
        .mount(&server)
        .await;

    let lh = client(&server);
    let x = lh.x402();
    let s = x.supported().await.unwrap();
    assert_eq!(s.kinds.len(), 2);
    assert_eq!(s.kinds[0].x402_version, 2);
    assert!(!s.settle);
    assert_eq!(s.signers["eip155:8453"], vec!["0x44".to_string()]);

    let r = x
        .requirements(&X402RequirementsParams {
            network: Some("base".into()),
            amount: "10000".into(),
            pay_to: PAYOUT.into(),
            resource: "https://api.example.com/v1/weather".into(),
            description: Some("Current weather".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(r.payee.deposit_address, "0x33");
    assert_eq!(r.amount, "10025");
    assert_eq!(r.fee.amount, "25");
    assert_eq!(r.accepts_v1[0]["maxAmountRequired"], "10025");
    assert_eq!(r.payment_required["x402Version"], 2);

    let req = X402FacilitatorRequest {
        x402_version: Some(2),
        payment_payload: json!({ "payload": { "signature": "0xsig" } }),
        payment_requirements: json!({ "scheme": "exact" }),
    };
    let v = x.verify(&req).await.unwrap();
    assert!(!v.is_valid);
    assert_eq!(v.invalid_reason.as_deref(), Some("insufficient_funds"));
    let st = x.settle(&req).await.unwrap();
    assert!(!st.success);
    assert_eq!(st.error_reason.as_deref(), Some("settlement_pending"));
    assert_eq!(st.transaction, "0xabc");

    let page = x
        .payments()
        .list(&ListX402PaymentsParams { status: Some("settled".into()), limit: Some(10), ..Default::default() })
        .await
        .unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));
    assert_eq!(page.payments[0].merchant_amount, "10000");
    let p = x.payments().get("x402_01k7t9m2c4x8q6r0v3b5n7p9d").await.unwrap();
    assert_eq!(p.status, "settled");
    assert_eq!(p.sweep_tx, None);
    let e = x.payments().get("x402_missing").await.unwrap_err();
    assert_eq!(e.code(), Some("NOT_FOUND"));
}

#[tokio::test]
async fn usage_get_and_errors() {
    let server = MockServer::start().await;
    let counts = json!({ "2xx": 90, "3xx": 0, "4xx": 8, "5xx": 2 });
    Mock::given(method("GET"))
        .and(path("/v1/usage"))
        .and(header("x-api-key", "lh_live_k"))
        .and(query_param("bucket", "day"))
        .and(query_param("from", "2026-11-01"))
        .and(query_param_is_missing("to"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "bucket": "day", "from": "2026-11-01T00:00:00.000Z", "to": "2026-11-04T00:00:00.000Z", "livemode": true, "keyId": null,
            "totals": { "requests": 100, "errors": 10, "byStatusClass": counts, "avgMs": 120.5, "p50Ms": 80, "p95Ms": null },
            "series": [{ "ts": "2026-11-01T00:00:00.000Z", "requests": 100, "errors": 10, "byStatusClass": counts, "p50Ms": 80, "p95Ms": 300 }],
            "byEndpoint": [{ "group": "quote", "requests": 100, "errors": 10, "byStatusClass": counts, "avgMs": 120.5, "p50Ms": 80, "p95Ms": 300 }],
            "errorCodes": [{ "code": "NO_ROUTE", "count": 8, "byEndpoint": { "quote": 8 } }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/usage/errors"))
        .and(query_param("code", "NO_ROUTE"))
        .and(query_param("limit", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "errors": [{
            "ts": "2026-11-04T10:00:00.000Z", "keyId": "key_1", "requestId": "req_1", "method": "GET", "path": "/v1/quote",
            "endpointGroup": "quote", "status": 404, "code": "NO_ROUTE", "latencyMs": 812
        }] })))
        .mount(&server)
        .await;

    let lh = client(&server);
    let u = lh
        .usage()
        .get(&UsageParams { bucket: Some("day".into()), from: Some("2026-11-01".into()), ..Default::default() })
        .await
        .unwrap();
    assert_eq!(u.totals.requests, 100);
    assert_eq!(u.totals.by_status_class.s4xx, 8);
    assert_eq!(u.totals.p95_ms, None);
    assert_eq!(u.series[0].p95_ms, Some(300.0));
    assert_eq!(u.by_endpoint[0].group, "quote");
    assert_eq!(u.error_codes[0].by_endpoint["quote"], 8);
    let errs = lh
        .usage()
        .errors(&UsageErrorsParams { code: Some("NO_ROUTE".into()), limit: Some(5), ..Default::default() })
        .await
        .unwrap();
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].request_id, "req_1");
    assert_eq!(errs[0].status, 404);
}

#[tokio::test]
async fn webhooks_resend_and_stream_since() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/events/evt_1/resend"))
        .and(body_json(json!({ "endpointId": "we_1" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "eventId": "evt_1", "type": "order.filled", "livemode": true, "deliveries": [{ "id": "wd_1", "endpointId": "we_1" }]
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/events/evt_2/resend"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "eventId": "evt_2", "type": "x402.payment.settled", "livemode": true, "deliveries": [] })))
        .expect(1)
        .mount(&server)
        .await;
    let ev = json!({ "id": "evt_9", "type": "x402.payment.settled", "livemode": true, "createdAt": "2026-11-04T10:00:00.000Z",
                     "data": { "payment": payment_json() } });
    Mock::given(method("GET"))
        .and(path("/v1/webhooks/stream"))
        .and(query_param("since", "2026-11-04T09:30:00.000Z"))
        .respond_with(sse(format!("event: ready\ndata: {{}}\n\nevent: event\ndata: {ev}\n\n")))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let r = lh.webhooks().resend("evt_1", Some("we_1")).await.unwrap();
    assert_eq!(r.deliveries[0].endpoint_id, "we_1");
    let r = lh.webhooks().resend("evt_2", None).await.unwrap();
    assert_eq!(r.event_type, "x402.payment.settled");
    assert!(r.deliveries.is_empty());

    let got: Vec<_> = lh.webhooks().stream_since("2026-11-04T09:30:00.000Z").collect().await;
    assert_eq!(got.len(), 1);
    let e = got[0].as_ref().unwrap();
    assert_eq!(e.data.payment.as_ref().unwrap().merchant_amount, "10000");
}

fn quote_params() -> QuoteParams {
    QuoteParams {
        src_chain: "base".into(),
        src_token: "USDC".into(),
        amount: "100000000".into(),
        dst_chain: "solana".into(),
        dst_token: "SOL".into(),
        sender: SENDER.into(),
        recipient: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn quote_stream_yields_updates_and_error_event() {
    let server = MockServer::start().await;
    let u1 = json!({ "seq": 1, "best": quote_json("q_1"), "quotes": [quote_json("q_1")], "errors": [], "livemode": true });
    let u2 = json!({ "seq": 2, "best": quote_json("q_2"), "quotes": [quote_json("q_2")], "errors": [], "screening": screening_json() });
    let body = format!(
        "event: ready\ndata: {{\"intervalMs\":5000,\"expiresAt\":\"x\",\"retryMs\":3000}}\n\nevent: quote\ndata: {u1}\n\n: ping\n\n\
         event: quote\r\ndata: {u2}\r\n\r\nevent: error\ndata: {{\"status\":429,\"error\":{{\"code\":\"RATE_LIMITED\",\"message\":\"Slow down\",\"requestId\":\"req_9\"}}}}\n\n"
    );
    Mock::given(method("GET"))
        .and(path("/v1/quote/stream"))
        .and(header("accept", "text/event-stream"))
        .and(query_param("amount", "100000000"))
        .and(query_param("srcChain", "base"))
        .and(query_param("intervalMs", "5000"))
        .and(query_param("channel", "api"))
        .respond_with(sse(body))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let got: Vec<_> = lh.quote_stream(&quote_params(), &QuoteStreamOptions { interval_ms: Some(5000) }).collect().await;
    assert_eq!(got.len(), 3);
    let a = got[0].as_ref().unwrap();
    assert_eq!((a.seq, a.response.best.quote_id.as_str()), (1, "q_1"));
    let b = got[1].as_ref().unwrap();
    assert_eq!(b.seq, 2);
    assert_eq!(b.response.screening.as_ref().unwrap().sanctioned[0].role, "recipient");
    match &got[2] {
        Err(Error::Api { code, status, request_id, .. }) => {
            assert_eq!(code, "RATE_LIMITED");
            assert_eq!(*status, 429);
            assert_eq!(request_id.as_deref(), Some("req_9"));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn quote_stream_exact_output_ends_on_end_event_and_open_errors_are_api_errors() {
    let server = MockServer::start().await;
    let u1 = json!({ "seq": 1, "best": quote_json("q_1"), "quotes": [quote_json("q_1")] });
    Mock::given(method("GET"))
        .and(path("/v1/quote/stream"))
        .and(query_param("dstAmount", "10000000"))
        .and(query_param_is_missing("amount"))
        .and(query_param_is_missing("intervalMs"))
        .respond_with(sse(format!(
            "event: quote\ndata: {u1}\n\nevent: end\ndata: {{\"reason\":\"expired\",\"quotesSent\":1}}\n\nevent: quote\ndata: {u1}\n\n"
        )))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/quote/stream"))
        .and(query_param("amount", "1"))
        .respond_with(ResponseTemplate::new(451).set_body_json(json!({ "error": { "code": "ADDRESS_SANCTIONED", "message": "Listed address" } })))
        .mount(&server)
        .await;

    let lh = client(&server);
    let p = ExactOutputQuoteParams {
        src_chain: "base".into(),
        src_token: "USDC".into(),
        dst_chain: "arbitrum".into(),
        dst_token: "USDC".into(),
        dst_amount: "10000000".into(),
        sender: SENDER.into(),
        recipient: SENDER.into(),
        ..Default::default()
    };
    let got: Vec<_> = lh.quote_stream_exact_output(&p, &QuoteStreamOptions::default()).collect().await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].as_ref().unwrap().response.best.src_amount_max.as_deref(), Some("10098240"));

    let got: Vec<_> =
        lh.quote_stream(&QuoteParams { amount: "1".into(), ..quote_params() }, &QuoteStreamOptions::default()).collect().await;
    assert_eq!(got.len(), 1);
    let e = got[0].as_ref().unwrap_err();
    assert_eq!(e.code(), Some("ADDRESS_SANCTIONED"));
}

#[tokio::test]
async fn quote_exact_output_sends_dst_amount() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .and(query_param("dstAmount", "10000000"))
        .and(query_param_is_missing("amount"))
        .and(query_param("slippageBps", "30"))
        .and(query_param("purpose", "payout"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "best": quote_json("q_1"), "quotes": [quote_json("q_1")], "errors": [],
            "warnings": [{ "code": "SOMETHING_NEW", "message": "x" }], "screening": { "source": "ofac_sdn", "listUpdatedAt": null, "sanctioned": [] }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let lh = client(&server);
    let r = lh
        .quote_exact_output(&ExactOutputQuoteParams {
            src_chain: "base".into(),
            src_token: "USDC".into(),
            dst_chain: "arbitrum".into(),
            dst_token: "USDC".into(),
            dst_amount: "10000000".into(),
            sender: SENDER.into(),
            recipient: SENDER.into(),
            slippage_bps: Some(30),
            purpose: Some("payout".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(r.best.src_amount.as_deref(), Some("10048000"));
    assert_eq!(r.best.src_amount_max.as_deref(), Some("10098240"));
    assert_eq!(r.warnings.unwrap()[0].code, "SOMETHING_NEW");
    let s = r.screening.unwrap();
    assert!(s.sanctioned.is_empty());
    assert_eq!(s.list_updated_at, None);
}

#[tokio::test]
async fn screen_addresses() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/screen"))
        .and(body_json(json!({ "addresses": ["0x098B716B8Aaf21512996dC57EB0615e2383E2f96", SENDER] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                { "address": "0x098B716B8Aaf21512996dC57EB0615e2383E2f96", "sanctioned": true, "lists": ["OFAC SDN"], "source": "ofac_sdn", "listUpdatedAt": "2026-10-28T00:00:00.000Z" },
                { "address": SENDER, "sanctioned": false, "lists": [], "source": "ofac_sdn", "listUpdatedAt": "2026-10-28T00:00:00.000Z" }
            ],
            "list": { "source": "ofac_sdn", "name": "OFAC SDN", "publishedAt": "2026-10-28T00:00:00.000Z", "fetchedAt": "2026-11-04T06:00:00.000Z", "addressCount": 1081 },
            "enforced": false
        })))
        .mount(&server)
        .await;
    let r = client(&server).screen(&["0x098B716B8Aaf21512996dC57EB0615e2383E2f96", SENDER]).await.unwrap();
    assert!(r.results[0].sanctioned);
    assert_eq!(r.results[0].lists, vec!["OFAC SDN".to_string()]);
    assert!(!r.results[1].sanctioned);
    assert_eq!(r.list.unwrap().address_count, 1081);
    assert!(!r.enforced);
}

fn session_with(extra: serde_json::Value) -> serde_json::Value {
    let mut s = json!({
        "id": CS, "object": "checkout.session", "livemode": true, "status": "open", "statusDetail": null, "amount": "49000000",
        "settlement": { "chainId": 8453, "token": "0x83", "symbol": "USDC", "decimals": 6 },
        "recipient": PAYOUT, "url": format!("https://layerhop.com/pay/{CS}"), "expiresAt": "2026-11-04T11:00:00.000Z",
        "payment": null, "createdAt": "2026-11-04T10:00:00.000Z", "updatedAt": "2026-11-04T10:00:00.000Z"
    });
    for (k, v) in extra.as_object().unwrap() {
        s[k] = v.clone();
    }
    s
}

#[tokio::test]
async fn checkout_embed_transfer_and_bitcoin() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .and(body_json(json!({ "amount": "49000000", "settleChain": "base", "settleToken": "USDC", "recipient": PAYOUT,
                               "embedOrigins": ["https://shop.example.com"] })))
        .respond_with(ResponseTemplate::new(201).set_body_json(session_with(json!({
            "embedOrigins": ["https://shop.example.com"], "transferAvailable": true, "bitcoinAvailable": true, "transfer": null, "bitcoin": null
        }))))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/checkout/sessions/{CS}/transfer")))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_with(json!({
            "status": "paid",
            "payment": { "method": "transfer", "quoteId": null, "payer": SENDER, "srcChainId": 8453, "srcToken": "0x83",
                         "srcAmount": "49000137", "srcTxHash": "0xaaa", "dstTxHash": "0xaaa", "paidAt": "2026-11-04T10:05:00.000Z" },
            "transfer": { "kind": "evm_splitter", "chainId": 8453, "token": "0x83", "symbol": "USDC", "decimals": 6, "address": "0x66",
                          "amount": "49122500", "amountDue": "49122500", "fee": null, "uri": "ethereum:0x83@8453/transfer?address=0x66&uint256=49122500",
                          "transferUri": null, "reference": null, "received": "49122500", "seen": "0", "state": "paid",
                          "txs": [{ "hash": "0xaaa", "amount": "49122500", "at": null }], "startedAt": "2026-11-04T10:01:00.000Z", "sweepTxHash": "0xbbb" }
        }))))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/checkout/sessions/{CS}/bitcoin")))
        .and(body_json(json!({ "refundAddress": "bc1qrefund" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_with(json!({
            "bitcoinAvailable": true,
            "bitcoin": { "address": "bc1qdeposit", "amountSats": "71234", "amountBtc": "0.00071234", "uri": "bitcoin:bc1qdeposit?amount=0.00071234",
                         "amountOut": "49000000", "expiresAt": "2026-11-04T10:20:00.000Z", "expired": false, "status": "waiting", "seenSats": "0",
                         "requestId": "0xreq", "feeBps": 25, "refundAddress": "bc1qrefund",
                         "quotes": [{ "address": "bc1qdeposit", "amountSats": "71234", "status": "waiting", "requestId": "0xreq", "createdAt": "2026-11-04T10:00:00.000Z" }] }
        }))))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions/cs_2/bitcoin"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "error": { "code": "FORBIDDEN", "message": "Bitcoin payments are off" } })))
        .mount(&server)
        .await;

    let lh = client(&server);
    let s = lh
        .checkout()
        .create(&CreateCheckoutParams {
            amount: "49000000".into(),
            settle_chain: "base".into(),
            settle_token: "USDC".into(),
            recipient: PAYOUT.into(),
            embed_origins: Some(vec!["https://shop.example.com".into()]),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(s.embed_origins.unwrap(), vec!["https://shop.example.com".to_string()]);
    assert_eq!(s.transfer_available, Some(true));
    assert_eq!(s.bitcoin_available, Some(true));
    assert!(s.transfer.is_none() && s.bitcoin.is_none());

    let s = lh.checkout().start_transfer(CS).await.unwrap();
    let p = s.payment.unwrap();
    assert_eq!(p.method.as_deref(), Some("transfer"));
    assert_eq!(p.quote_id, None);
    let t = s.transfer.unwrap();
    assert_eq!(t.kind, "evm_splitter");
    assert_eq!(t.txs[0].hash, "0xaaa");
    assert_eq!(t.sweep_tx_hash.as_deref(), Some("0xbbb"));

    let s = lh.checkout().start_bitcoin(CS, Some("bc1qrefund")).await.unwrap();
    let b = s.bitcoin.unwrap();
    assert_eq!(b.amount_sats, "71234");
    assert_eq!(b.fee_bps, 25);
    assert_eq!(b.quotes.len(), 1);

    let e = lh.checkout().start_bitcoin("cs_2", None).await.unwrap_err();
    assert_eq!(e.code(), Some("FORBIDDEN"));
}
