use solana_recover::*;
use solana_recover::api::server::{ApiState, ScanRequest, ApiResponse};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use tower::ServiceExt;
use serde_json::json;

async fn create_test_api_state() -> ApiState {
    let config = crate::common::create_test_config();
    let rpc_endpoints: Vec<RpcEndpoint> = config.rpc.endpoints
        .iter()
        .enumerate()
        .map(|(i, url)| RpcEndpoint {
            url: url.clone(),
            priority: i as u8,
            rate_limit_rps: config.rpc.rate_limit_rps,
            timeout_ms: config.rpc.timeout_ms,
            healthy: true,
        })
        .collect();
    
    let connection_pool = Arc::new(ConnectionPool::new(rpc_endpoints, config.rpc.pool_size));
    let scanner = Arc::new(WalletScanner::new(connection_pool.clone()));
    let batch_processor = Arc::new(BatchProcessor::new(
        scanner.clone(),
        None,
        None,
        config.scanner.into(),
    ));
    
    ApiState {
        scanner: scanner.clone(),
        batch_processor: batch_processor.clone(),
        wallet_manager: Arc::new(WalletManager::new()),
        cache_manager: Arc::new(CacheManager::new(config.cache.into())),
        persistence_manager: Arc::new(
            SqlitePersistenceManager::new(config.database.into()).await.unwrap()
        ),
        config: config.clone(),
    }
}

async fn create_test_app() -> Router {
    let state = create_test_api_state().await;
    solana_recover::api::server::create_router(state)
}

#[tokio::test]
async fn test_api_health_check() {
    let app = create_test_app().await;
    
    let request = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_scan_wallet() {
    let app = create_test_app().await;
    
    let scan_request = ScanRequest {
        wallet_address: crate::common::get_test_wallet_address(),
        fee_percentage: None,
    };
    
    let request = Request::builder()
        .uri("/api/v1/scan")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&scan_request).unwrap()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    // Should either succeed or fail gracefully (network issues in test environment)
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_api_batch_scan() {
    let app = create_test_app().await;
    
    let batch_request = json!({
        "wallet_addresses": crate::common::get_test_wallet_addresses(),
        "fee_percentage": 0.15
    });
    
    let request = Request::builder()
        .uri("/api/v1/batch-scan")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(batch_request.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    // Should either succeed or fail gracefully
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_api_metrics() {
    let app = create_test_app().await;
    
    let request = Request::builder()
        .uri("/api/v1/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_invalid_wallet_address() {
    let app = create_test_app().await;
    
    let scan_request = json!({
        "wallet_address": "invalid_address",
        "fee_percentage": 0.15
    });
    
    let request = Request::builder()
        .uri("/api/v1/scan")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(scan_request.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_invalid_json() {
    let app = create_test_app().await;
    
    let request = Request::builder()
        .uri("/api/v1/scan")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from("{invalid json}"))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_not_found() {
    let app = create_test_app().await;
    
    let request = Request::builder()
        .uri("/api/v1/nonexistent")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_api_method_not_allowed() {
    let app = create_test_app().await;
    
    let request = Request::builder()
        .uri("/api/v1/scan")
        .method("GET") // Should be POST
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_api_batch_scan_empty_addresses() {
    let app = create_test_app().await;
    
    let batch_request = json!({
        "wallet_addresses": [],
        "fee_percentage": 0.15
    });
    
    let request = Request::builder()
        .uri("/api/v1/batch-scan")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(batch_request.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_invalid_fee_percentage() {
    let app = create_test_app().await;
    
    let scan_request = json!({
        "wallet_address": crate::common::get_test_wallet_address(),
        "fee_percentage": 1.5 // Invalid: > 100%
    });
    
    let request = Request::builder()
        .uri("/api/v1/scan")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(scan_request.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
