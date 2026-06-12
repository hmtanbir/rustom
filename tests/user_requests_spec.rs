mod common;

use axum::{body::Body, http::{Request, StatusCode}};
use tower::Service;
use serde_json::{json, Value};
use jsonwebtoken::{encode, EncodingKey, Header};
use chrono::{Utc, Duration};
use rustom::models::user::Claims;

fn generate_test_token(user_id: uuid::Uuid, role: i32) -> String {
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret_for_tests_123456789".to_string());
    let expiration = Utc::now()
        .checked_add_signed(Duration::seconds(3600))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        user_id,
        role,
        exp: expiration as u64,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

#[tokio::test]
async fn test_user_registration() {
    let (app, _db) = common::setup_app().await;
    let gateway_key = common::get_gateway_key();

    let email = format!("test_{}@example.com", uuid::Uuid::new_v4());
    
    let payload = json!({
        "user": {
            "name": "Test User",
            "email": email,
            "password": "password123"
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/registration")
        .header("Content-Type", "application/json")
        .header("x-api-gateway-key", &gateway_key)
        .body(Body::from(payload.to_string()))
        .unwrap();

    let mut app = app;
    let response = app.call(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_get_me_authenticated() {
    let (app, db) = common::setup_app().await;
    let gateway_key = common::get_gateway_key();

    let user_id = uuid::Uuid::new_v4();
    let email = format!("me_{}@example.com", user_id);
    let password_hash = "$argon2id$v=19$m=19456,t=2,p=1$mIk38++6ZCEyzKo+edgXEw$/h0anRjDkzS46suJM6/P3+DySS3qp1+6jXtNjd6UMTs";
    
    sqlx::query!(
        r#"
        INSERT INTO users (id, name, email, password_digest, role, status)
        VALUES ($1, 'Me User', $2, $3, 1, 1)
        "#,
        user_id, email, password_hash
    ).execute(&db).await.unwrap();

    let token = generate_test_token(user_id, 1);

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/users/me")
        .header("x-api-gateway-key", &gateway_key)
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let mut app = app;
    let response = app.call(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_body: Value = serde_json::from_slice(&body_bytes).unwrap();
    
    assert_eq!(response_body["data"]["email"], email);
}

#[tokio::test]
async fn test_get_users_as_admin() {
    let (app, _db) = common::setup_app().await;
    let gateway_key = common::get_gateway_key();

    let admin_id = uuid::Uuid::new_v4();
    let token = generate_test_token(admin_id, 0); // 0 = Admin

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/users")
        .header("x-api-gateway-key", &gateway_key)
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let mut app = app;
    let response = app.call(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_users_as_standard_user_is_forbidden() {
    let (app, _db) = common::setup_app().await;
    let gateway_key = common::get_gateway_key();

    let user_id = uuid::Uuid::new_v4();
    let token = generate_test_token(user_id, 1); // 1 = Standard User

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/users")
        .header("x-api-gateway-key", &gateway_key)
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let mut app = app;
    let response = app.call(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
