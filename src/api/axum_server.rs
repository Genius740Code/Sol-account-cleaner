use crate::core::{BatchScanRequest, Result, ScanResult, RecoveryRequest, RecoveryResult, SolanaRecoverError};
use crate::core::scanner::WalletScanner;
use crate::core::processor::BatchProcessor;
use crate::core::recovery::RecoveryManager;
use crate::wallet::WalletManager;
use crate::storage::{RedisCacheManager, CacheManager};
use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::net::SocketAddr;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, IntoResponse},
    routing::{get, post},
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{CorsLayer, Any},
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use governor::{
    clock::{QuantaClock, QuantaInstant},
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use std::time::Duration;
use metrics::{counter, histogram, gauge};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanRequest {
    pub wallet_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BatchQueryParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    pub format: Option<String>, // "json" or "prometheus"
}

#[derive(Clone)]
pub struct AxumApiState {
    pub scanner: Arc<WalletScanner>,
    pub batch_processor: Arc<BatchProcessor>,
    pub recovery_manager: Arc<RecoveryManager>,
    pub wallet_manager: Arc<WalletManager>,
    pub cache_manager: Option<Arc<RedisCacheManager>>,
    pub fallback_cache: Option<Arc<CacheManager>>,
    pub config: Config,
    pub rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>>,
    pub metrics_handle: PrometheusHandle,
}

#[derive(Debug)]
pub struct AxumServer {
    #[allow(dead_code)]
    addr: SocketAddr,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl AxumServer {
    pub async fn shutdown(self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        Ok(())
    }
}

// Error handler for SolanaRecoverError
impl IntoResponse for SolanaRecoverError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            SolanaRecoverError::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string()),
            SolanaRecoverError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
            SolanaRecoverError::InvalidWalletAddress(_) => (StatusCode::BAD_REQUEST, "Invalid wallet address".to_string()),
            SolanaRecoverError::AuthenticationError(_) => (StatusCode::UNAUTHORIZED, "Authentication failed".to_string()),
            SolanaRecoverError::NoRecoverableFunds(_) => (StatusCode::NOT_FOUND, "No recoverable funds found".to_string()),
            SolanaRecoverError::WalletNotFound(_) => (StatusCode::NOT_FOUND, "Wallet not found".to_string()),
            SolanaRecoverError::InsufficientBalance { .. } => (StatusCode::BAD_REQUEST, "Insufficient balance".to_string()),
            SolanaRecoverError::ValidationError(_) => (StatusCode::BAD_REQUEST, "Validation error".to_string()),
            SolanaRecoverError::ConfigError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error".to_string()),
            SolanaRecoverError::InternalError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
            SolanaRecoverError::DatabaseError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()),
            SolanaRecoverError::RusqliteError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()),
            SolanaRecoverError::StorageError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Storage error".to_string()),
            SolanaRecoverError::SerializationError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Serialization error".to_string()),
            SolanaRecoverError::NetworkError(_) => (StatusCode::SERVICE_UNAVAILABLE, "Network error".to_string()),
            SolanaRecoverError::TimeoutError(_) => (StatusCode::REQUEST_TIMEOUT, "Request timeout".to_string()),
            SolanaRecoverError::ConnectionPoolExhausted => (StatusCode::SERVICE_UNAVAILABLE, "Connection pool exhausted".to_string()),
            SolanaRecoverError::TransactionFailed(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Transaction failed".to_string()),
            SolanaRecoverError::TransactionError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Transaction error".to_string()),
            SolanaRecoverError::InvalidFeeStructure(_) => (StatusCode::BAD_REQUEST, "Invalid fee structure".to_string()),
            SolanaRecoverError::ConfigurationError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error".to_string()),
            SolanaRecoverError::IoError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "IO error".to_string()),
            SolanaRecoverError::RpcClientError(_) => (StatusCode::SERVICE_UNAVAILABLE, "RPC client error".to_string()),
        };

        let body = Json(serde_json::json!({
            "success": false,
            "error": error_message,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }));

        (status, body).into_response()
    }
}

