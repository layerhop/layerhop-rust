use layerhop::{BalancesParams, LayerHop, TokenSearchParams};
use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).build()
}

const WETH: &str = "0x4200000000000000000000000000000000000006";

#[tokio::test]
async fn chains_with_route_health_only_route_destination_only() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/chains"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "chains": [
            { "id": 81457, "slug": "blast", "name": "Blast", "vm": "evm", "nativeSymbol": "ETH", "nativeDecimals": 18,
              "logoURI": "https://x/blast.svg", "explorerTx": "https://blastscan.io/tx/", "explorerAddress": "https://blastscan.io/address/",
              "providers": ["blast-native"], "popular": false, "destinationOnly": true,
              "onlyRoute": { "srcChainId": 1, "srcToken": "0x0000000000000000000000000000000000000000", "dstToken": "0x0000000000000000000000000000000000000000" },
              "routeHealth": { "in": "ok", "out": "failing", "status": "deposit_only", "checkedAt": "2026-10-28T10:00:00Z" } },
            { "id": 728126428, "slug": "tron", "name": "Tron", "vm": "tvm", "nativeSymbol": "TRX", "nativeDecimals": 6,
              "explorerTx": "https://tronscan.org/#/transaction/" }
        ]})))
        .mount(&server)
        .await;
    let chains = client(&server).chains().await.unwrap();
    let blast = &chains[0];
    assert_eq!(blast.destination_only, Some(true));
    assert_eq!(blast.only_route.as_ref().unwrap().src_chain_id, 1);
    let h = blast.route_health.as_ref().unwrap();
    assert_eq!((h.inbound.as_str(), h.outbound.as_str(), h.status.as_str()), ("ok", "failing", "deposit_only"));
    assert_eq!(h.checked_at.as_deref(), Some("2026-10-28T10:00:00Z"));
    assert_eq!(blast.logo_uri.as_deref(), Some("https://x/blast.svg"));
    assert_eq!(blast.providers, vec!["blast-native".to_string()]);
    assert_eq!(chains[1].vm, "tvm");
    assert_eq!(chains[1].id, layerhop::TRON_CHAIN_ID);
    assert!(chains[1].route_health.is_none() && chains[1].only_route.is_none());
}

#[tokio::test]
async fn search_tokens_pages() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/tokens"))
        .and(query_param("chain", "berachain"))
        .and(query_param("q", "HONEY"))
        .and(query_param("limit", "10"))
        .and(query_param("cursor", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tokens": [{ "chainId": 80094, "address": "0xFCBD", "symbol": "HONEY", "name": "HONEY", "decimals": 18,
                         "logoURI": "https://x/honey.svg", "priceUsd": 0.9979, "verified": true }],
            "nextCursor": null
        })))
        .expect(1)
        .mount(&server)
        .await;
    let page = client(&server)
        .search_tokens(&TokenSearchParams {
            chain: "berachain".into(),
            q: Some("HONEY".into()),
            limit: Some(10),
            cursor: Some("1".into()),
        })
        .await
        .unwrap();
    assert_eq!(page.next_cursor, None);
    let t = &page.tokens[0];
    assert_eq!(t.name, "HONEY");
    assert_eq!(t.price_usd, Some(0.9979));
    assert!(t.verified);
    assert_eq!(t.popular, None);
    assert_eq!(t.logo_uri.as_deref(), Some("https://x/honey.svg"));
}

