use crate::core::{BatchScanRequest, Result};
use crate::core::processor::BatchProcessor;
use crate::api::handlers;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanRequest {
    pub wallet_address: String,
}

#[derive(Debug, Serialize)]
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
    pub batch_processor: Arc<BatchProcessor>,
}

// Simple HTTP server using std::net
pub async fn start_server(batch_processor: Arc<BatchProcessor>, port: u16) -> Result<()> {
    use std::net::{TcpListener, SocketAddr};

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr)?;

    println!("Server listening on {}", addr);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let processor = batch_processor.clone();
                tokio::spawn(async move {
                    handle_connection(stream, processor).await;
                });
            }
            Err(e) => eprintln!("Failed to accept connection: {}", e),
        }
    }

    Ok(())
}

async fn handle_connection(
    mut stream: std::net::TcpStream,
    processor: Arc<BatchProcessor>,
) {
    use std::io::{prelude::*, Write};

    let mut buffer = [0; 1024];
    let request = match stream.read(&mut buffer) {
        Ok(size) => {
            String::from_utf8_lossy(&buffer[..size])
        }
        Err(e) => {
            eprintln!("Failed to read request: {}", e);
            return;
        }
    };

    let response = if request.contains("GET / HTTP") {
        create_json_response("Solana Recover API v1.0.0 - Scalable wallet scanner")
    } else if request.contains("GET /health HTTP") {
        create_json_response("OK")
    } else if request.contains("POST /api/v1/scan HTTP") {
        // Handle scan request
        let json_str = extract_json_from_request(&request);
        if let Ok(scan_req) = serde_json::from_str::<ScanRequest>(&json_str) {
            let result = handlers::scan_wallet(processor, scan_req).await;
            create_json_response(&result)
        } else {
            create_json_response("Invalid scan request")
        }
    } else if request.contains("POST /api/v1/batch-scan HTTP") {
        // Handle batch scan request
        let json_str = extract_json_from_request(&request);
        if let Ok(batch_req) = serde_json::from_str::<BatchScanRequest>(&json_str) {
            let result = handlers::batch_scan(processor, batch_req).await;
            create_json_response(&result)
        } else {
            create_json_response("Invalid batch scan request")
        }
    } else {
        "HTTP/1.1 404 Not Found\r\n\r\n".to_string()
    };

    let _ = stream.write(response.as_bytes());
}

fn extract_json_from_request(request: &str) -> &str {
    // Find JSON body in HTTP request
    if let Some(start) = request.find("{") {
        &request[start..]
    } else {
        "{}"
    }
}

fn create_json_response(json_body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        json_body.len(),
        json_body
    )
}