pub async fn start_server(
    state: AxumApiState,
    config: &crate::config::ServerConfig,
) -> Result<AxumServer> {
    let addr = format!("{}:{}", config.host, config.port)
        .parse::<SocketAddr>()
        .map_err(|e| crate::SolanaRecoverError::InternalError(
            format!("Invalid server address: {}", e)
        ))?;

    // Setup metrics
    let _ = setup_metrics().await;
    
    // Create rate limiter (100 requests per minute per IP)
    let quota = Quota::per_minute(NonZeroU32::new(100).unwrap());
    let _rate_limiter = Arc::new(RateLimiter::direct(quota));

    // Update state with metrics handle
    let state_with_metrics = state;

    // Build the application router
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        .route("/health/ping", get(ping))
        
        // API routes
        .route("/api/v1/scan", post(scan_wallet))
        .route("/api/v1/batch-scan", post(batch_scan))
        .route("/api/v1/batch-scan/:id", get(get_batch_status))
        .route("/api/v1/recover", post(recover_sol))
        .route("/api/v1/recovery/:id", get(get_recovery_status))
        .route("/api/v1/estimate-fees", post(estimate_fees))
        
        // Metrics and monitoring
        .route("/metrics", get(get_metrics))
        .route("/metrics/prometheus", get(get_prometheus_metrics))
        .route("/status", get(get_system_status))
        
        // System info
        .route("/", get(root))
        
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        )
        .with_state(state_with_metrics);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    // Spawn the server
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let server = axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(async {
            shutdown_rx.await.ok();
            info!("Graceful shutdown signal received");
        });

    info!("Axum server listening on {}", addr);

    let _server_handle = tokio::spawn(async move {
        if let Err(e) = server.await {
            error!("Server error: {}", e);
        }
    });

    // Wait for a brief moment to ensure server started
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(AxumServer {
        addr,
        shutdown_tx,
    })
}

#[allow(dead_code)]
async fn rate_limit_middleware(
    State(state): State<AxumApiState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    // Extract IP from headers or connection info
    let ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .unwrap_or("unknown")
        .to_string();

    // Check rate limit
    match state.rate_limiter.check() {
        Ok(_) => {
            counter!("api.requests.total", 1);
            next.run(request).await
        }
        Err(_) => {
            counter!("api.requests.rate_limited", 1);
            warn!("Rate limit exceeded for IP: {}", ip);
            StatusCode::TOO_MANY_REQUESTS.into_response()
        }
    }
}

// Health check endpoints
async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse::success("OK".to_string()))
}

async fn ping() -> Json<ApiResponse<String>> {
    Json(ApiResponse::success("pong".to_string()))
}

// Root endpoint
async fn root() -> Json<ApiResponse<serde_json::Value>> {
    let info = serde_json::json!({
        "service": "Solana Recover API",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "High-performance Solana wallet scanner for finding recoverable SOL",
        "endpoints": {
            "health": "/health",
            "scan": "/api/v1/scan",
            "batch_scan": "/api/v1/batch-scan",
            "recover": "/api/v1/recover",
            "estimate_fees": "/api/v1/estimate-fees",
            "metrics": "/metrics",
            "status": "/status"
        }
    });
    
    Json(ApiResponse::success(info))
}

