use std::collections::BTreeMap;

use layerhop::{
    CreateCheckoutProductParams, Error, LayerHop, ListCheckoutProductsParams, StartProductCheckoutParams,
    UpdateCheckoutProductParams,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PID: &str = "prod_01m57hq2c4x8q6r0v3b5n7p9d";

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).api_key("lh_test_k").build()
}

fn product_json() -> serde_json::Value {
    json!({
        "id": PID, "object": "checkout.product", "livemode": true, "active": true, "name": "Pro plan",
        "description": "1 month", "merchantName": null, "amount": "49000000",
        "settlement": { "chainId": 8453, "token": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "symbol": "USDC", "decimals": 6 },
        "url": format!("https://layerhop.com/buy/{PID}"),
        "recipient": "0x2222222222222222222222222222222222222222", "successUrl": "https://shop.example.com/thanks",
        "cancelUrl": null, "expiresInMinutes": 60, "allowedOrigins": ["https://shop.example.com"],
        "createdAt": "2026-10-18T10:00:00.000Z", "updatedAt": "2026-10-18T10:00:00.000Z",
        "someFutureField": 1
    })
}

fn session_json() -> serde_json::Value {
    json!({
        "id": "cs_1", "object": "checkout.session", "livemode": true, "status": "open", "statusDetail": null,
        "amount": "49000000",
        "settlement": { "chainId": 8453, "token": "0x83", "symbol": "USDC", "decimals": 6 },
        "recipient": "0x2222", "merchantName": null, "description": "1 month", "url": "https://layerhop.com/pay/cs_1",
        "productId": PID, "successUrl": null, "cancelUrl": null, "expiresAt": "e", "payment": null,
        "reference": "order_1042", "metadata": { "plan": "pro" }, "createdAt": "c", "updatedAt": "u"
    })
}

#[test]
fn version_is_0_4_0() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.4.0");
}

