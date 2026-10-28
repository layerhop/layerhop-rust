use layerhop::{CallsQuoteParams, DestinationAction, DestinationCall, Error, LayerHop, QuoteParams};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn quote_json() -> serde_json::Value {
    json!({
        "quoteId": "q_1", "provider": "relay", "dstAmount": "10000000", "dstAmountMin": "10000000",
        "estimatedSeconds": 1, "fees": { "ourFeeBps": 25 }, "expiresAt": 1792430763u64
    })
}

fn params() -> QuoteParams {
    QuoteParams {
        src_chain: "base".into(),
        src_token: "USDC".into(),
        amount: "5000000".into(),
        dst_chain: "arbitrum".into(),
        dst_token: "USDC".into(),
        sender: "0x1111111111111111111111111111111111111111".into(),
        recipient: "0x2222222222222222222222222222222222222222".into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn quote_sends_attribution_params() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .and(query_param("referrer", "0x3333333333333333333333333333333333333333"))
        .and(query_param("copyOf", "q_card"))
        .and(query_param("purpose", "split"))
        .and(query_param("splitId", "spl_1"))
        .and(query_param("splitShare", "2"))
        .and(query_param("note", "dinner"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "best": quote_json(), "quotes": [quote_json()] })))
        .expect(1)
        .mount(&server)
        .await;
    let lh = LayerHop::builder().base_url(server.uri()).build();
    lh.quote(&QuoteParams {
        referrer: Some("0x3333333333333333333333333333333333333333".into()),
        copy_of: Some("q_card".into()),
        purpose: Some("split".into()),
        split_id: Some("spl_1".into()),
        split_share: Some(2),
        note: Some("dinner".into()),
        ..params()
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn quote_calls_posts_json_body() {
    let server = MockServer::start().await;
    let mut q = quote_json();
    q["srcAmount"] = json!("10083608");
    q["calls"] = json!({
        "items": [{ "to": "0xaf88", "data": "0x095ea7b3", "value": "0" }, { "to": "0x794a", "data": "0x617ba037", "value": "0" }],
        "executor": "0xb92f", "executed": true,
        "simulation": { "ok": true, "gasUsed": "288491", "leftover": "0" }
    });
    Mock::given(method("POST"))
        .and(path("/v1/quote"))
        .and(body_json(json!({
            "srcChain": "base", "srcToken": "USDC", "dstChain": "arbitrum", "dstToken": "USDC",
            "dstAmount": "10000000", "sender": "0x1111111111111111111111111111111111111111",
            "recipient": "0x2222222222222222222222222222222222222222", "slippageBps": 50, "channel": "widget",
            "integrator": "myapp", "integratorFeeBps": 20,
            "calls": [{ "to": "0x794a", "data": "0x617ba037" }, { "to": "0x794b", "data": "0x", "value": "5" }],
            "referrer": "0x3333"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "best": q.clone(), "quotes": [q], "errors": [] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/quote"))
        .and(body_json(json!({
            "srcChain": "base", "srcToken": "USDC", "dstChain": "arbitrum", "dstToken": "USDC",
            "dstAmount": "10000000", "sender": "0x1111111111111111111111111111111111111111",
            "recipient": "0x2222222222222222222222222222222222222222", "slippageBps": 100, "channel": "widget",
            "integrator": "myapp", "integratorFeeBps": 20,
            "action": { "type": "aave-v3-supply" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "best": quote_json(), "quotes": [quote_json()] })))
        .expect(1)
        .mount(&server)
        .await;

    let lh = LayerHop::builder().base_url(server.uri()).channel("widget").integrator("myapp").integrator_fee_bps(20).build();
    let base = CallsQuoteParams {
        src_chain: "base".into(),
        src_token: "USDC".into(),
        dst_chain: "arbitrum".into(),
        dst_token: "USDC".into(),
        dst_amount: "10000000".into(),
        sender: "0x1111111111111111111111111111111111111111".into(),
        recipient: "0x2222222222222222222222222222222222222222".into(),
        ..Default::default()
    };
    let r = lh
        .quote_calls(&CallsQuoteParams {
            calls: Some(vec![
                DestinationCall { to: "0x794a".into(), data: "0x617ba037".into(), value: None },
                DestinationCall { to: "0x794b".into(), data: "0x".into(), value: Some("5".into()) },
            ]),
            referrer: Some("0x3333".into()),
            ..base.clone()
        })
        .await
        .unwrap();
    assert_eq!(r.best.src_amount.as_deref(), Some("10083608"));
    let plan = r.best.calls.unwrap();
    assert_eq!(plan.items.len(), 2);
    assert_eq!(plan.items[1].value.as_deref(), Some("0"));
    assert_eq!(plan.executor, "0xb92f");
    assert!(plan.executed);
    let sim = plan.simulation.unwrap();
    assert!(sim.ok);
    assert_eq!(sim.gas_used.as_deref(), Some("288491"));
    assert_eq!(sim.leftover.as_deref(), Some("0"));

    lh.quote_calls(&CallsQuoteParams {
        action: Some(DestinationAction::aave_v3_supply()),
        slippage_bps: Some(100),
        ..base
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn no_route_error_carries_suggestions() {
    let server = MockServer::start().await;
    let leg = |src: u64, dst: u64| {
        json!({ "srcChain": src, "srcToken": "0xa", "amount": "5000000", "dstChain": dst, "dstToken": "0xb", "dstAmount": "4990000" })
    };
    let mut via_hub = leg(8453, 1);
    via_hub["kind"] = json!("via_hub");
    via_hub["message"] = json!("Go through Arbitrum");
    via_hub["symbol"] = json!("USDC");
    via_hub["hub"] = json!({ "chainId": 42161, "token": "0xaf88", "symbol": "USDC", "decimals": 6 });
    via_hub["steps"] = json!([leg(8453, 42161), leg(42161, 1)]);
    let mut dst = leg(8453, 1);
    dst["kind"] = json!("dst_token");
    dst["message"] = json!("Receive USDC on Ethereum instead");
    let mut hub_without_steps = via_hub.clone();
    hub_without_steps["steps"] = json!([leg(8453, 42161)]);
    let mut zero_amount = dst.clone();
    zero_amount["amount"] = json!("0");
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "error": {
            "code": "NO_ROUTE", "message": "No route", "requestId": "r", "docsUrl": "x",
            "details": {
                "errors": [{ "provider": "relay", "code": "NO_ROUTE", "message": "none" }, { "bad": 1 }],
                "suggestions": [
                    dst, { "kind": "dst_token", "message": "x" }, hub_without_steps, via_hub,
                    zero_amount, { "kind": "nope" }, "garbage"
                ]
            }
        } })))
        .mount(&server)
        .await;

    let lh = LayerHop::builder().base_url(server.uri()).build();
    let e = lh.quote(&params()).await.unwrap_err();
    assert_eq!(e.code(), Some("NO_ROUTE"));
    assert!(e.details().is_some());
    assert!(e.amount_too_small().is_none());
    let pe = e.provider_errors();
    assert_eq!(pe.len(), 1);
    assert_eq!(pe[0].provider, "relay");
    let s = e.suggestions();
    assert_eq!(s.len(), 2, "{s:?}");
    assert_eq!(s[0].kind, "dst_token");
    assert_eq!(s[0].message, "Receive USDC on Ethereum instead");
    assert_eq!(s[0].src_chain, 8453);
    assert_eq!(s[0].dst_amount.as_deref(), Some("4990000"));
    assert!(s[0].hub.is_none() && s[0].steps.is_none());
    assert_eq!(s[1].kind, "via_hub");
    assert_eq!(s[1].symbol.as_deref(), Some("USDC"));
    assert_eq!(s[1].hub.as_ref().unwrap().chain_id, 42161);
    assert_eq!(s[1].hub.as_ref().unwrap().decimals, 6);
    let steps = s[1].steps.as_ref().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].dst_chain, 42161);
    assert_eq!(steps[1].src_chain, 42161);
    assert_eq!(s[1].leg().dst_chain, 1);
}

#[test]
fn suggestions_capped_at_four() {
    let s = json!({ "kind": "src_token", "message": "m", "srcChain": 1, "srcToken": "a", "amount": "1", "dstChain": 2, "dstToken": "b" });
    let details = json!({ "suggestions": [s.clone(), s.clone(), s.clone(), s.clone(), s] });
    assert_eq!(layerhop::parse_route_suggestions(&details).len(), 4);
    assert!(layerhop::parse_route_suggestions(&json!({ "suggestions": {} })).is_empty());
    assert!(layerhop::parse_route_suggestions(&json!(null)).is_empty());
}

#[tokio::test]
async fn amount_too_small_min_amount() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/quote"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({ "error": {
            "code": "AMOUNT_TOO_SMALL", "message": "Too small", "requestId": "r", "docsUrl": "x",
            "details": { "minAmount": "10000000", "minAmountUsd": 10.0, "minAmountApprox": true, "errors": [] }
        } })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/chains"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({ "error": {
            "code": "SOME_NEW_CODE", "message": "new", "requestId": "r", "docsUrl": "x" } })))
        .mount(&server)
        .await;
    let lh = LayerHop::builder().base_url(server.uri()).build();
    let e = lh.quote(&params()).await.unwrap_err();
    let d = e.amount_too_small().unwrap();
    assert_eq!(d.min_amount.as_deref(), Some("10000000"));
    assert_eq!(d.min_amount_usd, Some(10.0));
    assert_eq!(d.min_amount_approx, Some(true));
    assert!(e.suggestions().is_empty());
    assert!(e.provider_errors().is_empty());

    // Unknown codes still decode; no details is None.
    let e = lh.chains().await.unwrap_err();
    assert_eq!(e.code(), Some("SOME_NEW_CODE"));
    assert!(e.details().is_none());
    assert!(matches!(e, Error::Api { status: 400, .. }));
}