#[tokio::test]
async fn lookup_token_with_and_without_chain() {
    let server = MockServer::start().await;
    let body = json!({ "token": { "chainId": 8453, "address": WETH, "symbol": "WETH", "name": "Wrapped Ether",
                                  "decimals": 18, "priceUsd": 2423.34, "verified": true, "popular": true } });
    Mock::given(method("GET"))
        .and(path("/v1/tokens/lookup"))
        .and(query_param("address", WETH))
        .and(query_param("chain", "base"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/tokens/lookup"))
        .and(query_param("address", WETH))
        .and(query_param_is_missing("chain"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;
    let lh = client(&server);
    let t = lh.lookup_token(WETH, Some("base")).await.unwrap();
    assert_eq!(t.symbol, "WETH");
    assert_eq!(t.popular, Some(true));
    let t = lh.lookup_token(WETH, None).await.unwrap();
    assert_eq!(t.chain_id, 8453);
}

#[tokio::test]
async fn get_token_risk() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/tokens/base/0x1111111111111111111111111111111111111111/risk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "risk": {
            "chainId": 8453, "address": "0x1111111111111111111111111111111111111111", "level": "danger",
            "verified": false, "symbol": "USDC",
            "reasons": [
                { "code": "fake_symbol", "level": "danger", "message": "Calls itself USDC" },
                { "code": "sell_tax", "level": "danger", "value": 25, "message": "25% is taken on every sell." },
                { "code": "brand_new_code", "level": "caution", "message": "new" }
            ],
            "source": "goplus", "checkedAt": "2026-10-27T09:00:00.000Z"
        } })))
        .mount(&server)
        .await;
    let r = client(&server).get_token_risk("base", "0x1111111111111111111111111111111111111111").await.unwrap();
    assert_eq!(r.level, "danger");
    assert!(!r.verified);
    assert_eq!(r.symbol.as_deref(), Some("USDC"));
    assert_eq!(r.reasons.len(), 3);
    assert_eq!(r.reasons[1].value, Some(25.0));
    assert_eq!(r.reasons[2].code, "brand_new_code");
    assert_eq!(r.source, "goplus");
}

#[tokio::test]
async fn prices_and_balances() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/prices"))
        .and(query_param("ids", "8453:0xusdc,7565164:So11111111111111111111111111111111111111112"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "prices": {
            "8453:0xusdc": 0.9998, "7565164:So11111111111111111111111111111111111111112": 103.88 } })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/balances"))
        .and(query_param("address", "0xd8dA"))
        .and(query_param("chains", "base,berachain"))
        .and(query_param("tokens", "80094:0x59a6,8453:0xusdc"))
        .and(query_param("fresh", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "address": "0xd8dA", "vm": "evm",
            "balances": [{ "chainId": 8453, "address": "0x0000000000000000000000000000000000000000", "symbol": "ETH", "name": "ETH",
                           "decimals": 18, "priceUsd": 2423.34, "verified": true, "popular": true,
                           "amount": "3128821803472737280", "valueUsd": 7582.2 }],
            "totalUsd": 7582.2,
            "errors": [{ "chainId": 80094, "message": "Berachain: timed out" }]
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/balances"))
        .and(query_param("address", "9WzD"))
        .and(query_param_is_missing("chains"))
        .and(query_param_is_missing("tokens"))
        .and(query_param_is_missing("fresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "address": "9WzD", "vm": "svm", "balances": [], "totalUsd": 0, "errors": [] })))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    assert!(lh.prices(&[]).await.unwrap().is_empty());
    let p = lh.prices(&["8453:0xusdc", "7565164:So11111111111111111111111111111111111111112"]).await.unwrap();
    assert_eq!(p["8453:0xusdc"], 0.9998);

    let b = lh
        .balances(&BalancesParams {
            address: "0xd8dA".into(),
            chains: Some(vec!["base".into(), "berachain".into()]),
            tokens: Some(vec!["80094:0x59a6".into(), "8453:0xusdc".into()]),
            fresh: true,
        })
        .await
        .unwrap();
    assert_eq!(b.vm, "evm");
    assert_eq!(b.balances[0].amount, "3128821803472737280");
    assert_eq!(b.balances[0].token.symbol, "ETH");
    assert_eq!(b.balances[0].value_usd, Some(7582.2));
    assert_eq!(b.total_usd, 7582.2);
    assert_eq!(b.errors[0].chain_id, 80094);

    let b = lh.balances(&BalancesParams { address: "9WzD".into(), ..Default::default() }).await.unwrap();
    assert!(b.balances.is_empty());
}