#[tokio::test]
async fn create_sends_body_and_parses_product() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/products"))
        .and(header("x-api-key", "lh_test_k"))
        .and(body_json(json!({
            "name": "Pro plan", "description": "1 month", "amount": "49000000", "settleChain": "base",
            "settleToken": "USDC", "recipient": "0x2222222222222222222222222222222222222222",
            "successUrl": "https://shop.example.com/thanks", "expiresInMinutes": 60,
            "allowedOrigins": ["https://shop.example.com"]
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(product_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/products"))
        .and(body_json(json!({
            "name": "Tip", "amount": "1", "settleChain": "8453", "settleToken": "USDC", "recipient": "0x2", "active": false
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(product_json()))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let p = lh
        .checkout()
        .products()
        .create(&CreateCheckoutProductParams {
            name: "Pro plan".into(),
            description: Some("1 month".into()),
            amount: "49000000".into(),
            settle_chain: "base".into(),
            settle_token: "USDC".into(),
            recipient: "0x2222222222222222222222222222222222222222".into(),
            success_url: Some("https://shop.example.com/thanks".into()),
            expires_in_minutes: Some(60),
            allowed_origins: Some(vec!["https://shop.example.com".into()]),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(p.id, PID);
    assert_eq!(p.object, "checkout.product");
    assert!(p.livemode && p.active);
    assert_eq!(p.url, format!("https://layerhop.com/buy/{PID}"));
    assert_eq!(p.settlement.decimals, 6);
    assert_eq!(p.merchant_name, None);
    assert_eq!(p.cancel_url, None);
    assert_eq!(p.expires_in_minutes, Some(60));
    assert_eq!(p.allowed_origins.as_deref(), Some(&["https://shop.example.com".to_string()][..]));

    lh.checkout()
        .products()
        .create(&CreateCheckoutProductParams {
            name: "Tip".into(),
            amount: "1".into(),
            settle_chain: "8453".into(),
            settle_token: "USDC".into(),
            recipient: "0x2".into(),
            active: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn get_public_and_encoded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/checkout/products/prod_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "prod_1", "object": "checkout.product", "livemode": false, "active": true, "name": "Tip",
            "description": null, "merchantName": null, "amount": "1",
            "settlement": { "chainId": 8453, "token": "0x83", "symbol": "USDC", "decimals": 6 }, "url": "u"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/checkout/products/a%2Fb"))
        .respond_with(ResponseTemplate::new(200).set_body_json(product_json()))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let p = lh.checkout().products().get("prod_1").await.unwrap();
    assert!(!p.livemode);
    assert!(p.recipient.is_none() && p.allowed_origins.is_none() && p.created_at.is_none());
    assert_eq!(lh.checkout().products().get("a/b").await.unwrap().id, PID);
}

#[tokio::test]
async fn list_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/checkout/products"))
        .and(query_param("active", "true"))
        .and(query_param("cursor", "c1"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "products": [product_json()], "nextCursor": "c2" })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/checkout/products"))
        .and(query_param("active", "false"))
        .and(query_param_is_missing("limit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "products": [], "nextCursor": null })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/checkout/products"))
        .and(query_param_is_missing("active"))
        .and(query_param_is_missing("cursor"))
        .and(query_param_is_missing("limit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "products": [], "nextCursor": null })))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let page = lh
        .checkout()
        .products()
        .list(&ListCheckoutProductsParams { active: Some(true), cursor: Some("c1".into()), limit: Some(10) })
        .await
        .unwrap();
    assert_eq!(page.products[0].id, PID);
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));
    let inactive = lh
        .checkout()
        .products()
        .list(&ListCheckoutProductsParams { active: Some(false), ..Default::default() })
        .await
        .unwrap();
    assert!(inactive.products.is_empty());
    let all = lh.checkout().products().list(&ListCheckoutProductsParams::default()).await.unwrap();
    assert_eq!(all.next_cursor, None);
}

#[tokio::test]
async fn update_is_partial() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/checkout/products/{PID}")))
        .and(body_json(json!({ "active": false })))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut p = product_json();
            p["active"] = json!(false);
            p
        }))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/checkout/products/{PID}")))
        .and(body_json(json!({
            "name": "Pro", "description": null, "amount": "50000000", "settleChain": "42161",
            "cancelUrl": null, "allowedOrigins": []
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(product_json()))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let p = lh
        .checkout()
        .products()
        .update(PID, &UpdateCheckoutProductParams { active: Some(false), ..Default::default() })
        .await
        .unwrap();
    assert!(!p.active);
    lh.checkout()
        .products()
        .update(
            PID,
            &UpdateCheckoutProductParams {
                name: Some("Pro".into()),
                description: Some(None),
                amount: Some("50000000".into()),
                settle_chain: Some("42161".into()),
                cancel_url: Some(None),
                allowed_origins: Some(vec![]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(UpdateCheckoutProductParams::default()).unwrap(), json!({}));
}

#[tokio::test]
async fn start_session_public() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/checkout/products/{PID}/sessions")))
        .and(body_json(json!({ "reference": "order_1042", "metadata": { "plan": "pro" } })))
        .respond_with(ResponseTemplate::new(201).set_body_json(session_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/products/prod_empty/sessions"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(201).set_body_json({
            let mut s = session_json();
            s.as_object_mut().unwrap().remove("productId");
            s
        }))
        .expect(1)
        .mount(&server)
        .await;

    let lh = LayerHop::builder().base_url(server.uri()).build();
    let s = lh
        .checkout()
        .products()
        .start_session(
            PID,
            &StartProductCheckoutParams {
                reference: Some("order_1042".into()),
                metadata: Some(BTreeMap::from([("plan".to_string(), "pro".to_string())])),
            },
        )
        .await
        .unwrap();
    assert_eq!(s.object, "checkout.session");
    assert_eq!(s.product_id.as_deref(), Some(PID));
    assert_eq!(s.reference.as_deref(), Some("order_1042"));
    let requests = server.received_requests().await.unwrap();
    assert!(requests[0].headers.get("x-api-key").is_none());

    let s = lh.checkout().products().start_session("prod_empty", &StartProductCheckoutParams::default()).await.unwrap();
    assert_eq!(s.product_id, None);
}

#[tokio::test]
async fn inactive_product_conflict() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/checkout/products/{PID}/sessions")))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "error": { "code": "CONFLICT", "message": "Product is inactive", "requestId": "r1" }
        })))
        .mount(&server)
        .await;
    let err = client(&server)
        .checkout()
        .products()
        .start_session(PID, &StartProductCheckoutParams::default())
        .await
        .unwrap_err();
    match err {
        Error::Api { code, status, .. } => {
            assert_eq!(code, "CONFLICT");
            assert_eq!(status, 409);
        }
        other => panic!("unexpected {other:?}"),
    }
}
