use layerhop::{
    CreateInvoiceParams, CreateRefundParams, Error, InvoiceLineItem, LayerHop, ListInvoicesParams, Order,
    UpdateInvoiceParams, WebhookEvent,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const IID: &str = "inv_01m57hq2c4x8q6r0v3b5n7p9d";

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).api_key("lh_test_k").build()
}

fn invoice_json() -> serde_json::Value {
    json!({
        "id": IID, "object": "invoice", "livemode": true, "number": "INV-0001", "status": "open",
        "merchantName": "Acme", "customerName": "Jane", "memo": null,
        "lineItems": [
            { "description": "Design work", "quantity": 2, "unitAmount": "100000000" },
            { "description": "Hosting", "quantity": 1, "unitAmount": "25000000" }
        ],
        "amount": "225000000",
        "settlement": { "chainId": 8453, "token": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "symbol": "USDC", "decimals": 6 },
        "dueAt": "2026-11-01T23:59:59.000Z", "paidAt": null, "url": format!("https://layerhop.com/invoice/{IID}"),
        "receiptUrl": null, "createdAt": "2026-10-18T10:00:00.000Z", "customerEmail": "jane@example.com",
        "recipient": "0x2222222222222222222222222222222222222222", "sessionId": null,
        "sentAt": "2026-10-18T10:00:00.000Z", "receiptSentAt": null, "voidedAt": null,
        "updatedAt": "2026-10-18T10:00:00.000Z", "someFutureField": 1
    })
}

fn refund_json(status: &str) -> serde_json::Value {
    json!({
        "id": "re_1", "object": "refund", "sessionId": "cs_1", "livemode": true, "status": status, "statusDetail": null,
        "amount": "10000000", "reason": "Damaged", "quoteId": "q_r1",
        "to": { "chainId": 42161, "token": "0x0000000000000000000000000000000000000000", "address": "0x1111" },
        "txHash": null, "createdAt": "2026-10-18T12:00:00.000Z", "updatedAt": "2026-10-18T12:00:00.000Z"
    })
}

