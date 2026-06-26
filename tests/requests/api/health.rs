use crate::common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::Service;

#[tokio::test]
async fn test_health_check() {
    let (app, _db) = common::setup_app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let mut app = app;
    let response = app.call(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response_body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(response_body["status"], "OK");
}
