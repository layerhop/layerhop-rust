use layerhop::{
    CreateSubscriptionParams, Error, Invoice, LayerHop, ListSubscriptionsParams, UpdateSubscriptionParams,
    WebhookEvent,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SID: &str = "sub_01k7v0c2c4x8q6r0v3b5n7p9d";

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).api_key("lh_test_k").build()
}

fn subscription_json() -> serde_json::Value {
    json!({
        "id": SID, "object": "subscription", "livemode": false, "status": "active",
        "customerEmail": "ada@example.com", "customerName": "Ada Lovelace", "merchantName": "Studio Nine",
        "description": "Pro plan", "memo": null, "productId": null, "amount": "49000000",
        "settlement": { "chainId": 8453, "token": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "symbol": "USDC", "decimals": 6 },
        "recipient": "0x2222222222222222222222222222222222222222", "interval": "month", "intervalCount": 1,
        "daysUntilDue": 3, "currentPeriodStart": "2026-10-18T10:00:00.000Z",
        "currentPeriodEnd": "2026-11-18T10:00:00.000Z", "nextInvoiceAt": "2026-11-18T10:00:00.000Z",
        "cancelAt": null, "cancelAtPeriodEnd": false, "canceledAt": null, "cancellationReason": null,
        "pausedAt": null, "latestInvoiceId": "inv_1", "metadata": { "plan": "pro" },
        "createdAt": "2026-10-18T10:00:00.000Z", "updatedAt": "2026-10-18T10:00:00.000Z", "someFutureField": 1
    })
}

fn invoice_json() -> serde_json::Value {
    json!({
        "id": "inv_1", "object": "invoice", "livemode": false, "number": "INV-0001", "status": "open",
        "lineItems": [{ "description": "Pro plan (Oct 18 – Nov 18, 2026)", "quantity": 1, "unitAmount": "49000000" }],
        "amount": "49000000",
        "settlement": { "chainId": 8453, "token": "0x83", "symbol": "USDC", "decimals": 6 },
        "url": "https://layerhop.com/invoice/inv_1", "createdAt": "c", "customerEmail": "ada@example.com",
        "recipient": "0x2222", "sessionId": null, "subscriptionId": SID, "updatedAt": "u"
    })
}

fn with(changes: serde_json::Value) -> serde_json::Value {
    let mut v = subscription_json();
    for (k, val) in changes.as_object().unwrap() {
        v[k] = val.clone();
    }
    v
}

