use crate::core::{BatchScanRequest, BatchScanResult, ScanResult, RecoveryRequest, RecoveryResult};
use crate::core::processor::BatchProcessor;
use crate::core::recovery::RecoveryManager;
use crate::api::server::{ScanRequest, ApiResponse};

pub async fn scan_wallet(
    _processor: std::sync::Arc<BatchProcessor>,
    _request: ScanRequest,
) -> ApiResponse<ScanResult> {
    // Access scanner through public method or add a public getter
    ApiResponse {
        success: false,
        data: None,
        error: Some("Direct scanner access not available through handlers. Use API endpoints instead.".to_string()),
    }
}

pub async fn batch_scan(
    processor: std::sync::Arc<BatchProcessor>,
    request: BatchScanRequest,
) -> ApiResponse<BatchScanResult> {
    match processor.process_batch(&request).await {
        Ok(result) => ApiResponse::success(result),
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        },
    }
}

pub async fn get_scan_status(
    _scan_id: String,
) -> ApiResponse<ScanResult> {
    ApiResponse {
        success: false,
        data: None,
        error: Some("Scan status retrieval not implemented yet".to_string()),
    }
}

pub async fn health_check() -> ApiResponse<String> {
    ApiResponse::success("OK".to_string())
}

// Recovery endpoints
pub async fn recover_sol(
    recovery_manager: std::sync::Arc<RecoveryManager>,
    request: RecoveryRequest,
) -> ApiResponse<RecoveryResult> {
    // Validate request first
    match recovery_manager.validate_recovery_request(&request).await {
        Ok(_) => {
            match recovery_manager.recover_sol(&request).await {
                Ok(result) => ApiResponse::success(result),
                Err(e) => ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                },
            }
        }
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        },
    }
}

pub async fn estimate_recovery_fees(
    recovery_manager: std::sync::Arc<RecoveryManager>,
    accounts: Vec<String>,
) -> ApiResponse<u64> {
    match recovery_manager.estimate_recovery_fees(&accounts).await {
        Ok(fees) => ApiResponse::success(fees),
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        },
    }
}

pub async fn get_recovery_status(
    recovery_manager: std::sync::Arc<RecoveryManager>,
    recovery_id: uuid::Uuid,
) -> ApiResponse<Option<RecoveryResult>> {
    match recovery_manager.get_recovery_status(&recovery_id).await {
        Ok(result) => ApiResponse::success(result),
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        },
    }
}
