//! Web API server example
//! 
//! This example demonstrates how to build a simple web API using the solana-recover crate
//! with the `api` feature enabled.

use axum::{
    extract::Query,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use solana_recover::{scan_wallet, BatchProcessor, BatchScanRequest, WalletScanner, RpcEndpoint};
use solana_recover::{rpc::ConnectionPool, config::Config};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::Instant;

#[derive(Debug, Deserialize)]
struct ScanQuery {
    address: String,
    rpc_endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BatchRequest {
    wallets: Vec<String>,
    fee_percentage: Option<f64>,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
    timestamp: u64,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

#[derive(Debug, Serialize)]
struct HealthStatus {
    status: String,
    uptime_seconds: u64,
    version: String,
}

// Global state for the server
#[derive(Clone)]
struct AppState {
    start_time: Instant,
    batch_processor: Arc<BatchProcessor>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration and create components
    let config = Config::load().unwrap_or_else(|_| Config::default());
    
    // Create RPC endpoints
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
    
    // Create connection pool and scanner
    let connection_pool = Arc::new(ConnectionPool::new(rpc_endpoints, config.rpc.pool_size));
    let scanner = Arc::new(WalletScanner::new(connection_pool));
    
    // Initialize batch processor
    let batch_processor = {
        let processor = BatchProcessor::new(
            scanner,
            None, // No cache
            None, // No persistence
            config.scanner.into(),
        )?;
        Arc::new(processor)
    };
    
    let state = AppState {
        start_time: Instant::now(),
        batch_processor,
    };
    
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/scan", post(scan_wallet_handler))
        .route("/scan", get(scan_wallet_get))
        .route("/batch", post(batch_scan_handler))
        .route("/stats", get(stats_handler))
        .with_state(state);
    
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    
    println!("🚀 Solana Recover API Server");
    println!("📍 Server running on http://{}", addr);
    println!("📖 API Documentation:");
    println!("  GET  /           - Welcome message");
    println!("  GET  /health     - Health check");
    println!("  GET  /scan       - Scan wallet (query params)");
    println!("  POST /scan       - Scan wallet (JSON body)");
    println!("  POST /batch      - Batch scan multiple wallets");
    println!("  GET  /stats      - Server statistics");
    println!();
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn root() -> Json<Value> {
    Json(json!({
        "name": "Solana Recover API",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "High-performance Solana wallet scanning API",
        "endpoints": {
            "health": "GET /health",
            "scan": "GET|POST /scan",
            "batch": "POST /batch",
            "stats": "GET /stats"
        }
    }))
}

async fn health_check(axum::extract::State(state): axum::extract::State<AppState>) -> Json<ApiResponse<HealthStatus>> {
    let uptime = state.start_time.elapsed().as_secs();
    
    Json(ApiResponse::success(HealthStatus {
        status: "healthy".to_string(),
        uptime_seconds: uptime,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}

async fn scan_wallet_get(
    axum::extract::State(_state): axum::extract::State<AppState>,
    Query(params): Query<ScanQuery>,
) -> Json<ApiResponse<Value>> {
    let rpc_endpoint = params.rpc_endpoint.as_deref();
    
    match scan_wallet(&params.address, rpc_endpoint).await {
        Ok(result) => {
            Json(ApiResponse::success(json!({
                "wallet_address": result.address,
                "total_accounts": result.total_accounts,
                "empty_accounts": result.empty_accounts,
                "recoverable_sol": result.recoverable_sol,
                "scan_time_ms": result.scan_time_ms,
                "empty_account_addresses": result.empty_account_addresses
            })))
        }
        Err(e) => {
            Json(ApiResponse::error(format!("Scan failed: {}", e)))
        }
    }
}

async fn scan_wallet_handler(
    axum::extract::State(_state): axum::extract::State<AppState>,
    Json(payload): Json<Value>,
) -> Json<ApiResponse<Value>> {
    let wallet_address = match payload["wallet_address"].as_str() {
        Some(addr) => addr,
        None => return Json(ApiResponse::error("Missing wallet_address field".to_string())),
    };
    
    let rpc_endpoint = payload["rpc_endpoint"].as_str();
    
    match scan_wallet(wallet_address, rpc_endpoint).await {
        Ok(result) => {
            Json(ApiResponse::success(json!({
                "wallet_address": result.address,
                "total_accounts": result.total_accounts,
                "empty_accounts": result.empty_accounts,
                "recoverable_sol": result.recoverable_sol,
                "scan_time_ms": result.scan_time_ms,
                "empty_account_addresses": result.empty_account_addresses
            })))
        }
        Err(e) => {
            Json(ApiResponse::error(format!("Scan failed: {}", e)))
        }
    }
}

async fn batch_scan_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<BatchRequest>,
) -> Json<ApiResponse<Value>> {
    if payload.wallets.is_empty() {
        return Json(ApiResponse::error("No wallets provided".to_string()));
    }
    
    if payload.wallets.len() > 100 {
        return Json(ApiResponse::error("Too many wallets (max 100 per request)".to_string()));
    }
    
    let request = BatchScanRequest {
        id: uuid::Uuid::new_v4(),
        wallet_addresses: payload.wallets,
        user_id: Some("web_api_user".to_string()),
        fee_percentage: payload.fee_percentage,
        created_at: chrono::Utc::now(),
    };
    
    match state.batch_processor.process_batch(&request).await {
        Ok(results) => {
            let successful = results.results.iter().filter(|r| r.result.is_some()).count();
            let failed = results.results.iter().filter(|r| r.result.is_none()).count();
            
            Json(ApiResponse::success(json!({
                "total_wallets": results.results.len(),
                "successful": successful,
                "failed": failed,
                "results": results.results.into_iter().map(|r| {
                    match r.result {
                        Some(scan_result) => json!({
                            "wallet_address": r.wallet_address,
                            "success": true,
                            "total_accounts": scan_result.total_accounts,
                            "empty_accounts": scan_result.empty_accounts,
                            "recoverable_sol": scan_result.recoverable_sol,
                            "scan_time_ms": scan_result.scan_time_ms
                        }),
                        None => json!({
                            "wallet_address": r.wallet_address,
                            "success": false,
                            "error": r.error_message.as_deref().unwrap_or("Unknown error")
                        })
                    }
                }).collect::<Vec<_>>()
            })))
        }
        Err(e) => {
            Json(ApiResponse::error(format!("Batch scan failed: {}", e)))
        }
    }
}

async fn stats_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<ApiResponse<Value>> {
    let uptime = state.start_time.elapsed();
    
    Json(ApiResponse::success(json!({
        "uptime_seconds": uptime.as_secs(),
        "uptime_human": format!("{}m {}s", uptime.as_secs() / 60, uptime.as_secs() % 60),
        "version": env!("CARGO_PKG_VERSION"),
        "features": {
            "scanner": cfg!(feature = "scanner"),
            "api": cfg!(feature = "api"),
            "database": cfg!(feature = "database"),
            "cache": cfg!(feature = "cache"),
            "metrics": cfg!(feature = "metrics"),
            "security": cfg!(feature = "security"),
            "config": cfg!(feature = "config")
        }
    })))
}
