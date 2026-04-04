use crate::core::{BatchScanRequest, BatchScanResult, ScanResult};
use crate::core::processor::BatchProcessor;
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
