use layerhop::{BuiltTx, Error, LayerHop, QuoteParams, TRON_CHAIN_ID};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).build()
}

const TXID: &str = "9f1c2b3a4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8";

fn tron_tx() -> serde_json::Value {
    json!({
        "vm": "tvm", "chainId": 728126428, "from": "TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf",
        "call": { "to": "TQn9Y2khEsLJW1ChVWFMSMeRDow5KcbLSE", "data": "0xa9059cbb", "value": "0", "feeLimit": "30000000" },
        "unsigned": {
            "txID": TXID, "raw_data_hex": "0a02abcd",
            "raw_data": { "contract": [{ "type": "TriggerSmartContract" }], "ref_block_bytes": "abcd", "expiration": 1760000600000u64, "fee_limit": 30000000 },
            "visible": false
        },
        "approval": {
            "token": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t", "spender": "TQn9Y2khEsLJW1ChVWFMSMeRDow5KcbLSE", "amount": "5000000",
            "call": { "to": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t", "data": "0x095ea7b3", "value": "0", "feeLimit": "15000000" },
            "unsigned": { "txID": "aa".repeat(32), "raw_data_hex": "0a02", "raw_data": { "expiration": 1 } }
        },
        "someFutureField": 1
    })
}

#[tokio::test]
async fn build_tx_tron_with_approval_and_unsigned() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .and(body_json(json!({ "quoteId": "q_tron" })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "quoteId": "q_tron", "provider": "relay", "tx": tron_tx() })),
        )
        .mount(&server)
        .await;

    match client(&server).build_tx("q_tron").await.unwrap() {
        BuiltTx::Tvm(tx) => {
            assert_eq!(tx.chain_id, TRON_CHAIN_ID);
            assert_eq!(tx.from, "TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf");
            assert_eq!(tx.call.fee_limit, "30000000");
            let u = tx.unsigned.as_ref().unwrap();
            assert_eq!(u.tx_id, TXID);
            assert_eq!(u.raw_data_hex, "0a02abcd");
            assert_eq!(u.raw_data["expiration"], json!(1760000600000u64));
            assert_eq!(u.raw_data["contract"][0]["type"], "TriggerSmartContract");
            assert_eq!(u.visible, Some(false));
            let a = tx.approval.as_ref().unwrap();
            assert_eq!(a.amount, "5000000");
            assert_eq!(a.call.data, "0x095ea7b3");
            assert_eq!(a.unsigned.as_ref().unwrap().visible, None);
            // Round-trips unchanged (the wallet signs raw_data as built).
            let back = serde_json::to_value(BuiltTx::Tvm(tx.clone())).unwrap();
            assert_eq!(back["vm"], "tvm");
            assert_eq!(back["unsigned"]["txID"], TXID);
            assert_eq!(back["unsigned"]["raw_data"]["fee_limit"], 30000000);
        }
        other => panic!("expected tvm, got {other:?}"),
    }
}

#[tokio::test]
async fn simulate_reports_energy_used() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/simulate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "quoteId": "q_tron", "provider": "relay", "ok": true, "vm": "tvm", "energyUsed": 64285,
            "needsApproval": false, "simulated": "tx", "tx": tron_tx()
        })))
        .mount(&server)
        .await;
    let s = client(&server).simulate("q_tron").await.unwrap();
    assert!(s.ok);
    assert_eq!(s.vm, "tvm");
    assert_eq!(s.energy_used, Some(64285));
    assert!(matches!(s.tx, Some(BuiltTx::Tvm(_))));
}

