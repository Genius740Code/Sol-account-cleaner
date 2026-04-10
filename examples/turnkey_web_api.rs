//! Turnkey Web API Integration Example
//! 
//! This example demonstrates how to integrate Turnkey wallet functionality
//! into a web API server for wallet recovery operations.

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use solana_recover::{
    WalletManager, WalletCredentials, WalletType, WalletCredentialData,
    TurnkeyProvider, TurnkeyConfig, WalletConnection
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn, error};
use uuid::Uuid;

/// Application state containing shared services
#[derive(Clone)]
struct AppState {
    wallet_manager: Arc<WalletManager>,
    turnkey_provider: Arc<TurnkeyProvider>,
}

/// API response wrapper
#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Turnkey connection request
#[derive(Debug, Deserialize)]
struct ConnectRequest {
    api_key: String,
    organization_id: String,
    private_key_id: String,
}

/// Turnkey connection response
#[derive(Debug, Serialize)]
struct ConnectResponse {
    connection_id: String,
    public_key: String,
    wallet_address: String,
}

/// Transaction signing request
#[derive(Debug, Deserialize)]
struct SignRequest {
    connection_id: String,
    transaction: String, // Base64 encoded transaction
}

/// Transaction signing response
#[derive(Debug, Serialize)]
struct SignResponse {
    signature: String,
    signed_transaction: String, // Base64 encoded signed transaction
}

/// Wallet info response
#[derive(Debug, Serialize)]
struct WalletInfoResponse {
    wallet_id: String,
    public_key: String,
    wallet_address: String,
    wallet_type: String,
    created_at: String,
    last_used: Option<String>,
}

/// Health check response
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    turnkey_healthy: bool,
    cache_stats: CacheStats,
}

/// Cache statistics
#[derive(Debug, Serialize)]
struct CacheStats {
    total_sessions: usize,
    valid_sessions: usize,
}

/// Error types
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    code: String,
}