#[tokio::test]
async fn create_sends_body_and_parses_invoice() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/invoices"))
        .and(header("x-api-key", "lh_test_k"))
        .and(body_json(json!({
            "customerEmail": "jane@example.com", "customerName": "Jane",
            "lineItems": [
                { "description": "Design work", "quantity": 2, "unitAmount": "100000000" },
                { "description": "Hosting", "quantity": 1, "unitAmount": "25000000" }
            ],
            "settleChain": "base", "settleToken": "USDC", "recipient": "0x2222", "dueAt": "2026-11-01", "send": true
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(invoice_json()))
        .expect(1)
        .mount(&server)
        .await;
    let i = client(&server)
        .invoices()
        .create(&CreateInvoiceParams {
            customer_email: Some("jane@example.com".into()),
            customer_name: Some("Jane".into()),
            line_items: vec![
                InvoiceLineItem { description: "Design work".into(), quantity: 2, unit_amount: "100000000".into() },
                InvoiceLineItem { description: "Hosting".into(), quantity: 1, unit_amount: "25000000".into() },
            ],
            settle_chain: "base".into(),
            settle_token: "USDC".into(),
            recipient: "0x2222".into(),
            due_at: Some("2026-11-01".into()),
            send: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(i.id, IID);
    assert_eq!(i.status, "open");
    assert!(!i.is_paid());
    assert_eq!(i.number, "INV-0001");
    assert_eq!(i.amount, "225000000");
    assert_eq!(i.line_items.len(), 2);
    assert_eq!(i.settlement.symbol, "USDC");
    assert_eq!(i.memo, None);
    assert_eq!(i.url.as_deref(), Some(format!("https://layerhop.com/invoice/{IID}").as_str()));
    assert_eq!(i.customer_email.as_deref(), Some("jane@example.com"));
    assert_eq!(i.session_id, None);
}

#[tokio::test]
async fn get_list_update_send_void() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/invoices/inv_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(invoice_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/invoices"))
        .and(query_param("status", "paid"))
        .and(query_param("cursor", "c1"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "invoices": [invoice_json()], "nextCursor": "c2" })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/invoices"))
        .and(query_param_is_missing("status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "invoices": [], "nextCursor": null })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/invoices/inv_1"))
        .and(body_json(json!({
            "customerName": null, "memo": "Thanks!", "dueAt": null, "settleChain": "arbitrum",
            "lineItems": [{ "description": "Audit", "quantity": 1, "unitAmount": "5" }]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut v = invoice_json();
            v["status"] = json!("draft");
            v
        }))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/invoices/inv_1/send"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(invoice_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/invoices/inv_1/void"))
        .respond_with(ResponseTemplate::new(409).set_body_json(
            json!({ "error": { "code": "CONFLICT", "message": "This invoice is paid", "requestId": "r1" } }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    assert_eq!(lh.invoices().get("inv_1").await.unwrap().number, "INV-0001");
    let page = lh
        .invoices()
        .list(&ListInvoicesParams { status: Some("paid".into()), cursor: Some("c1".into()), limit: Some(10) })
        .await
        .unwrap();
    assert_eq!(page.invoices.len(), 1);
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));
    let empty = lh.invoices().list(&ListInvoicesParams::default()).await.unwrap();
    assert!(empty.invoices.is_empty() && empty.next_cursor.is_none());
    let draft = lh
        .invoices()
        .update(
            "inv_1",
            &UpdateInvoiceParams {
                customer_name: Some(None),
                memo: Some(Some("Thanks!".into())),
                due_at: Some(None),
                settle_chain: Some("arbitrum".into()),
                line_items: Some(vec![InvoiceLineItem {
                    description: "Audit".into(),
                    quantity: 1,
                    unit_amount: "5".into(),
                }]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(draft.status, "draft");
    assert_eq!(lh.invoices().send("inv_1").await.unwrap().status, "open");
    match lh.invoices().void("inv_1").await.unwrap_err() {
        Error::Api { code, status, .. } => {
            assert_eq!(code, "CONFLICT");
            assert_eq!(status, 409);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn public_view_and_pay() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/invoices/inv_1/public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "inv_1", "object": "invoice", "livemode": false, "number": "INV-0001", "status": "paid",
            "merchantName": null, "customerName": null, "memo": null,
            "lineItems": [{ "description": "x", "quantity": 1, "unitAmount": "1" }], "amount": "1",
            "settlement": { "chainId": 8453, "token": "0x83", "symbol": "USDC", "decimals": 6 },
            "dueAt": null, "paidAt": "2026-10-18T11:00:00.000Z", "url": "u",
            "receiptUrl": "https://api.layerhop.com/v1/invoices/inv_1/receipt.pdf", "createdAt": "c"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/invoices/inv_1/pay"))
        .and(body_json(json!({})))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(json!({ "sessionId": "cs_test_1", "url": "https://layerhop.com/pay/cs_test_1" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let lh = client(&server);
    let i = lh.invoices().public_view("inv_1").await.unwrap();
    assert!(i.is_paid());
    assert!(i.receipt_url.is_some());
    assert_eq!(i.recipient, None);
    assert_eq!(i.customer_email, None);
    let p = lh.invoices().pay("inv_1").await.unwrap();
    assert_eq!(p.session_id, "cs_test_1");
    assert_eq!(p.url, "https://layerhop.com/pay/cs_test_1");
}

#[tokio::test]
async fn refunds_create_and_list() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions/cs_1/refunds"))
        .and(header("x-api-key", "lh_test_k"))
        .and(body_json(json!({ "amount": "10000000", "reason": "Damaged", "to": "original" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "refund": refund_json("pending"),
            "quote": {
                "quoteId": "q_r1", "provider": "relay", "srcAmount": "10000000", "dstAmount": "3100000000000000",
                "dstAmountMin": "3000000000000000", "estimatedSeconds": 20, "expiresAt": 1760792400u64, "from": "0x2222"
            },
            "next": "Build the transaction..."
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/checkout/sessions/cs_1/refunds"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "refunds": [refund_json("completed")] })))
        .expect(1)
        .mount(&server)
        .await;
    let lh = client(&server);
    let r = lh
        .checkout()
        .refunds()
        .create(
            "cs_1",
            &CreateRefundParams {
                amount: Some("10000000".into()),
                reason: Some("Damaged".into()),
                to: Some("original".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.refund.id, "re_1");
    assert_eq!(r.refund.status, "pending");
    assert_eq!(r.refund.to.chain_id, 42161);
    assert_eq!(r.refund.tx_hash, None);
    assert_eq!(r.quote.quote_id, "q_r1");
    assert_eq!(r.quote.from, "0x2222");
    assert_eq!(r.quote.expires_at, 1760792400);
    assert!(!r.next.is_empty());
    let l = lh.checkout().refunds().list("cs_1").await.unwrap();
    assert_eq!(l.refunds.len(), 1);
    assert_eq!(l.refunds[0].status, "completed");
}

#[test]
fn new_fields_and_events_parse() {
    let o: Order = serde_json::from_value(json!({ "quoteId": "q_r1", "refundId": "re_1" })).unwrap();
    assert_eq!(o.refund_id.as_deref(), Some("re_1"));
    let session = json!({
        "id": "cs_1", "object": "checkout.session", "livemode": true, "status": "paid", "amount": "49000000",
        "settlement": { "chainId": 8453, "token": "0x83", "symbol": "USDC", "decimals": 6 },
        "recipient": "0x2222", "url": "u", "invoiceId": "inv_1", "refundedAmount": "10000000",
        "expiresAt": "e", "payment": null, "createdAt": "c", "updatedAt": "u"
    });
    let e: WebhookEvent = serde_json::from_value(json!({
        "id": "evt_1", "type": "checkout.session.refunded", "livemode": true, "createdAt": "t",
        "data": { "session": session, "refund": refund_json("completed") }
    }))
    .unwrap();
    let s = e.data.session.unwrap();
    assert_eq!(s.invoice_id.as_deref(), Some("inv_1"));
    assert_eq!(s.refunded_amount.as_deref(), Some("10000000"));
    assert_eq!(e.data.refund.unwrap().amount, "10000000");
    // invoice.paid carries an invoice summary (no line items or url).
    let e: WebhookEvent = serde_json::from_value(json!({
        "id": "evt_2", "type": "invoice.paid", "livemode": true, "createdAt": "t",
        "data": {
            "invoice": {
                "id": "inv_1", "object": "invoice", "livemode": true, "number": "INV-0001", "status": "paid",
                "amount": "1", "settlement": { "chainId": 8453, "token": "0x83", "symbol": "USDC", "decimals": 6 },
                "customerEmail": null, "customerName": null, "sessionId": "cs_1", "paidAt": "p"
            },
            "session": null
        }
    }))
    .unwrap();
    let inv = e.data.invoice.unwrap();
    assert!(inv.is_paid());
    assert!(inv.line_items.is_empty() && inv.url.is_none());
    assert_eq!(inv.session_id.as_deref(), Some("cs_1"));
}

#[tokio::test]
async fn service_status_parses() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "degraded", "updatedAt": "2026-10-18T12:00:00.000Z",
            "components": [
                { "id": "api", "name": "API", "group": "Core", "status": "operational", "latencyMs": 42, "uptime90d": 99.98,
                  "days": [{ "date": "2026-10-17", "uptime": null }, { "date": "2026-10-18", "uptime": 100 }] },
                { "id": "provider:relay", "name": "Relay", "group": "Liquidity providers", "status": "degraded",
                  "latencyMs": null, "uptime90d": null, "days": [] }
            ],
            "incidents": [{ "id": "inc_1", "componentId": "provider:relay", "component": "Relay",
                "title": "Relay quotes failing", "startedAt": "2026-10-18T11:00:00.000Z", "resolvedAt": null }],
            "url": "https://layerhop.com/status"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let s = client(&server).service_status().await.unwrap();
    assert_eq!(s.status, "degraded");
    assert_eq!(s.components.len(), 2);
    assert_eq!(s.components[0].latency_ms, Some(42));
    assert_eq!(s.components[0].uptime_90d, Some(99.98));
    assert_eq!(s.components[0].days[0].uptime, None);
    assert_eq!(s.components[0].days[1].uptime, Some(100.0));
    assert_eq!(s.components[1].latency_ms, None);
    assert_eq!(s.incidents[0].resolved_at, None);
    assert_eq!(s.url, "https://layerhop.com/status");
}