fn sign_request() -> serde_json::Value {
    let typed = |primary: &str| {
        json!({
            "types": { "EIP712Domain": [{ "name": "name", "type": "string" }], primary: [{ "name": "value", "type": "uint256" }] },
            "domain": { "name": "USD Coin", "chainId": 8453 },
            "message": { "value": "100000000" },
            "primaryType": primary
        })
    };
    json!({
        "chainId": 8453,
        "approval": { "type": "permit", "eip712": typed("Permit") },
        "trade": { "type": "settler_metatransaction", "eip712": typed("MetaTransaction") },
        "gasFee": { "amount": "21000", "token": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913" }
    })
}

#[tokio::test]
async fn gasless_quote_build_submit_status() {
    let server = MockServer::start().await;
    let q = json!({
        "quoteId": "q_g", "provider": "zerox", "dstAmount": "99", "dstAmountMin": "98", "estimatedSeconds": 5,
        "fees": { "ourFeeBps": 10 }, "expiresAt": 1760000030u64,
        "gasless": { "gasFee": { "amount": "21000", "token": "0xusdc" }, "approval": true }
    });
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .and(query_param("gasless", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "best": q.clone(), "quotes": [q], "errors": [] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .and(body_json(json!({ "quoteId": "q_g" })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "quoteId": "q_g", "provider": "zerox", "gasless": sign_request() })),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/orders/gasless"))
        .and(body_json(json!({ "quoteId": "q_g", "signatures": { "trade": "0xtrade", "approval": "0xappr" } })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "quoteId": "q_g", "provider": "zerox", "orderId": "0xorder", "state": "pending"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/orders/gasless"))
        .and(body_json(json!({ "quoteId": "q_g2", "signatures": { "trade": "0xtrade" } })))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({ "error": {
            "code": "GASLESS_PENDING", "message": "being submitted", "requestId": "r", "docsUrl": "x" } })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/orders/gasless/q_g"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "quoteId": "q_g", "provider": "zerox", "orderId": "0xorder", "state": "filled", "srcTxHash": "0xsrc"
        })))
        .mount(&server)
        .await;

    let lh = client(&server);
    let r = lh
        .quote(&QuoteParams {
            src_chain: "base".into(),
            src_token: "USDC".into(),
            amount: "100000000".into(),
            dst_chain: "base".into(),
            dst_token: "ETH".into(),
            sender: "0x1".into(),
            recipient: "0x1".into(),
            gasless: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    let g = r.best.gasless.unwrap();
    assert!(g.approval);
    assert_eq!(g.gas_fee.unwrap().amount, "21000");

    let req = lh.build_gasless("q_g").await.unwrap();
    assert_eq!(req.chain_id, 8453);
    assert_eq!(req.trade.kind, "settler_metatransaction");
    assert_eq!(req.trade.eip712.primary_type, "MetaTransaction");
    assert_eq!(req.trade.eip712.message["value"], "100000000");
    assert_eq!(req.approval.as_ref().unwrap().eip712.domain["chainId"], 8453);
    assert_eq!(req.gas_fee.unwrap().amount, "21000");

    let s = lh.submit_gasless("q_g", "0xtrade", Some("0xappr")).await.unwrap();
    assert_eq!(s.order_id, "0xorder");
    assert_eq!(s.state, "pending");
    let e = lh.submit_gasless("q_g2", "0xtrade", None).await.unwrap_err();
    assert_eq!(e.code(), Some("GASLESS_PENDING"));

    let s = lh.gasless_status("q_g").await.unwrap();
    assert_eq!(s.state, "filled");
    assert_eq!(s.src_tx_hash.as_deref(), Some("0xsrc"));
    assert_eq!(s.reason, None);

    // build_tx on a gasless quote is a clear usage error, not a decode failure.
    match lh.build_tx("q_g").await.unwrap_err() {
        Error::Usage(m) => assert!(m.contains("build_gasless")),
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn build_gasless_on_regular_quote_is_usage_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "quoteId": "q_1", "provider": "relay",
            "tx": { "vm": "evm", "chainId": 8453, "to": "0xabc", "data": "0x", "value": "0" }
        })))
        .mount(&server)
        .await;
    match client(&server).build_gasless("q_1").await.unwrap_err() {
        Error::Usage(m) => assert!(m.contains("build_tx")),
        e => panic!("unexpected {e:?}"),
    }
}
