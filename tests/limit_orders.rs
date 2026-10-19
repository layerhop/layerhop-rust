use layerhop::{CreateLimitOrderParams, Error, LayerHop, ListLimitOrdersParams, WebhookEvent};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const LID: &str = "lo_01m5k8c2c4x8q6r0v3b5n7p9d";
const WALLET: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const SOL: &str = "So11111111111111111111111111111111111111112";

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).api_key("lh_test_k").build()
}

fn limit_order_json() -> serde_json::Value {
    json!({
        "id": LID, "object": "limit_order", "livemode": false, "status": "open", "statusDetail": null,
        "provider": "sandbox", "chainId": 7565164, "wallet": WALLET, "srcToken": USDC, "dstToken": SOL,
        "amount": "100000000", "dstAmount": "500000000", "expiresAt": null,
        "fees": { "ourFeeBps": 10, "integratorFeeBps": 0, "integrator": null },
        "providerOrderId": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU", "filledAmount": "0",
        "filledDstAmount": "0", "fillCount": 0, "openTxHash": "5sig", "cancelTxHash": null, "channel": "api",
        "metadata": { "userId": "42" }, "createdAt": "2026-10-19T10:00:00.000Z",
        "updatedAt": "2026-10-19T10:00:00.000Z", "someFutureField": 1
    })
}

fn with(changes: serde_json::Value) -> serde_json::Value {
    let mut v = limit_order_json();
    for (k, val) in changes.as_object().unwrap() {
        v[k] = val.clone();
    }
    v
}

fn fill_json() -> serde_json::Value {
    json!({
        "quoteId": "lof_1", "provider": "sandbox", "status": "filled", "statusDetail": null, "channel": "api",
        "ourFeeBps": 10, "partnerFeeBps": 0, "sender": WALLET, "recipient": WALLET, "srcChainId": 7565164,
        "srcToken": USDC, "dstChainId": 7565164, "dstToken": SOL, "amount": "50000000", "dstAmount": "250000000",
        "srcTxHash": "fillsig", "dstTxHash": "fillsig", "livemode": false, "limitOrderId": LID,
        "createdAt": "t", "updatedAt": "t"
    })
}

