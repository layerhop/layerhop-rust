use layerhop::{CreateDepositAddressParams, Error, LayerHop, ListDepositAddressesParams, Order};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DID: &str = "da_01m5f3k2c4x8q6r0v3b5n7p9d";

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).api_key("lh_test_k").build()
}

fn deposit_address_json() -> serde_json::Value {
    json!({
        "id": DID, "object": "deposit_address", "livemode": false, "status": "active", "provider": "sandbox",
        "address": "0x4c3f1a0b9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f4a", "chainId": 8453,
        "token": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "dstChainId": 42161,
        "dstToken": "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
        "recipient": "0x2222222222222222222222222222222222222222", "refundTo": "0x1111111111111111111111111111111111111111",
        "reusable": true, "amount": null, "fees": { "ourFeeBps": 20, "integratorFeeBps": 0, "integrator": null },
        "label": "user_42", "metadata": { "userId": "42" }, "depositCount": 0, "lastDepositAt": null,
        "createdAt": "2026-10-19T10:00:00.000Z", "someFutureField": 1
    })
}

fn deposit_order_json() -> serde_json::Value {
    json!({
        "quoteId": "dep_5f0c1e2d", "provider": "sandbox", "status": "submitted", "statusDetail": null, "channel": "api",
        "ourFeeBps": 20, "partnerFeeBps": 0, "sender": "0x1111", "recipient": "0x2222", "srcChainId": 8453,
        "srcToken": "0x83", "dstChainId": 42161, "dstToken": "0xaf", "amount": "25000000", "dstAmount": "24930000",
        "srcTxHash": "0xabc", "dstTxHash": null, "livemode": false, "checkoutSessionId": null, "refundId": null,
        "depositAddressId": DID, "keyId": "key_1", "partnerId": null, "partnerFeeEstimate": "0",
        "createdAt": "t", "updatedAt": "t"
    })
}