#[tokio::test]
async fn create_sends_body_and_parses_subscription() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/subscriptions"))
        .and(header("x-api-key", "lh_test_k"))
        .and(body_json(json!({
            "customerEmail": "ada@example.com", "interval": "month", "intervalCount": 1, "amount": "49000000",
            "settleChain": "base", "settleToken": "USDC", "recipient": "0x2222",
            "description": "Pro plan", "customerName": "Ada Lovelace", "daysUntilDue": 3,
            "metadata": { "plan": "pro" }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(subscription_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/subscriptions"))
        .and(body_json(json!({ "customerEmail": "bob@example.com", "interval": "year", "productId": "prod_1" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(with(json!({ "productId": "prod_1" }))))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let s = lh
        .subscriptions()
        .create(&CreateSubscriptionParams {
            customer_email: "ada@example.com".into(),
            interval: "month".into(),
            interval_count: Some(1),
            amount: Some("49000000".into()),
            settle_chain: Some("base".into()),
            settle_token: Some("USDC".into()),
            recipient: Some("0x2222".into()),
            description: Some("Pro plan".into()),
            customer_name: Some("Ada Lovelace".into()),
            days_until_due: Some(3),
            metadata: Some([("plan".to_string(), "pro".to_string())].into_iter().collect()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(s.id, SID);
    assert_eq!(s.object, "subscription");
    assert!(s.is_active());
    assert!(!s.livemode);
    assert_eq!(s.amount, "49000000");
    assert_eq!(s.settlement.symbol, "USDC");
    assert_eq!(s.interval, "month");
    assert_eq!(s.interval_count, 1);
    assert_eq!(s.days_until_due, 3);
    assert_eq!(s.next_invoice_at.as_deref(), Some("2026-11-18T10:00:00.000Z"));
    assert!(!s.cancel_at_period_end);
    assert_eq!(s.latest_invoice_id.as_deref(), Some("inv_1"));
    assert_eq!(s.metadata.as_ref().unwrap()["plan"], "pro");
    assert_eq!(s.memo, None);

    let p = lh
        .subscriptions()
        .create(&CreateSubscriptionParams {
            customer_email: "bob@example.com".into(),
            interval: "year".into(),
            product_id: Some("prod_1".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(p.product_id.as_deref(), Some("prod_1"));
}

#[tokio::test]
async fn get_list_update() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/subscriptions/{SID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(subscription_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/subscriptions"))
        .and(query_param("status", "past_due"))
        .and(query_param("customerEmail", "ada@example.com"))
        .and(query_param("cursor", "c1"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "subscriptions": [with(json!({ "status": "past_due" }))], "nextCursor": "c2"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/subscriptions"))
        .and(query_param_is_missing("status"))
        .and(query_param_is_missing("customerEmail"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "subscriptions": [], "nextCursor": null })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/subscriptions/{SID}")))
        .and(body_json(json!({ "memo": null, "amount": "59000000", "metadata": null, "cancelAtPeriodEnd": true })))
        .respond_with(ResponseTemplate::new(200).set_body_json(with(json!({
            "amount": "59000000", "metadata": null, "cancelAtPeriodEnd": true, "cancelAt": "2026-11-18T10:00:00.000Z"
        }))))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    assert_eq!(lh.subscriptions().get(SID).await.unwrap().customer_email, "ada@example.com");
    let page = lh
        .subscriptions()
        .list(&ListSubscriptionsParams {
            status: Some("past_due".into()),
            customer_email: Some("ada@example.com".into()),
            cursor: Some("c1".into()),
            limit: Some(10),
        })
        .await
        .unwrap();
    assert_eq!(page.subscriptions[0].status, "past_due");
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));
    let empty = lh.subscriptions().list(&ListSubscriptionsParams::default()).await.unwrap();
    assert!(empty.subscriptions.is_empty() && empty.next_cursor.is_none());
    let u = lh
        .subscriptions()
        .update(
            SID,
            &UpdateSubscriptionParams {
                memo: Some(None),
                amount: Some("59000000".into()),
                metadata: Some(None),
                cancel_at_period_end: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(u.amount, "59000000");
    assert!(u.cancel_at_period_end);
    assert_eq!(u.cancel_at.as_deref(), Some("2026-11-18T10:00:00.000Z"));
    assert!(u.metadata.is_none());
}

#[tokio::test]
async fn cancel_pause_resume_invoices() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/subscriptions/sub_1/cancel"))
        .and(body_json(json!({ "atPeriodEnd": false })))
        .respond_with(ResponseTemplate::new(200).set_body_json(with(json!({
            "status": "canceled", "canceledAt": "t", "cancellationReason": "requested", "nextInvoiceAt": null
        }))))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/subscriptions/sub_2/cancel"))
        .and(body_json(json!({ "atPeriodEnd": true })))
        .respond_with(ResponseTemplate::new(200).set_body_json(with(json!({ "cancelAtPeriodEnd": true }))))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/subscriptions/sub_1/pause"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(with(json!({
            "status": "paused", "pausedAt": "t", "nextInvoiceAt": null
        }))))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/subscriptions/sub_1/resume"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(subscription_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/subscriptions/sub_1/invoices"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "invoices": [invoice_json()] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/subscriptions/sub_3/pause"))
        .respond_with(ResponseTemplate::new(409).set_body_json(
            json!({ "error": { "code": "CONFLICT", "message": "Only an active subscription can be paused", "requestId": "r1" } }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let c = lh.subscriptions().cancel("sub_1", false).await.unwrap();
    assert_eq!(c.status, "canceled");
    assert_eq!(c.cancellation_reason.as_deref(), Some("requested"));
    assert!(c.next_invoice_at.is_none());
    assert!(lh.subscriptions().cancel("sub_2", true).await.unwrap().cancel_at_period_end);
    let p = lh.subscriptions().pause("sub_1").await.unwrap();
    assert_eq!(p.status, "paused");
    assert_eq!(p.paused_at.as_deref(), Some("t"));
    assert!(lh.subscriptions().resume("sub_1").await.unwrap().is_active());
    let invoices = lh.subscriptions().invoices("sub_1").await.unwrap();
    assert_eq!(invoices.len(), 1);
    assert_eq!(invoices[0].subscription_id.as_deref(), Some(SID));
    match lh.subscriptions().pause("sub_3").await.unwrap_err() {
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
        .and(path("/v1/subscriptions/a%2Fb"))
        .respond_with(ResponseTemplate::new(200).set_body_json(subscription_json()))
        .expect(1)
        .mount(&server)
        .await;
    client(&server).subscriptions().get("a/b").await.unwrap();
}

#[test]
fn invoice_subscription_id_and_events_parse() {
    let i: Invoice = serde_json::from_value(invoice_json()).unwrap();
    assert_eq!(i.subscription_id.as_deref(), Some(SID));
    let mut plain = invoice_json();
    plain.as_object_mut().unwrap().remove("subscriptionId");
    let i: Invoice = serde_json::from_value(plain).unwrap();
    assert_eq!(i.subscription_id, None);

    let e: WebhookEvent = serde_json::from_value(json!({
        "id": "evt_1", "type": "subscription.renewed", "livemode": false, "createdAt": "t",
        "data": { "subscription": subscription_json(), "invoice": invoice_json() }
    }))
    .unwrap();
    assert_eq!(e.event_type, "subscription.renewed");
    assert_eq!(e.data.subscription.unwrap().id, SID);
    assert_eq!(e.data.invoice.unwrap().subscription_id.as_deref(), Some(SID));
}
