use std::time::Duration;

use futures_util::StreamExt;
use layerhop::{BuiltTx, Error, GasOnArrivalRequest, LayerHop, ListOrdersParams, Order, QuoteParams, WatchOptions};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(format!("{}/", server.uri())).api_key("k_test").build()
}

fn quote_json(id: &str, provider: &str, out: &str) -> serde_json::Value {
    json!({
        "quoteId": id, "provider": provider, "dstAmount": out, "dstAmountMin": out,
        "estimatedSeconds": 8, "fees": { "ourFeeBps": 20, "gasUsd": 0.03 }, "expiresAt": 1760000030u64,
        "someFutureField": true
    })
}

fn params() -> QuoteParams {
    QuoteParams {
        src_chain: "base".into(),
        src_token: "USDC".into(),
        amount: "100000000".into(),
        dst_chain: "solana".into(),
        dst_token: "SOL".into(),
        sender: "0x1111111111111111111111111111111111111111".into(),
        recipient: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".into(),
        slippage_bps: None,
        gas_on_arrival: None,
    }
}

#[test]
fn builder_defaults() {
    assert_eq!(LayerHop::new().base_url(), "https://api.layerhop.com");
    assert_eq!(LayerHop::builder().base_url("http://x:1///").build().base_url(), "http://x:1");
}

#[tokio::test]
async fn health_chains_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true, "providers": ["relay"] })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/chains"))
        .and(header("x-api-key", "k_test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "chains": [
            { "id": 7565164, "slug": "solana", "name": "Solana", "vm": "svm", "nativeSymbol": "SOL",
              "nativeDecimals": 9, "explorerTx": "https://solscan.io/tx/" }
        ]})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/tokens"))
        .and(query_param("chain", "base"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "tokens": [
            { "chainId": 8453, "address": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "symbol": "USDC", "decimals": 6 }
        ]})))
        .mount(&server)
        .await;

    let lh = client(&server);
    assert_eq!(lh.health().await.unwrap().providers, vec!["relay".to_string()]);
    let chains = lh.chains().await.unwrap();
    assert_eq!(chains[0].id, layerhop::SOLANA_CHAIN_ID);
    assert_eq!(chains[0].native_decimals, 9);
    let tokens = lh.tokens("base").await.unwrap();
    assert_eq!(tokens[0].symbol, "USDC");
    assert_eq!(tokens[0].decimals, 6);
}