fn params() -> CreateLimitOrderParams {
    CreateLimitOrderParams {
        wallet: WALLET.into(),
        src_token: "USDC".into(),
        dst_token: "SOL".into(),
        amount: "100000000".into(),
        dst_amount: "500000000".into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn create_sends_body_and_parses_tx() {
    let server = MockServer::start().await;
    let created = with(json!({
        "status": "built", "openTxHash": null,
        "tx": { "vm": "svm", "serializedTx": "AQID", "submitViaApi": true, "sandbox": true }
    }));
    Mock::given(method("POST"))
        .and(path("/v1/limit-orders"))
        .and(header("x-api-key", "lh_test_k"))
        .and(body_json(json!({
            "wallet": WALLET, "srcToken": "USDC", "dstToken": "SOL", "amount": "100000000",
            "dstAmount": "500000000", "expiresAt": 1792000000u64, "chain": "solana", "channel": "bot",
            "metadata": { "userId": "42" }, "integrator": "acme", "integratorFeeBps": 10
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(created.clone()))
        .expect(1)
        .mount(&server)
        .await;
    let o = client(&server)
        .limit_orders()
        .create(&CreateLimitOrderParams {
            expires_at: Some(1792000000),
            chain: Some("solana".into()),
            channel: Some("bot".into()),
            metadata: Some([("userId".to_string(), "42".to_string())].into_iter().collect()),
            integrator: Some("acme".into()),
            integrator_fee_bps: Some(10),
            ..params()
        })
        .await
        .unwrap();
    assert_eq!(o.id, LID);
    assert_eq!(o.object, "limit_order");
    assert_eq!(o.status, "built");
    assert!(!o.is_final());
    assert!(!o.livemode);
    assert_eq!(o.chain_id, 7565164);
    assert_eq!(o.fees.our_fee_bps, 10);
    assert_eq!(o.fees.integrator, None);
    assert_eq!(o.fill_count, 0);
    assert_eq!(o.metadata.as_ref().unwrap()["userId"], "42");
    let tx = o.tx.unwrap();
    assert_eq!(tx.serialized_tx, "AQID");
    assert_eq!(tx.submit_via_api, Some(true));
    assert!(o.fills.is_none());

    // Minimal body gets the client's channel; a partner client adds its integrator settings.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/limit-orders"))
        .and(body_json(json!({
            "wallet": WALLET, "srcToken": "USDC", "dstToken": "SOL", "amount": "100000000",
            "dstAmount": "500000000", "channel": "api"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(created.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/limit-orders"))
        .and(body_json(json!({
            "wallet": WALLET, "srcToken": "USDC", "dstToken": "SOL", "amount": "100000000",
            "dstAmount": "500000000", "channel": "widget", "integrator": "acme", "integratorFeeBps": 5
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(created))
        .expect(1)
        .mount(&server)
        .await;
    client(&server).limit_orders().create(&params()).await.unwrap();
    let partner =
        LayerHop::builder().base_url(server.uri()).channel("widget").integrator("acme").integrator_fee_bps(5).build();
    partner.limit_orders().create(&params()).await.unwrap();
}

#[tokio::test]
async fn live_mode_disabled_is_forbidden() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/limit-orders"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error": { "code": "FORBIDDEN", "message": "Limit orders are not enabled for live mode yet.", "requestId": "r1" }
        })))
        .expect(1)
        .mount(&server)
        .await;
    match client(&server).limit_orders().create(&params()).await.unwrap_err() {
        Error::Api { code, status, .. } => {
            assert_eq!(code, "FORBIDDEN");
            assert_eq!(status, 403);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn submit_get_list_cancel_simulate() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/limit-orders/lo_1/submit"))
        .and(body_json(json!({ "signedTransaction": "c2lnbmVk" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "limitOrder": limit_order_json(), "txHash": "5sig" })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/limit-orders/lo_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(with(json!({
            "filledAmount": "50000000", "filledDstAmount": "250000000", "fillCount": 1, "fills": [fill_json()]
        }))))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/limit-orders"))
        .and(query_param("wallet", WALLET))
        .and(query_param("status", "open"))
        .and(query_param("cursor", "c1"))
        .and(query_param("limit", "10"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "limitOrders": [limit_order_json()], "nextCursor": "c2" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/limit-orders"))
        .and(query_param_is_missing("wallet"))
        .and(query_param_is_missing("status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "limitOrders": [], "nextCursor": null })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/limit-orders/lo_1/cancel"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "limitOrder": limit_order_json(),
            "tx": { "vm": "svm", "serializedTx": "Y2FuY2Vs", "submitViaApi": true }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/limit-orders/lo_2/cancel"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "limitOrder": with(json!({ "status": "cancelled" })), "tx": null
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/limit-orders/lo_1/simulate"))
        .and(body_json(json!({ "outcome": "partial_fill" })))
        .respond_with(ResponseTemplate::new(202).set_body_json(limit_order_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/limit-orders/lo_3/submit"))
        .respond_with(ResponseTemplate::new(409).set_body_json(
            json!({ "error": { "code": "CONFLICT", "message": "This order was already submitted", "requestId": "r2" } }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let s = lh.limit_orders().submit("lo_1", "c2lnbmVk").await.unwrap();
    assert_eq!(s.tx_hash, "5sig");
    assert_eq!(s.limit_order.status, "open");
    let o = lh.limit_orders().get("lo_1").await.unwrap();
    assert_eq!(o.fill_count, 1);
    assert_eq!(o.filled_dst_amount, "250000000");
    let fills = o.fills.unwrap();
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].limit_order_id.as_deref(), Some(LID));
    assert_eq!(fills[0].quote_id.as_deref(), Some("lof_1"));
    let page = lh
        .limit_orders()
        .list(&ListLimitOrdersParams {
            wallet: Some(WALLET.into()),
            status: Some("open".into()),
            cursor: Some("c1".into()),
            limit: Some(10),
        })
        .await
        .unwrap();
    assert_eq!(page.limit_orders.len(), 1);
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));
    let empty = lh.limit_orders().list(&ListLimitOrdersParams::default()).await.unwrap();
    assert!(empty.limit_orders.is_empty() && empty.next_cursor.is_none());
    let c = lh.limit_orders().cancel("lo_1").await.unwrap();
    assert_eq!(c.tx.unwrap().serialized_tx, "Y2FuY2Vs");
    let c = lh.limit_orders().cancel("lo_2").await.unwrap();
    assert!(c.tx.is_none());
    assert!(c.limit_order.is_final());
    assert_eq!(lh.limit_orders().simulate("lo_1", "partial_fill").await.unwrap().id, LID);
    match lh.limit_orders().submit("lo_3", "x").await.unwrap_err() {
        Error::Api { code, status, .. } => {
            assert_eq!(code, "CONFLICT");
            assert_eq!(status, 409);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn get_encodes_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/limit-orders/a%2Fb"))
        .respond_with(ResponseTemplate::new(200).set_body_json(with(json!({ "fills": [] }))))
        .expect(1)
        .mount(&server)
        .await;
    let o = client(&server).limit_orders().get("a/b").await.unwrap();
    assert_eq!(o.fills.unwrap().len(), 0);
}

#[test]
fn limit_order_events_parse() {
    let e: WebhookEvent = serde_json::from_value(json!({
        "id": "evt_1", "type": "limit_order.filled", "livemode": false, "createdAt": "t",
        "data": { "limitOrder": with(json!({ "status": "filled" })), "fill": fill_json() }
    }))
    .unwrap();
    assert_eq!(e.event_type, "limit_order.filled");
    assert!(e.data.limit_order.unwrap().is_final());
    assert_eq!(e.data.fill.unwrap().limit_order_id.as_deref(), Some(LID));
}
