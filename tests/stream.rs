use futures_util::StreamExt;
use layerhop::{Error, LayerHop};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CS: &str = "cs_01k7t9m2c4x8q6r0v3b5n7p9d";

fn session(status: &str) -> serde_json::Value {
    json!({
        "id": CS, "object": "checkout.session", "livemode": false, "status": status, "amount": "49000000",
        "settlement": { "chainId": 8453, "token": "0x83", "symbol": "USDC", "decimals": 6 },
        "recipient": "0x22", "url": format!("https://layerhop.com/pay/{CS}"), "expiresAt": "e",
        "payment": null, "createdAt": "c", "updatedAt": "u"
    })
}

fn event_json(id: &str) -> serde_json::Value {
    json!({ "id": id, "type": "order.filled", "livemode": false, "sample": true, "createdAt": "2026-10-18T10:00:00.000Z",
            "data": { "order": { "quoteId": "q_1", "status": "filled" } } })
}

/// An SSE body with comments, a ready frame, \r\n line endings and multi-line data.
fn sse_body(event: &str, payloads: &[serde_json::Value]) -> String {
    let mut s = String::from(": connected\r\n\r\nevent: ready\r\ndata: {}\r\n\r\n");
    for (i, p) in payloads.iter().enumerate() {
        if i % 2 == 0 {
            s += &format!("event: {event}\r\ndata: {p}\r\n\r\n: ping\r\n\r\n");
        } else {
            let pretty = serde_json::to_string_pretty(p).unwrap();
            s += &format!("event: {event}\n");
            for line in pretty.lines() {
                s += &format!("data: {line}\n");
            }
            s += "\n";
        }
    }
    s + "event: other\ndata: {\"ignored\":true}\n\n"
}

fn sse(body: String) -> ResponseTemplate {
    ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(body)
}

#[tokio::test]
async fn stream_order_yields_each_order() {
    let server = MockServer::start().await;
    let body = sse_body(
        "order",
        &[
            json!({ "quoteId": "q/1", "status": "submitted", "provider": "relay" }),
            json!({ "quoteId": "q/1", "status": "filled", "dstTxHash": "0xdef" }),
        ],
    );
    Mock::given(method("GET"))
        .and(path("/v1/orders/q%2F1/events"))
        .and(header("accept", "text/event-stream"))
        .and(header("x-api-key", "lh_live_k"))
        .respond_with(sse(body))
        .expect(1)
        .mount(&server)
        .await;
    let lh = LayerHop::builder().base_url(server.uri()).api_key("lh_live_k").build();
    let orders: Vec<_> = lh.stream_order("q/1").map(|r| r.unwrap()).collect().await;
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].status.as_deref(), Some("submitted"));
    assert_eq!(orders[0].provider.as_deref(), Some("relay"));
    assert_eq!(orders[1].status.as_deref(), Some("filled"));
    assert_eq!(orders[1].dst_tx_hash.as_deref(), Some("0xdef"));
}

#[tokio::test]
async fn stream_is_lazy_and_stops_early() {
    let server = MockServer::start().await;
    let body = sse_body("order", &[json!({ "status": "pending" }), json!({ "status": "filled" })]);
    Mock::given(method("GET")).and(path("/v1/orders/q_1/events")).respond_with(sse(body)).mount(&server).await;
    let lh = LayerHop::builder().base_url(server.uri()).build();
    let s = lh.stream_order("q_1");
    assert!(server.received_requests().await.unwrap().is_empty()); // sent on first poll
    let first: Vec<_> = s.take(1).collect().await;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].as_ref().unwrap().status.as_deref(), Some("pending"));
    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    assert!(!reqs[0].headers.contains_key("x-api-key"));
}

#[tokio::test]
async fn checkout_stream_yields_sessions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/checkout/sessions/{CS}/events")))
        .and(header("accept", "text/event-stream"))
        .respond_with(sse(sse_body("session", &[session("processing"), session("paid")])))
        .mount(&server)
        .await;
    let lh = LayerHop::builder().base_url(server.uri()).build();
    let sessions: Vec<_> = lh.checkout().stream(CS).map(|r| r.unwrap()).collect().await;
    let statuses: Vec<_> = sessions.iter().map(|s| s.status.as_str()).collect();
    assert_eq!(statuses, ["processing", "paid"]);
    assert!(sessions[1].is_paid());
    assert_eq!(sessions[0].settlement.decimals, 6);
}

#[tokio::test]
async fn webhooks_stream_and_trigger() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/webhooks/stream"))
        .and(header("x-api-key", "lh_test_k"))
        .and(header("accept", "text/event-stream"))
        .respond_with(sse(sse_body("event", &[event_json("evt_1"), event_json("evt_2")])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/trigger"))
        .and(header("x-api-key", "lh_test_k"))
        .and(body_json(json!({ "type": "order.filled" })))
        .respond_with(
            ResponseTemplate::new(202)
                .set_body_json(json!({ "eventId": "evt_1", "deliveries": 2, "payload": event_json("evt_1") })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let lh = LayerHop::builder().base_url(server.uri()).api_key("lh_test_k").build();
    let events: Vec<_> = lh.webhooks().stream().map(|r| r.unwrap()).collect().await;
    assert_eq!(events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["evt_1", "evt_2"]);
    let e = &events[0];
    assert_eq!(e.event_type, "order.filled");
    assert!(!e.livemode);
    assert_eq!(e.sample, Some(true));
    assert_eq!(e.data.order.as_ref().unwrap().status.as_deref(), Some("filled"));
    assert!(e.data.session.is_none());

    let r = lh.webhooks().trigger("order.filled").await.unwrap();
    assert_eq!(r.event_id, "evt_1");
    assert_eq!(r.deliveries, 2);
    assert_eq!(r.payload.event_type, "order.filled");
}

#[tokio::test]
async fn stream_404_yields_one_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/orders/q_x/events"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "error": {
            "code": "ORDER_NOT_FOUND", "message": "No order", "requestId": "r1" } })))
        .mount(&server)
        .await;
    let lh = LayerHop::builder().base_url(server.uri()).build();
    let items: Vec<_> = lh.stream_order("q_x").collect().await;
    assert_eq!(items.len(), 1);
    match &items[0] {
        Err(Error::Api { code, status, request_id, message }) => {
            assert_eq!((code.as_str(), *status, request_id.as_deref()), ("ORDER_NOT_FOUND", 404, Some("r1")));
            assert_eq!(message, "No order");
        }
        other => panic!("unexpected {other:?}"),
    }

    Mock::given(method("GET"))
        .and(path(format!("/v1/checkout/sessions/{CS}/events")))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;
    let items: Vec<_> = lh.checkout().stream(CS).collect().await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].as_ref().unwrap_err().code(), Some("INTERNAL"));
}

#[tokio::test]
async fn trigger_with_live_key_is_forbidden() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/webhooks/trigger"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "error": {
            "code": "FORBIDDEN", "message": "Use a test key", "requestId": "r3" } })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/webhooks/stream"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({ "error": {
            "code": "UNAUTHORIZED", "message": "Missing key" } })))
        .mount(&server)
        .await;
    let lh = LayerHop::builder().base_url(server.uri()).api_key("lh_live_k").build();
    let err = lh.webhooks().trigger("order.filled").await.unwrap_err();
    assert!(matches!(err, Error::Api { status: 403, .. }));
    assert_eq!(err.code(), Some("FORBIDDEN"));
    let items: Vec<_> = lh.webhooks().stream().collect().await;
    assert!(matches!(items.as_slice(), [Err(Error::Api { status: 401, .. })]));
}
