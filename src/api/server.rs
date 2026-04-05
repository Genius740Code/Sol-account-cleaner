use crate::core::{BatchScanRequest, Result, ScanResult, RecoveryRequest, RecoveryResult};
use crate::core::scanner::WalletScanner;
use crate::core::processor::BatchProcessor;
use crate::core::recovery::RecoveryManager;
use crate::wallet::WalletManager;
use crate::storage::{CacheManager, PersistenceManager};
use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanRequest {
    pub wallet_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

#[derive(Clone)]
pub struct ApiState {
    pub scanner: Arc<WalletScanner>,
    pub batch_processor: Arc<BatchProcessor>,
    pub recovery_manager: Arc<RecoveryManager>,
    pub wallet_manager: Arc<WalletManager>,
    pub cache_manager: Arc<CacheManager>,
    pub persistence_manager: Arc<dyn PersistenceManager>,
    pub config: Config,
}

pub struct Server {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl Server {
    pub async fn shutdown(self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        Ok(())
    }
}

pub async fn start_server(
    state: ApiState,
    config: &crate::config::ServerConfig,
) -> Result<Server> {
    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await
        .map_err(|e| crate::SolanaRecoverError::InternalError(
            format!("Failed to bind to {}: {}", addr, e)
        ))?;

    println!("Server listening on {}", addr);

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
    let state_clone = state.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            let state = state_clone.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, state).await {
                                    eprintln!("Error handling connection from {}: {}", addr, e);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Failed to accept connection: {}", e);
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    println!("Server shutdown signal received");
                    break;
                }
            }
        }
    });

    Ok(Server { shutdown_tx })
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    state: ApiState,
) -> Result<()> {
    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer).await
        .map_err(|e| crate::SolanaRecoverError::IoError(e))?;
    
    let request = String::from_utf8_lossy(&buffer[..n]);
    
    let response = if request.contains("GET / HTTP") {
        create_json_response(&ApiResponse::success("Solana Recover API v1.0.0 - Scalable wallet scanner"))
    } else if request.contains("GET /health HTTP") {
        create_json_response(&ApiResponse::success("OK"))
    } else if request.contains("POST /api/v1/scan HTTP") {
        handle_scan_request(&request, &state).await
    } else if request.contains("POST /api/v1/batch-scan HTTP") {
        handle_batch_scan_request(&request, &state).await
    } else if request.contains("POST /api/v1/recover HTTP") {
        handle_recovery_request(&request, &state).await
    } else if request.contains("POST /api/v1/estimate-fees HTTP") {
        handle_estimate_fees_request(&request, &state).await
    } else if request.contains("GET /api/v1/recovery/") {
        handle_get_recovery_status(&request, &state).await
    } else {
        "HTTP/1.1 404 Not Found\r\n\r\n".to_string()
    };

    stream.write_all(response.as_bytes()).await
        .map_err(|e| crate::SolanaRecoverError::IoError(e))?;

    Ok(())
}

async fn handle_scan_request(request: &str, state: &ApiState) -> String {
    let json_str = extract_json_from_request(request);
    
    match serde_json::from_str::<ScanRequest>(json_str) {
        Ok(scan_req) => {
            match state.scanner.scan_wallet(&scan_req.wallet_address).await {
                Ok(result) => create_json_response(&ApiResponse::success(result)),
                Err(e) => create_json_response(&ApiResponse::<ScanResult>::error(e.to_string())),
            }
        }
        Err(e) => create_json_response(&ApiResponse::<ScanResult>::error(format!("Invalid scan request: {}", e))),
    }
}

async fn handle_batch_scan_request(request: &str, state: &ApiState) -> String {
    let json_str = extract_json_from_request(request);
    
    match serde_json::from_str::<BatchScanRequest>(json_str) {
        Ok(batch_req) => {
            match state.batch_processor.process_batch(&batch_req).await {
                Ok(result) => create_json_response(&ApiResponse::success(result)),
                Err(e) => create_json_response(&ApiResponse::<crate::core::BatchScanResult>::error(e.to_string())),
            }
        }
        Err(e) => create_json_response(&ApiResponse::<crate::core::BatchScanResult>::error(format!("Invalid batch scan request: {}", e))),
    }
}

async fn handle_recovery_request(request: &str, state: &ApiState) -> String {
    let json_str = extract_json_from_request(request);
    
    match serde_json::from_str::<RecoveryRequest>(json_str) {
        Ok(recovery_req) => {
            match state.recovery_manager.recover_sol(&recovery_req).await {
                Ok(result) => create_json_response(&ApiResponse::success(result)),
                Err(e) => create_json_response(&ApiResponse::<RecoveryResult>::error(e.to_string())),
            }
        }
        Err(e) => create_json_response(&ApiResponse::<RecoveryResult>::error(format!("Invalid recovery request: {}", e))),
    }
}

async fn handle_estimate_fees_request(request: &str, state: &ApiState) -> String {
    let json_str = extract_json_from_request(request);
    
    match serde_json::from_str::<Vec<String>>(json_str) {
        Ok(accounts) => {
            match state.recovery_manager.estimate_recovery_fees(&accounts).await {
                Ok(fees) => create_json_response(&ApiResponse::success(fees)),
                Err(e) => create_json_response(&ApiResponse::<u64>::error(e.to_string())),
            }
        }
        Err(e) => create_json_response(&ApiResponse::<u64>::error(format!("Invalid accounts list: {}", e))),
    }
}

async fn handle_get_recovery_status(request: &str, state: &ApiState) -> String {
    // Extract recovery ID from URL path like /api/v1/recovery/uuid
    if let Some(start) = request.find("/api/v1/recovery/") {
        let path_part = &request[start..];
        if let Some(end) = path_part.find(" HTTP") {
            let recovery_id_str = &path_part[19..end]; // Skip "/api/v1/recovery/"
            if let Ok(recovery_id) = uuid::Uuid::parse_str(recovery_id_str) {
                match state.recovery_manager.get_recovery_status(&recovery_id).await {
                    Ok(result) => create_json_response(&ApiResponse::success(result)),
                    Err(e) => create_json_response(&ApiResponse::<Option<RecoveryResult>>::error(e.to_string())),
                }
            } else {
                create_json_response(&ApiResponse::<Option<RecoveryResult>>::error("Invalid recovery ID".to_string()))
            }
        } else {
            create_json_response(&ApiResponse::<Option<RecoveryResult>>::error("Invalid request format".to_string()))
        }
    } else {
        create_json_response(&ApiResponse::<Option<RecoveryResult>>::error("Recovery ID not found".to_string()))
    }
}

fn extract_json_from_request(request: &str) -> &str {
    // Find JSON body in HTTP request
    if let Some(start) = request.find("{") {
        if let Some(end) = request.rfind("}") {
            &request[start..=end]
        } else {
            &request[start..]
        }
    } else {
        "{}"
    }
}

fn create_json_response<T: Serialize>(data: &T) -> String {
    let json_body = serde_json::to_string(data).unwrap_or_default();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        json_body.len(),
        json_body
    )
}
