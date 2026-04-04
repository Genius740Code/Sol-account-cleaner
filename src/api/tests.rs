#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::server::{ScanRequest, ApiResponse, ApiState};
    use crate::core::{BatchScanRequest, ScanResult, ScanStatus};
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_scan_request_serialization() {
        let request = ScanRequest {
            wallet_address: "11111111111111111111111111111112".to_string(),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&request);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: ScanRequest = serde_json::from_str(&json.unwrap()).unwrap();
        assert_eq!(deserialized.wallet_address, "11111111111111111111111111111112");
    }

    #[tokio::test]
    async fn test_api_response_success() {
        let data = "test data".to_string();
        let response = ApiResponse::success(data);

        assert!(response.success);
        assert!(response.data.is_some());
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_api_response_error() {
        let error = "test error".to_string();
        let response = ApiResponse::<String>::error(error);

        assert!(!response.success);
        assert!(response.data.is_none());
        assert!(response.error.is_some());
    }

    #[tokio::test]
    async fn test_api_response_serialization() {
        let response = ApiResponse::success("test data".to_string());

        // Test JSON serialization
        let json = serde_json::to_string(&response);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: ApiResponse<String> = serde_json::from_str(&json.unwrap()).unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.data.unwrap(), "test data");
    }

    #[tokio::test]
    async fn test_batch_scan_request_serialization() {
        let request = BatchScanRequest {
            id: Uuid::new_v4(),
            wallet_addresses: vec![
                "11111111111111111111111111111112".to_string(),
                "11111111111111111111111111111113".to_string(),
            ],
            user_id: None,
            fee_percentage: None,
            created_at: chrono::Utc::now(),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&request);
        assert!(json.is_ok());

        // Test JSON deserialization
        let deserialized: BatchScanRequest = serde_json::from_str(&json.unwrap()).unwrap();
        assert_eq!(deserialized.wallet_addresses.len(), 2);
    }

    #[tokio::test]
    async fn test_extract_json_from_request() {
        let request = "POST /api/v1/scan HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"wallet_address\":\"11111111111111111111111111111112\"}";
        
        let json = crate::api::server::extract_json_from_request(request);
        
        assert!(json.contains("wallet_address"));
        assert!(json.contains("11111111111111111111111111111112"));
    }

    #[tokio::test]
    async fn test_create_json_response() {
        let data = ApiResponse::success("test data".to_string());
        
        let response = crate::api::server::create_json_response(&data);
        
        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: application/json"));
        assert!(response.contains("\"success\":true"));
    }

    #[tokio::test]
    async fn test_health_check_handler() {
        let response = crate::api::handlers::health_check().await;
        
        assert!(response.success);
        assert!(response.data.is_some());
        assert_eq!(response.data.unwrap(), "OK");
    }

    #[tokio::test]
    async fn test_get_scan_status_handler() {
        let scan_id = Uuid::new_v4().to_string();
        let response = crate::api::handlers::get_scan_status(scan_id).await;
        
        assert!(!response.success);
        assert!(response.data.is_none());
        assert!(response.error.is_some());
        assert!(response.error.unwrap().contains("not implemented yet"));
    }
}