impl ErrorResponse {
    fn new(error: String, code: &str) -> Self {
        Self {
            error,
            code: code.to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Turnkey Web API Server ===\n");

    // Initialize Turnkey provider with custom configuration
    let turnkey_config = TurnkeyConfig {
        api_url: std::env::var("TURNKEY_API_URL")
            .unwrap_or_else(|_| "https://api.turnkey.com".to_string()),
        timeout_seconds: std::env::var("TURNKEY_TIMEOUT_SECONDS")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .unwrap_or(30),
        retry_attempts: std::env::var("TURNKEY_RETRY_ATTEMPTS")
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .unwrap_or(3),
        enable_session_caching: std::env::var("TURNKEY_ENABLE_CACHE")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
    };

    let turnkey_provider = Arc::new(TurnkeyProvider::with_config(turnkey_config));

    // Initialize WalletManager
    let wallet_manager = Arc::new(WalletManager::new());

    // Create application state
    let app_state = AppState {
        wallet_manager,
        turnkey_provider,
    };

    // Build the router
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/api/v1/turnkey/connect", post(connect_wallet))
        .route("/api/v1/turnkey/wallet/:connection_id", get(get_wallet_info))
        .route("/api/v1/turnkey/sign", post(sign_transaction))
        .route("/api/v1/turnkey/disconnect/:connection_id", post(disconnect_wallet))
        .route("/api/v1/turnkey/cache/stats", get(get_cache_stats))
        .route("/api/v1/turnkey/cache/clear", post(clear_cache))
        .with_state(app_state);

    // Start the server
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);

    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await?;

    println!("Turnkey Web API server listening on: http://{}", addr);
    println!("\nAvailable endpoints:");
    println!("  GET  /                    - Root endpoint");
    println!("  GET  /health              - Health check");
    println!("  POST /api/v1/turnkey/connect           - Connect wallet");
    println!("  GET  /api/v1/turnkey/wallet/:id       - Get wallet info");
    println!("  POST /api/v1/turnkey/sign             - Sign transaction");
    println!("  POST /api/v1/turnkey/disconnect/:id   - Disconnect wallet");
    println!("  GET  /api/v1/turnkey/cache/stats      - Cache statistics");
    println!("  POST /api/v1/turnkey/cache/clear      - Clear cache");
    println!("\nExample usage:");
    println!("curl -X POST http://localhost:8080/api/v1/turnkey/connect \\");
    println!("  -H 'Content-Type: application/json' \\");
    println!("  -d '{{\"api_key\":\"your_key\",\"organization_id\":\"your_org\",\"private_key_id\":\"your_key_id\"}}'");

    axum::serve(listener, app).await?;

    Ok(())
}

/// Root endpoint
async fn root() -> ResponseJson<ApiResponse<String>> {
    ResponseJson(ApiResponse::success(
        "Turnkey Web API v1.0.0 - Wallet Integration Service".to_string()
    ))
}

/// Health check endpoint
async fn health_check(State(state): State<AppState>) -> ResponseJson<ApiResponse<HealthResponse>> {
    let turnkey_healthy = state.turnkey_provider.health_check().await.unwrap_or(false);
    let (total_sessions, valid_sessions) = state.turnkey_provider.get_cache_stats();

    let health_response = HealthResponse {
        status: if turnkey_healthy { "healthy" } else { "unhealthy" }.to_string(),
        turnkey_healthy,
        cache_stats: CacheStats {
            total_sessions,
            valid_sessions,
        },
    };

    ResponseJson(ApiResponse::success(health_response))
}

/// Connect Turnkey wallet
async fn connect_wallet(
    State(state): State<AppState>,
    Json(request): Json<ConnectRequest>,
) -> Result<ResponseJson<ApiResponse<ConnectResponse>>, StatusCode> {
    info!("Connecting Turnkey wallet for organization: {}", request.organization_id);

    // Validate request
    if request.api_key.is_empty() || request.organization_id.is_empty() || request.private_key_id.is_empty() {
        return Ok(ResponseJson(ApiResponse::error(
            "Missing required fields: api_key, organization_id, private_key_id".to_string()
        )));
    }

    // Create credentials
    let credentials = WalletCredentials {
        wallet_type: WalletType::Turnkey,
        credentials: WalletCredentialData::Turnkey {
            api_key: request.api_key,
            organization_id: request.organization_id,
            private_key_id: request.private_key_id,
        },
    };

    // Connect wallet
    match state.turnkey_provider.connect(&credentials).await {
        Ok(connection) => {
            // Get public key
            match state.turnkey_provider.get_public_key(&connection).await {
                Ok(public_key) => {
                    let response = ConnectResponse {
                        connection_id: connection.id,
                        public_key: public_key.clone(),
                        wallet_address: public_key,
                    };

                    info!("Successfully connected Turnkey wallet: {}", connection.id);
                    Ok(ResponseJson(ApiResponse::success(response)))
                }
                Err(e) => {
                    error!("Failed to get public key: {}", e);
                    Ok(ResponseJson(ApiResponse::error(format!("Failed to get public key: {}", e))))
                }
            }
        }
        Err(e) => {
            error!("Failed to connect Turnkey wallet: {}", e);
            Ok(ResponseJson(ApiResponse::error(format!("Failed to connect wallet: {}", e))))
        }
    }
}

/// Get wallet information
async fn get_wallet_info(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> Result<ResponseJson<ApiResponse<WalletInfoResponse>>, StatusCode> {
    info!("Getting wallet info for connection: {}", connection_id);

    // Get connection from WalletManager
    if let Some(connection) = state.wallet_manager.get_connection(&connection_id) {
        // Get public key
        match state.turnkey_provider.get_public_key(&connection).await {
            Ok(public_key) => {
                let wallet_info = WalletInfoResponse {
                    wallet_id: connection.id,
                    public_key: public_key.clone(),
                    wallet_address: public_key,
                    wallet_type: "Turnkey".to_string(),
                    created_at: connection.created_at.to_rfc3339(),
                    last_used: Some(chrono::Utc::now().to_rfc3339()),
                };

                Ok(ResponseJson(ApiResponse::success(wallet_info)))
            }
            Err(e) => {
                error!("Failed to get public key: {}", e);
                Ok(ResponseJson(ApiResponse::error(format!("Failed to get public key: {}", e))))
            }
        }
    } else {
        warn!("Connection not found: {}", connection_id);
        Ok(ResponseJson(ApiResponse::error(
            "Connection not found".to_string()
        )))
    }
}

/// Sign transaction
async fn sign_transaction(
    State(state): State<AppState>,
    Json(request): Json<SignRequest>,
) -> Result<ResponseJson<ApiResponse<SignResponse>>, StatusCode> {
    info!("Signing transaction for connection: {}", request.connection_id);

    // Validate request
    if request.transaction.is_empty() {
        return Ok(ResponseJson(ApiResponse::error(
            "Transaction cannot be empty".to_string()
        )));
    }

    // Get connection
    if let Some(connection) = state.wallet_manager.get_connection(&request.connection_id) {
        // Decode base64 transaction
        let transaction = match base64::decode(&request.transaction) {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to decode transaction: {}", e);
                return Ok(ResponseJson(ApiResponse::error(
                    format!("Invalid transaction encoding: {}", e)
                )));
            }
        };

        // Sign transaction
        match state.turnkey_provider.sign_transaction(&connection, &transaction).await {
            Ok(signed_transaction) => {
                // Extract signature (first 64 bytes)
                if signed_transaction.len() < 64 {
                    return Ok(ResponseJson(ApiResponse::error(
                        "Invalid signed transaction format".to_string()
                    )));
                }

                let signature = base64::encode(&signed_transaction[..64]);
                let signed_tx_base64 = base64::encode(&signed_transaction);

                let response = SignResponse {
                    signature,
                    signed_transaction: signed_tx_base64,
                };

                info!("Successfully signed transaction for connection: {}", request.connection_id);
                Ok(ResponseJson(ApiResponse::success(response)))
            }
            Err(e) => {
                error!("Failed to sign transaction: {}", e);
                Ok(ResponseJson(ApiResponse::error(format!("Failed to sign transaction: {}", e))))
            }
        }
    } else {
        warn!("Connection not found: {}", request.connection_id);
        Ok(ResponseJson(ApiResponse::error(
            "Connection not found".to_string()
        )))
    }
}

/// Disconnect wallet
async fn disconnect_wallet(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> Result<ResponseJson<ApiResponse<String>>, StatusCode> {
    info!("Disconnecting wallet: {}", connection_id);

    // Disconnect from WalletManager
    match state.wallet_manager.disconnect_wallet(&connection_id).await {
        Ok(_) => {
            info!("Successfully disconnected wallet: {}", connection_id);
            Ok(ResponseJson(ApiResponse::success(
                "Wallet disconnected successfully".to_string()
            )))
        }
        Err(e) => {
            error!("Failed to disconnect wallet: {}", e);
            Ok(ResponseJson(ApiResponse::error(format!("Failed to disconnect wallet: {}", e))))
        }
    }
}

/// Get cache statistics
async fn get_cache_stats(State(state): State<AppState>) -> ResponseJson<ApiResponse<CacheStats>> {
    let (total_sessions, valid_sessions) = state.turnkey_provider.get_cache_stats();

    let cache_stats = CacheStats {
        total_sessions,
        valid_sessions,
    };

    ResponseJson(ApiResponse::success(cache_stats))
}

/// Clear cache
async fn clear_cache(State(state): State<AppState>) -> ResponseJson<ApiResponse<String>> {
    state.turnkey_provider.clear_session_cache();
    info!("Session cache cleared");

    ResponseJson(ApiResponse::success(
        "Cache cleared successfully".to_string()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;

    #[tokio::test]
    async fn test_root_endpoint() {
        let app_state = create_test_state().await;
        let app = Router::new()
            .route("/", get(root))
            .with_state(app_state);

        let server = TestServer::new(app).unwrap();
        let response = server.get("/").await;

        assert_eq!(response.status_code(), StatusCode::OK);
        let body: ApiResponse<String> = response.json();
        assert!(body.success);
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app_state = create_test_state().await;
        let app = Router::new()
            .route("/health", get(health_check))
            .with_state(app_state);

        let server = TestServer::new(app).unwrap();
        let response = server.get("/health").await;

        assert_eq!(response.status_code(), StatusCode::OK);
        let body: ApiResponse<HealthResponse> = response.json();
        assert!(body.success);
    }

    async fn create_test_state() -> AppState {
        let turnkey_config = TurnkeyConfig {
            enable_session_caching: false, // Disable for testing
            ..Default::default()
        };
        let turnkey_provider = Arc::new(TurnkeyProvider::with_config(turnkey_config));
        let wallet_manager = Arc::new(WalletManager::new());

        AppState {
            wallet_manager,
            turnkey_provider,
        }
    }
}
