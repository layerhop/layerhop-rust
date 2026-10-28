use layerhop::{
    CreateSplitParams, LayerHop, PayoutPlanParams, PayoutRow, PayoutTransfer, PayoutTransfersParams, SplitShareInput,
    UpdateSplitParams,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).build()
}

const SID: &str = "spl_01k8x2m3n4p5q6r7s8t9v0w1x2";

fn split_json() -> serde_json::Value {
    json!({
        "id": SID, "url": format!("https://layerhop.com/split/{SID}"), "status": "open", "note": "Dinner",
        "recipient": "0x1111111111111111111111111111111111111111", "recipientName": "alice.eth",
        "chainId": 8453, "chain": "base",
        "token": { "address": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "symbol": "USDC", "decimals": 6 },
        "total": "120000000", "collected": "40000000", "paidCount": 1,
        "shares": [
            { "index": 0, "label": "Bob", "amount": "40000000", "received": "40000000", "status": "paid", "payments": 1, "pending": 0 },
            { "index": 1, "label": "", "amount": "40000000", "received": "0", "status": "unpaid", "payments": 0, "pending": 1 },
            { "index": 2, "label": "", "amount": "40000000", "received": "0", "status": "unpaid", "payments": 0, "pending": 0 }
        ],
        "createdAt": "2026-10-28T12:00:00.000Z", "updatedAt": "2026-10-28T12:00:00.000Z"
    })
}