fn params() -> CreateDepositAddressParams {
    CreateDepositAddressParams {
        src_chain: "base".into(),
        src_token: "USDC".into(),
        dst_chain: "arbitrum".into(),
        dst_token: "USDC".into(),
        recipient: "0x2222".into(),
        refund_to: "0x1111".into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn create_sends_body_and_parses_estimate() {
    let server = MockServer::start().await;
    let mut created = deposit_address_json();
    created["estimate"] =
        json!({ "amount": "100000000", "dstAmount": "99720000", "dstAmountMin": "99221400", "estimatedSeconds": 12 });
    Mock::given(method("POST"))
        .and(path("/v1/deposit-addresses"))
        .and(header("x-api-key", "lh_test_k"))
        .and(body_json(json!({
            "srcChain": "base", "srcToken": "USDC", "dstChain": "arbitrum", "dstToken": "USDC",
            "recipient": "0x2222", "refundTo": "0x1111", "reusable": true, "amount": "100000000",
            "label": "user_42", "metadata": { "userId": "42" }, "integrator": "acme", "integratorFeeBps": 10
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(created.clone()))
        .expect(1)
        .mount(&server)
        .await;
    let d = client(&server)
        .deposit_addresses()
        .create(&CreateDepositAddressParams {
            reusable: Some(true),
            amount: Some("100000000".into()),
            label: Some("user_42".into()),
            metadata: Some([("userId".to_string(), "42".to_string())].into_iter().collect()),
            integrator: Some("acme".into()),
            integrator_fee_bps: Some(10),
            ..params()
        })
        .await
        .unwrap();
    assert_eq!(d.id, DID);
    assert_eq!(d.object, "deposit_address");
    assert!(d.is_active());
    assert!(!d.livemode);
    assert_eq!(d.provider, "sandbox");
    assert_eq!(d.chain_id, 8453);
    assert_eq!(d.dst_chain_id, 42161);
    assert!(d.reusable);
    assert_eq!(d.amount, None);
    assert_eq!(d.fees.our_fee_bps, 20);
    assert_eq!(d.fees.integrator, None);
    assert_eq!(d.metadata.as_ref().unwrap()["userId"], "42");
    assert_eq!(d.last_deposit_at, None);
    let e = d.estimate.unwrap();
    assert_eq!(e.dst_amount_min, "99221400");
    assert_eq!(e.estimated_seconds, 12);
    assert!(d.deposits.is_none());

    // Minimal body, then the client's integrator settings as defaults.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/deposit-addresses"))
        .and(body_json(json!({
            "srcChain": "base", "srcToken": "USDC", "dstChain": "arbitrum", "dstToken": "USDC",
            "recipient": "0x2222", "refundTo": "0x1111"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(created.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/deposit-addresses"))
        .and(body_json(json!({
            "srcChain": "base", "srcToken": "USDC", "dstChain": "arbitrum", "dstToken": "USDC",
            "recipient": "0x2222", "refundTo": "0x1111", "reusable": false, "amount": "5000000",
            "integrator": "acme", "integratorFeeBps": 5
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(created))
        .expect(1)
        .mount(&server)
        .await;
    client(&server).deposit_addresses().create(&params()).await.unwrap();
    let partner = LayerHop::builder().base_url(server.uri()).integrator("acme").integrator_fee_bps(5).build();
    partner
        .deposit_addresses()
        .create(&CreateDepositAddressParams { reusable: Some(false), amount: Some("5000000".into()), ..params() })
        .await
        .unwrap();
}

#[tokio::test]
async fn live_mode_disabled_is_forbidden() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/deposit-addresses"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error": { "code": "FORBIDDEN", "message": "Deposit addresses are not enabled for live mode yet.", "requestId": "r1" }
        })))
        .expect(1)
        .mount(&server)
        .await;
    match client(&server).deposit_addresses().create(&params()).await.unwrap_err() {
        Error::Api { code, status, .. } => {
            assert_eq!(code, "FORBIDDEN");
            assert_eq!(status, 403);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn get_list_refresh_disable_simulate() {
    let server = MockServer::start().await;
    let mut with_deposits = deposit_address_json();
    with_deposits["depositCount"] = json!(1);
    with_deposits["deposits"] = json!([deposit_order_json()]);
    Mock::given(method("GET"))
        .and(path("/v1/deposit-addresses/da_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(with_deposits))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/deposit-addresses"))
        .and(query_param("status", "active"))
        .and(query_param("label", "user_42"))
        .and(query_param("cursor", "c1"))
        .and(query_param("limit", "10"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "depositAddresses": [deposit_address_json()], "nextCursor": "c2" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/deposit-addresses"))
        .and(query_param_is_missing("status"))
        .and(query_param_is_missing("label"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "depositAddresses": [], "nextCursor": null })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/deposit-addresses/da_1/refresh"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(202).set_body_json(deposit_address_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/deposit-addresses/da_1/disable"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut v = deposit_address_json();
            v["status"] = json!("disabled");
            v
        }))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/deposit-addresses/da_1/simulate"))
        .and(body_json(json!({ "amount": "25000000" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "order": deposit_order_json() })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/deposit-addresses/da_2/refresh"))
        .respond_with(ResponseTemplate::new(409).set_body_json(
            json!({ "error": { "code": "CONFLICT", "message": "This deposit address is disabled", "requestId": "r2" } }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let d = lh.deposit_addresses().get("da_1").await.unwrap();
    assert_eq!(d.deposit_count, 1);
    let deposits = d.deposits.unwrap();
    assert_eq!(deposits.len(), 1);
    assert_eq!(deposits[0].deposit_address_id.as_deref(), Some(DID));
    assert_eq!(deposits[0].amount.as_deref(), Some("25000000"));
    let page = lh
        .deposit_addresses()
        .list(&ListDepositAddressesParams {
            status: Some("active".into()),
            label: Some("user_42".into()),
            cursor: Some("c1".into()),
            limit: Some(10),
        })
        .await
        .unwrap();
    assert_eq!(page.deposit_addresses.len(), 1);
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));
    let empty = lh.deposit_addresses().list(&ListDepositAddressesParams::default()).await.unwrap();
    assert!(empty.deposit_addresses.is_empty() && empty.next_cursor.is_none());
    assert!(lh.deposit_addresses().refresh("da_1").await.unwrap().is_active());
    assert_eq!(lh.deposit_addresses().disable("da_1").await.unwrap().status, "disabled");
    let sim = lh.deposit_addresses().simulate("da_1", "25000000").await.unwrap();
    assert_eq!(sim.order.provider.as_deref(), Some("sandbox"));
    assert_eq!(sim.order.deposit_address_id.as_deref(), Some(DID));
    match lh.deposit_addresses().refresh("da_2").await.unwrap_err() {
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
    let mut v = deposit_address_json();
    v["deposits"] = json!([]);
    Mock::given(method("GET"))
        .and(path("/v1/deposit-addresses/a%2Fb"))
        .respond_with(ResponseTemplate::new(200).set_body_json(v))
        .expect(1)
        .mount(&server)
        .await;
    let d = client(&server).deposit_addresses().get("a/b").await.unwrap();
    assert_eq!(d.deposits.unwrap().len(), 0);
}

#[test]
fn order_deposit_address_id_parses() {
    let o: Order = serde_json::from_value(deposit_order_json()).unwrap();
    assert_eq!(o.deposit_address_id.as_deref(), Some(DID));
    assert_eq!(o.checkout_session_id, None);
}
