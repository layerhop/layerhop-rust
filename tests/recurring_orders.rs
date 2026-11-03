use layerhop::{CreateRecurringOrderParams, LayerHop, ListRecurringOrdersParams};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const WALLET: &str = "5tzFkiKscXHK5ZXCGbXZxdw7gTjjD1mBwuoFbhUvuAi9";
const BONK: &str = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263";
const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

fn client(server: &MockServer) -> LayerHop {
    LayerHop::builder().base_url(server.uri()).build()
}

#[tokio::test]
async fn recurring_orders_create_submit_list_cancel() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/recurring-orders"))
        .and(body_json(json!({
            "wallet": WALLET, "dstToken": BONK, "amount": "350000000", "orders": 7, "intervalSeconds": 86400,
            "srcToken": "USDC"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "object": "recurring_order_build", "livemode": true, "provider": "jupiter", "chainId": 7565164,
            "wallet": WALLET, "srcToken": USDC, "dstToken": BONK, "amount": "350000000", "amountPerOrder": "50000000",
            "orders": 7, "intervalSeconds": 86400, "fees": { "ourFeeBps": 0, "integratorFeeBps": 0 },
            "requestId": "req-1", "tx": { "vm": "svm", "serializedTx": "AQAA", "submitViaApi": true }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/recurring-orders/submit"))
        .and(body_json(json!({ "requestId": "req-1", "signedTransaction": "c2ln" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "txHash": "5sig", "order": "G1wn" })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/recurring-orders"))
        .and(query_param("wallet", WALLET))
        .and(query_param("status", "history"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "recurringOrders": [{
                "id": "G1wn", "object": "recurring_order", "provider": "jupiter", "chainId": 7565164, "status": "active",
                "wallet": WALLET, "srcToken": USDC, "dstToken": BONK, "amount": "350000000", "amountPerOrder": "50000000",
                "orders": 7, "ordersFilled": 1, "intervalSeconds": 86400, "spentAmount": "50000000", "withdrawnAmount": "0",
                "receivedAmount": "123", "openTxHash": "5sig", "closeTxHash": null, "createdAt": "2026-10-28T12:00:00Z",
                "fills": [{ "txHash": "f1", "amount": "50000000", "dstAmount": "123", "feeAmount": "50", "feeToken": null, "filledAt": null }]
            }],
            "page": 2, "totalPages": 3
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/recurring-orders/G1wn/cancel"))
        .and(body_json(json!({ "wallet": WALLET })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "requestId": "req-2", "tx": { "vm": "svm", "serializedTx": "AQBB", "submitViaApi": true }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let lh = client(&server);
    let ro = lh.recurring_orders();
    let b = ro
        .create(&CreateRecurringOrderParams {
            wallet: WALLET.into(),
            dst_token: BONK.into(),
            amount: "350000000".into(),
            orders: 7,
            interval_seconds: 86400,
            src_token: Some("USDC".into()),
            chain: None,
        })
        .await
        .unwrap();
    assert_eq!(b.request_id, "req-1");
    assert_eq!(b.amount_per_order, "50000000");
    assert_eq!(b.tx.submit_via_api, Some(true));
    assert_eq!(b.fees.our_fee_bps, 0);

    let s = ro.submit(&b.request_id, "c2ln").await.unwrap();
    assert_eq!(s.tx_hash, "5sig");
    assert_eq!(s.order.as_deref(), Some("G1wn"));

    let l = ro
        .list(&ListRecurringOrdersParams { wallet: WALLET.into(), status: Some("history".into()), page: Some(2) })
        .await
        .unwrap();
    assert_eq!((l.page, l.total_pages), (2, 3));
    let o = &l.recurring_orders[0];
    assert_eq!(o.orders_filled, 1);
    assert_eq!(o.close_tx_hash, None);
    assert_eq!(o.fills[0].fee_token, None);

    let c = ro.cancel("G1wn", WALLET).await.unwrap();
    assert_eq!(c.request_id, "req-2");
    assert_eq!(c.tx.serialized_tx, "AQBB");
}