#[tokio::test]
async fn splits_create_get_update() {
    let server = MockServer::start().await;
    let mut created = split_json();
    created["editToken"] = json!("secret123");
    created["editUrl"] = json!(format!("https://layerhop.com/split/{SID}#k=secret123"));
    Mock::given(method("POST"))
        .and(path("/v1/splits"))
        .and(body_json(json!({
            "recipient": "alice.eth", "chain": "base", "token": "USDC", "total": "120000000",
            "shares": [{ "label": "Bob" }, {}, { "label": "Carol", "amount": "1" }], "note": "Dinner"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(created))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/splits/{SID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(split_json()))
        .mount(&server)
        .await;
    let mut closed = split_json();
    closed["status"] = json!("closed");
    closed["note"] = json!(null);
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/splits/{SID}")))
        .and(header("x-edit-token", "secret123"))
        .and(body_json(json!({ "note": null, "labels": ["Bob", "Carol", "Dan"], "status": "closed" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(closed))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let c = lh
        .splits()
        .create(&CreateSplitParams {
            recipient: "alice.eth".into(),
            chain: "base".into(),
            token: "USDC".into(),
            total: "120000000".into(),
            shares: Some(vec![
                SplitShareInput { label: Some("Bob".into()), amount: None },
                SplitShareInput::default(),
                SplitShareInput { label: Some("Carol".into()), amount: Some("1".into()) },
            ]),
            note: Some("Dinner".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(c.edit_token, "secret123");
    assert!(c.edit_url.ends_with("#k=secret123"));
    assert_eq!(c.split.id, SID);
    assert_eq!(c.split.recipient_name.as_deref(), Some("alice.eth"));

    let s = lh.splits().get(SID).await.unwrap();
    assert_eq!(s.token.decimals, 6);
    assert_eq!(s.shares.len(), 3);
    assert_eq!(s.shares[0].status, "paid");
    assert_eq!(s.shares[1].pending, 1);
    assert_eq!(s.paid_count, 1);

    let u = lh
        .splits()
        .update(
            SID,
            "secret123",
            &UpdateSplitParams {
                note: Some(None),
                labels: Some(vec!["Bob".into(), "Carol".into(), "Dan".into()]),
                status: Some("closed".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(u.status, "closed");
    assert_eq!(u.note, None);
}

#[tokio::test]
async fn tip_jar() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/tip-jars/0xabc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "address": "0xabc", "tippers": 3, "tips": 5,
            "messages": [{ "text": "gm", "at": "2026-10-28T12:00:00.000Z" }]
        })))
        .mount(&server)
        .await;
    let t = client(&server).get_tip_jar("0xabc").await.unwrap();
    assert_eq!((t.tippers, t.tips), (3, 5));
    assert_eq!(t.messages[0].text, "gm");
}

#[tokio::test]
async fn payout_plan_and_transfers() {
    let server = MockServer::start().await;
    let usdc = json!({ "chainId": 8453, "address": "0x8335", "symbol": "USDC", "decimals": 6, "usd": 1.0 });
    Mock::given(method("POST"))
        .and(path("/v1/payouts/plan"))
        .and(body_json(json!({
            "srcChain": "base", "srcToken": "USDC", "sender": "0xAbC1",
            "rows": [{ "recipient": "alice.eth", "amount": "250", "token": "USDC", "chain": "base", "label": "March" },
                     { "recipient": "bad", "amount": "1", "token": "USDC", "chain": "nowhere" }]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "source": { "chainId": 8453, "chain": "base", "sender": "0xAbC1", "token": usdc },
            "rows": [
                { "index": 0, "line": 0, "input": { "recipient": "alice.eth", "amount": "250", "token": "USDC", "chain": "base" },
                  "ok": true, "recipient": { "address": "0x1111", "name": "alice.eth" }, "chainId": 8453, "chain": "base",
                  "token": usdc, "amount": "250000000", "kind": "transfer" },
                { "index": 1, "line": 0, "input": { "recipient": "bad", "amount": 1, "token": "USDC", "chain": "nowhere" },
                  "ok": false, "error": { "code": "UNSUPPORTED_CHAIN", "message": "Unknown chain" } }
            ],
            "transfers": [{ "chainId": 8453, "token": "0x8335", "rows": [0], "total": "250000000" }],
            "routes": [],
            "transferTotal": "250000000",
            "totals": [{ "chainId": 8453, "token": "0x8335", "symbol": "USDC", "decimals": 6, "amount": "250000000", "formatted": "250", "rows": 1 }],
            "valid": 1, "invalid": 1, "warnings": []
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/payouts/transfers"))
        .and(body_json(json!({
            "chain": "base", "token": "USDC", "sender": "0xAbC1",
            "transfers": [{ "recipient": "0x1111", "amount": "250000000" }]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "vm": "evm", "chainId": 8453, "token": usdc,
            "calls": [{ "to": "0x8335", "data": "0xa9059cbb", "value": "0" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let plan = lh
        .payouts()
        .plan(&PayoutPlanParams {
            src_chain: "base".into(),
            src_token: "USDC".into(),
            sender: "0xAbC1".into(),
            rows: Some(vec![
                PayoutRow {
                    recipient: "alice.eth".into(),
                    amount: "250".into(),
                    token: "USDC".into(),
                    chain: "base".into(),
                    label: Some("March".into()),
                },
                PayoutRow {
                    recipient: "bad".into(),
                    amount: "1".into(),
                    token: "USDC".into(),
                    chain: "nowhere".into(),
                    label: None,
                },
            ]),
            csv: None,
        })
        .await
        .unwrap();
    assert_eq!(plan.source.token.symbol, "USDC");
    assert_eq!(plan.valid, 1);
    assert_eq!(plan.rows[0].kind.as_deref(), Some("transfer"));
    assert_eq!(plan.rows[0].recipient.as_ref().unwrap().name.as_deref(), Some("alice.eth"));
    assert_eq!(plan.rows[1].error.as_ref().unwrap().code, "UNSUPPORTED_CHAIN");
    assert_eq!(plan.rows[1].input["amount"], 1);
    assert_eq!(plan.transfers[0].rows, vec![0]);
    assert_eq!(plan.totals[0].formatted, "250");

    let t = lh
        .payouts()
        .transfers(&PayoutTransfersParams {
            chain: "base".into(),
            token: "USDC".into(),
            sender: "0xAbC1".into(),
            transfers: vec![PayoutTransfer { recipient: "0x1111".into(), amount: "250000000".into() }],
        })
        .await
        .unwrap();
    assert_eq!(t.vm, "evm");
    assert_eq!(t.calls.unwrap()[0].data, "0xa9059cbb");
    assert_eq!(t.serialized_tx, None);
}