#[tokio::test]
async fn quote_sends_all_params() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .and(query_param("srcChain", "base"))
        .and(query_param("srcToken", "USDC"))
        .and(query_param("amount", "100000000"))
        .and(query_param("dstChain", "solana"))
        .and(query_param("dstToken", "SOL"))
        .and(query_param("sender", "0x1111111111111111111111111111111111111111"))
        .and(query_param("recipient", "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"))
        .and(query_param("slippageBps", "50"))
        .and(query_param("channel", "widget"))
        .and(query_param("integrator", "myapp"))
        .and(query_param("integratorFeeBps", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "best": quote_json("q_1", "relay", "612345678"),
            "quotes": [quote_json("q_1", "relay", "612345678"), quote_json("q_2", "debridge", "611900000")],
            "errors": [{ "provider": "lifi", "code": "PROVIDER_TIMEOUT", "message": "Timed out" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let lh = LayerHop::builder()
        .base_url(server.uri())
        .channel("widget")
        .integrator("myapp")
        .integrator_fee_bps(25)
        .build();
    let r = lh.quote(&params()).await.unwrap();
    assert_eq!(r.best.quote_id, "q_1");
    assert_eq!(r.best.dst_amount, "612345678");
    assert_eq!(r.best.fees.our_fee_bps, 20);
    assert_eq!(r.best.fees.gas_usd, Some(0.03));
    assert_eq!(r.quotes.len(), 2);
    assert_eq!(r.errors[0].code, "PROVIDER_TIMEOUT");
}

#[tokio::test]
async fn quote_gas_on_arrival() {
    let server = MockServer::start().await;
    let mut with_gas = quote_json("q_1", "relay", "612345678");
    with_gas["gasOnArrival"] = json!({
        "amount": "500000000000000", "token": "0x0000000000000000000000000000000000000000",
        "symbol": "ETH", "decimals": 18, "usd": 2.01
    });
    let body = json!({ "best": with_gas.clone(), "quotes": [with_gas], "errors": [] });
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .and(query_param("gasOnArrival", "true"))
        .and(query_param_is_missing("gasOnArrivalUsd"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .and(query_param("gasOnArrivalUsd", "5.5"))
        .and(query_param_is_missing("gasOnArrival"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .and(query_param_is_missing("gasOnArrival"))
        .and(query_param_is_missing("gasOnArrivalUsd"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "best": quote_json("q_2", "relay", "1"), "quotes": [quote_json("q_2", "relay", "1")], "errors": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let r = lh
        .quote(&QuoteParams { gas_on_arrival: Some(GasOnArrivalRequest::Default), ..params() })
        .await
        .unwrap();
    let g = r.best.gas_on_arrival.unwrap();
    assert_eq!(g.amount, "500000000000000");
    assert_eq!(g.symbol, "ETH");
    assert_eq!(g.decimals, 18);
    assert_eq!(g.usd, Some(2.01));
    lh.quote(&QuoteParams { gas_on_arrival: Some(GasOnArrivalRequest::Usd(5.5)), ..params() }).await.unwrap();
    let plain = lh.quote(&params()).await.unwrap();
    assert!(plain.best.gas_on_arrival.is_none());
}

#[test]
fn order_limit_order_id_parses() {
    let o: Order = serde_json::from_value(json!({ "quoteId": "lof_1", "limitOrderId": "lo_1" })).unwrap();
    assert_eq!(o.limit_order_id.as_deref(), Some("lo_1"));
    let o: Order = serde_json::from_value(json!({ "quoteId": "q_1" })).unwrap();
    assert_eq!(o.limit_order_id, None);
}

#[tokio::test]
async fn build_tx_evm_and_svm() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .and(body_json(json!({ "quoteId": "q_evm" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "quoteId": "q_evm", "provider": "relay",
            "tx": { "vm": "evm", "chainId": 8453, "to": "0xabc", "data": "0x58109c", "value": "0",
                    "approval": { "token": "0xusdc", "spender": "0xabc", "amount": "100000000" } }
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .and(body_json(json!({ "quoteId": "q_svm" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "quoteId": "q_svm", "provider": "jupiter",
            "tx": { "vm": "svm", "serializedTx": "AQID", "submitViaApi": true }
        })))
        .mount(&server)
        .await;

    let lh = client(&server);
    match lh.build_tx("q_evm").await.unwrap() {
        BuiltTx::Evm(tx) => {
            assert_eq!(tx.chain_id, 8453);
            assert_eq!(tx.value, "0");
            assert_eq!(tx.approval.unwrap().amount, "100000000");
        }
        other => panic!("expected evm, got {other:?}"),
    }
    match lh.build_tx("q_svm").await.unwrap() {
        BuiltTx::Svm(tx) => {
            assert_eq!(tx.serialized_tx, "AQID");
            assert_eq!(tx.submit_via_api, Some(true));
        }
        other => panic!("expected svm, got {other:?}"),
    }
}

#[tokio::test]
async fn submit_report_submitted_simulate() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/orders/submit"))
        .and(body_json(json!({ "quoteId": "q_1", "signedTransaction": "c2lnbmVk" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "provider": "jupiter", "txHash": "5VER" })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/orders/q_1/submitted"))
        .and(body_json(json!({ "txHash": "0xdead" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "quoteId": "q_1", "status": "submitted" })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/simulate"))
        .and(body_json(json!({ "quoteId": "q_1" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": false, "vm": "evm", "gasEstimate": 21000, "needsApproval": true,
            "error": { "code": "SIMULATION_FAILED", "message": "execution reverted", "logs": ["revert"] },
            "tx": { "vm": "evm", "chainId": 8453, "to": "0xabc", "data": "0x", "value": "0" }
        })))
        .mount(&server)
        .await;

    let lh = client(&server);
    assert_eq!(lh.submit("q_1", "c2lnbmVk").await.unwrap(), "5VER");
    let r = lh.report_submitted("q_1", "0xdead").await.unwrap();
    assert_eq!(r.quote_id, "q_1");
    assert_eq!(r.status, "submitted");
    let s = lh.simulate("q_1").await.unwrap();
    assert!(!s.ok);
    assert_eq!(s.gas_estimate.as_deref(), Some("21000"));
    assert_eq!(s.needs_approval, Some(true));
    assert_eq!(s.error.unwrap().logs.unwrap(), vec!["revert".to_string()]);
    assert!(matches!(s.tx, Some(BuiltTx::Evm(_))));
}

#[tokio::test]
async fn status_encodes_id_and_list_orders() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/orders/debridge/a%2Fb"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "provider": "debridge", "id": "a/b", "state": "filled", "srcTxHash": "0x1", "dstTxHash": "sig"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/orders"))
        .and(query_param("address", "0x1111"))
        .and(query_param("status", "filled"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "orders": [{ "quoteId": "q_1", "provider": "relay", "status": "filled", "amount": "100000000", "extra": 1 }],
            "nextCursor": "c_2"
        })))
        .mount(&server)
        .await;

    let lh = client(&server);
    let s = lh.status("debridge", "a/b").await.unwrap();
    assert!(s.is_final());
    assert_eq!(s.dst_tx_hash.as_deref(), Some("sig"));

    let r = lh
        .list_orders(&ListOrdersParams {
            address: Some("0x1111".into()),
            status: Some("filled".into()),
            limit: Some(10),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(r.orders[0].quote_id.as_deref(), Some("q_1"));
    assert_eq!(r.orders[0].amount.as_deref(), Some("100000000"));
    assert_eq!(r.next_cursor.as_deref(), Some("c_2"));
}

#[tokio::test]
async fn api_errors_carry_code_and_request_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/orders"))
        .respond_with(ResponseTemplate::new(410).set_body_json(json!({ "error": {
            "code": "QUOTE_EXPIRED", "message": "Quote expired, request a new one",
            "requestId": "req-1", "docsUrl": "https://docs.layerhop.com/errors/QUOTE_EXPIRED" } })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/chains"))
        .respond_with(ResponseTemplate::new(502).set_body_string("bad gateway").insert_header("x-request-id", "hdr-1"))
        .mount(&server)
        .await;

    let lh = client(&server);
    match lh.build_tx("q_old").await.unwrap_err() {
        Error::Api { code, status, request_id, message } => {
            assert_eq!(code, "QUOTE_EXPIRED");
            assert_eq!(status, 410);
            assert_eq!(request_id.as_deref(), Some("req-1"));
            assert!(message.contains("expired"));
        }
        e => panic!("unexpected {e:?}"),
    }
    let e = lh.chains().await.unwrap_err();
    assert_eq!(e.code(), Some("INTERNAL"));
    if let Error::Api { status, request_id, .. } = e {
        assert_eq!(status, 502);
        assert_eq!(request_id.as_deref(), Some("hdr-1"));
    }
}

#[tokio::test]
async fn decode_error_on_bad_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/chains"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;
    assert!(matches!(client(&server).chains().await, Err(Error::Decode(_))));
}

fn status_body(state: &str) -> serde_json::Value {
    json!({ "provider": "relay", "id": "0xabc", "state": state })
}

#[tokio::test]
async fn watch_yields_changes_until_final() {
    let server = MockServer::start().await;
    // Mocks are matched in mount order; each answers a limited number of times.
    for (state, times) in [("pending", 2u64), ("filled", 1)] {
        Mock::given(method("GET"))
            .and(path("/v1/orders/relay/0xabc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_body(state)))
            .up_to_n_times(times)
            .mount(&server)
            .await;
    }
    let lh = client(&server);
    let opts = WatchOptions { interval: Duration::from_millis(5), timeout: Duration::from_secs(5) };
    let states: Vec<String> = lh
        .watch("relay", "0xabc", opts)
        .map(|r| r.unwrap().state)
        .collect()
        .await;
    assert_eq!(states, vec!["pending".to_string(), "filled".to_string()]);
}

#[tokio::test]
async fn watch_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/orders/relay/0xabc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(status_body("pending")))
        .mount(&server)
        .await;
    let lh = client(&server);
    let opts = WatchOptions { interval: Duration::from_millis(10), timeout: Duration::from_millis(60) };
    let items: Vec<_> = lh.watch("relay", "0xabc", opts).collect().await;
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].as_ref().unwrap().state, "pending");
    assert!(matches!(items[1], Err(Error::Timeout)));
}

#[tokio::test]
async fn watch_stops_on_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/orders/relay/0xabc"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "error": {
            "code": "ORDER_NOT_FOUND", "message": "not found", "requestId": "r", "docsUrl": "x" } })))
        .mount(&server)
        .await;
    let items: Vec<_> = client(&server).watch("relay", "0xabc", WatchOptions::default()).collect().await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].as_ref().unwrap_err().code(), Some("ORDER_NOT_FOUND"));
}