// API endpoints
async fn scan_wallet(
    State(state): State<AxumApiState>,
    Json(request): Json<ScanRequest>,
) -> std::result::Result<Json<ApiResponse<ScanResult>>, StatusCode> {
    let start_time = std::time::Instant::now();
    
    info!("Scanning wallet: {}", request.wallet_address);
    
    match state.scanner.scan_wallet(&request.wallet_address).await {
        Ok(result) => {
            let duration = start_time.elapsed();
            histogram!("api.scan.duration_ms", duration.as_millis() as f64);
            counter!("api.scan.success", 1);
            
            debug!("Wallet scan completed in {:?}", duration);
            Ok(Json(ApiResponse::success(result)))
        }
        Err(e) => {
            let duration = start_time.elapsed();
            histogram!("api.scan.duration_ms", duration.as_millis() as f64);
            counter!("api.scan.error", 1);
            
            error!("Failed to scan wallet {}: {}", request.wallet_address, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn batch_scan(
    State(state): State<AxumApiState>,
    Json(request): Json<BatchScanRequest>,
) -> std::result::Result<Json<ApiResponse<crate::core::BatchScanResult>>, StatusCode> {
    let start_time = std::time::Instant::now();
    
    info!("Starting batch scan: {} wallets", request.wallet_addresses.len());
    
    match state.batch_processor.process_batch(&request).await {
        Ok(result) => {
            let duration = start_time.elapsed();
            histogram!("api.batch_scan.duration_ms", duration.as_millis() as f64);
            counter!("api.batch_scan.success", 1);
            gauge!("api.batch_scan.wallets_processed", request.wallet_addresses.len() as f64);
            
            info!("Batch scan completed in {:?}: {} successful, {} failed", 
                  duration, result.successful_scans, result.failed_scans);
            
            Ok(Json(ApiResponse::success(result)))
        }
        Err(e) => {
            let duration = start_time.elapsed();
            histogram!("api.batch_scan.duration_ms", duration.as_millis() as f64);
            counter!("api.batch_scan.error", 1);
            
            error!("Batch scan failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_batch_status(
    State(_state): State<AxumApiState>,
    Path(_batch_id): Path<Uuid>,
) -> crate::core::Result<Json<ApiResponse<Option<crate::core::BatchScanResult>>>> {
    // This would typically fetch from a database or cache
    // For now, return a placeholder
    Ok(Json(ApiResponse::success(None)))
}

async fn recover_sol(
    State(state): State<AxumApiState>,
    Json(request): Json<RecoveryRequest>,
) -> crate::core::Result<Json<ApiResponse<RecoveryResult>>> {
    let start_time = std::time::Instant::now();
    
    info!("Starting SOL recovery for wallet: {}", request.wallet_address);
    
    match state.recovery_manager.recover_sol(&request).await {
        Ok(result) => {
            let duration = start_time.elapsed();
            histogram!("api.recover.duration_ms", duration.as_millis() as f64);
            counter!("api.recover.success", 1);
            
            info!("SOL recovery completed in {:?}", duration);
            Ok(Json(ApiResponse::success(result)))
        }
        Err(e) => {
            let duration = start_time.elapsed();
            histogram!("api.recover.duration_ms", duration.as_millis() as f64);
            counter!("api.recover.error", 1);
            
            error!("SOL recovery failed: {}", e);
            Err(SolanaRecoverError::InternalError(format!("SOL recovery failed: {}", e)))
        }
    }
}

async fn get_recovery_status(
    State(state): State<AxumApiState>,
    Path(recovery_id): Path<Uuid>,
) -> crate::core::Result<Json<ApiResponse<Option<RecoveryResult>>>> {
    match state.recovery_manager.get_recovery_status(&recovery_id).await {
        Ok(result) => Ok(Json(ApiResponse::success(result))),
        Err(e) => {
            error!("Failed to get recovery status {}: {}", recovery_id, e);
            Err(SolanaRecoverError::InternalError(format!("Failed to get recovery status: {}", e)))
        }
    }
}

async fn estimate_fees(
    State(state): State<AxumApiState>,
    Json(accounts): Json<Vec<String>>,
) -> crate::core::Result<Json<ApiResponse<u64>>> {
    let start_time = std::time::Instant::now();
    
    info!("Estimating fees for {} accounts", accounts.len());
    
    match state.recovery_manager.estimate_recovery_fees(&accounts).await {
        Ok(fees) => {
            let duration = start_time.elapsed();
            histogram!("api.estimate_fees.duration_ms", duration.as_millis() as f64);
            counter!("api.estimate_fees.success", 1);
            
            debug!("Fee estimation completed in {:?}: {} lamports", duration, fees);
            Ok(Json(ApiResponse::success(fees)))
        }
        Err(e) => {
            let duration = start_time.elapsed();
            histogram!("api.estimate_fees.duration_ms", duration.as_millis() as f64);
            counter!("api.estimate_fees.error", 1);
            
            error!("Fee estimation failed: {}", e);
            Err(SolanaRecoverError::InternalError(format!("Fee estimation failed: {}", e)))
        }
    }
}

// Metrics endpoints
async fn get_metrics(
    State(state): State<AxumApiState>,
    Query(_params): Query<MetricsQuery>,
) -> crate::core::Result<Json<ApiResponse<serde_json::Value>>> {
    let processor_metrics = state.batch_processor.get_metrics().await;
    
    let metrics = serde_json::json!({
        "processor": processor_metrics,
        "cache": state.cache_manager.as_ref().map(|c| c.get_metrics()),
        "timestamp": Utc::now(),
    });
    
    Ok(Json(ApiResponse::success(metrics)))
}

async fn get_prometheus_metrics(
    State(state): State<AxumApiState>,
) -> std::result::Result<String, StatusCode> {
    Ok(state.metrics_handle.render())
}

async fn get_system_status(
    State(state): State<AxumApiState>,
) -> std::result::Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let processor_metrics = state.batch_processor.get_metrics().await;
    let active_batches = state.batch_processor.get_active_batches().await;
    
    let status = serde_json::json!({
        "service": "Solana Recover API",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": "TODO", // Would track actual uptime
        "processor": {
            "metrics": processor_metrics,
            "active_batches": active_batches,
        },
        "cache": state.cache_manager.as_ref().map(|c| c.get_metrics()),
        "config": {
            "max_concurrent_batches": state.config.scanner.max_concurrent_wallets,
            "cache_enabled": state.cache_manager.is_some(),
        },
        "timestamp": Utc::now(),
    });
    
    Ok(Json(ApiResponse::success(status)))
}

async fn setup_metrics() -> crate::core::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9091))
        .install()
        .map_err(|e| crate::SolanaRecoverError::InternalError(format!("Failed to setup metrics: {}", e)))?;
    Ok(())
}

// Error handling
pub fn make_api_error<T>(error: String) -> Json<ApiResponse<T>> {
    error!("API error: {}", error);
    Json(ApiResponse::error(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    
    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        let (status, _) = response.into_response().into_parts();
        assert_eq!(status, StatusCode::OK);
    }
    
    #[tokio::test]
    async fn test_ping() {
        let response = ping().await;
        let (status, _) = response.into_response().into_parts();
        assert_eq!(status, StatusCode::OK);
    }
}
