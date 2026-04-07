//! API client example
//! 
//! This example demonstrates how to interact with the Solana Recover API
//! using HTTP requests to scan wallets and manage batch operations.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, error, warn};

#[derive(Debug, Serialize)]
struct ScanRequest {
    wallet_address: String,
    fee_percentage: Option<f64>,
}

#[derive(Debug, Serialize)]
struct BatchScanRequest {
    wallet_addresses: Vec<String>,
    fee_percentage: Option<f64>,
    user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScanResponse {
    id: String,
    wallet_address: String,
    status: String,
    result: Option<WalletInfo>,
    error: Option<String>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct BatchScanResponse {
    id: String,
    total_wallets: usize,
    successful_scans: usize,
    failed_scans: usize,
    total_recoverable_sol: f64,
    estimated_fee_sol: f64,
    results: Vec<ScanResponse>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct WalletInfo {
    address: String,
    total_accounts: u64,
    empty_accounts: u64,
    recoverable_sol: f64,
    recoverable_lamports: u64,
    empty_account_addresses: Vec<String>,
    scan_time_ms: u64,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
    version: String,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct MetricsResponse {
    total_scans: u64,
    successful_scans: u64,
    failed_scans: u64,
    total_recoverable_sol: f64,
    average_scan_time_ms: f64,
    active_connections: u64,
}

struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client, base_url }
    }
    
    async fn health_check(&self) -> Result<HealthResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/health", self.base_url);
        let response = self.client.get(&url).send().await?;
        
        if response.status().is_success() {
            let health: HealthResponse = response.json().await?;
            Ok(health)
        } else {
            Err(format!("Health check failed: {}", response.status()).into())
        }
    }
    
    async fn scan_wallet(&self, address: &str, fee_percentage: Option<f64>) -> Result<ScanResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/scan", self.base_url);
        
        let request = ScanRequest {
            wallet_address: address.to_string(),
            fee_percentage,
        };
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;
        
        if response.status().is_success() {
            let scan_result: ScanResponse = response.json().await?;
            Ok(scan_result)
        } else {
            let error_text = response.text().await?;
            Err(format!("Scan request failed: {} - {}", response.status(), error_text).into())
        }
    }
    
    async fn batch_scan(&self, addresses: Vec<String>, fee_percentage: Option<f64>) -> Result<BatchScanResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/batch-scan", self.base_url);
        
        let request = BatchScanRequest {
            wallet_addresses: addresses,
            fee_percentage,
            user_id: Some("api_client_example".to_string()),
        };
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;
        
        if response.status().is_success() {
            let batch_result: BatchScanResponse = response.json().await?;
            Ok(batch_result)
        } else {
            let error_text = response.text().await?;
            Err(format!("Batch scan request failed: {} - {}", response.status(), error_text).into())
        }
    }
    
    async fn get_metrics(&self) -> Result<MetricsResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/metrics", self.base_url);
        let response = self.client.get(&url).send().await?;
        
        if response.status().is_success() {
            let metrics: MetricsResponse = response.json().await?;
            Ok(metrics)
        } else {
            Err(format!("Metrics request failed: {}", response.status()).into())
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let logging_config = solana_recover::LoggingConfig {
        level: "info".to_string(),
        format: solana_recover::utils::LogFormat::Pretty,
        output: solana_recover::utils::LogOutput::Stdout,
        file_path: None,
        json_fields: vec![],
    };
    
    solana_recover::Logger::init(logging_config)?;
    
    info!("Starting API client example");
    
    // Create API client
    let api_client = ApiClient::new("http://localhost:8080".to_string());
    
    // Health check
    info!("Performing health check...");
    match api_client.health_check().await {
        Ok(health) => {
            info!("✓ API is healthy");
            info!("Status: {}", health.status);
            info!("Version: {}", health.version);
            info!("Timestamp: {}", health.timestamp);
        }
        Err(e) => {
            error!("✗ Health check failed: {}", e);
            error!("Make sure the Solana Recover API server is running on http://localhost:8080");
            return Err(e);
        }
    }
    
    // Example wallet addresses
    let wallet_addresses = vec![
        "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "11111111111111111111111111111111112",
    ];
    
    // Single wallet scan examples
    info!("\n=== Single Wallet Scan Examples ===");
    
    for (i, address) in wallet_addresses.iter().take(3).enumerate() {
        info!("Scanning wallet {}: {}", i + 1, address);
        
        match api_client.scan_wallet(address, Some(0.15)).await {
            Ok(response) => {
                info!("✓ Scan completed for wallet: {}", address);
                info!("Scan ID: {}", response.id);
                info!("Status: {}", response.status);
                
                if let Some(wallet_info) = response.result {
                    info!("Total accounts: {}", wallet_info.total_accounts);
                    info!("Empty accounts: {}", wallet_info.empty_accounts);
                    info!("Recoverable SOL: {:.9}", wallet_info.recoverable_sol);
                    
                    if !wallet_info.empty_account_addresses.is_empty() {
                        info!("Empty account addresses:");
                        for (j, addr) in wallet_info.empty_account_addresses.iter().take(3).enumerate() {
                            info!("  {}. {}", j + 1, addr);
                        }
                        if wallet_info.empty_account_addresses.len() > 3 {
                            info!("  ... and {} more", wallet_info.empty_account_addresses.len() - 3);
                        }
                    }
                }
                
                if let Some(error) = response.error {
                    warn!("Scan error: {}", error);
                }
            }
            Err(e) => {
                error!("✗ Failed to scan wallet {}: {}", address, e);
            }
        }
        
        println!("{}", "-".repeat(60));
    }
    
    // Batch scan example
    info!("\n=== Batch Scan Example ===");
    
    let batch_addresses = wallet_addresses.clone();
    info!("Starting batch scan for {} wallets", batch_addresses.len());
    
    let start_time = std::time::Instant::now();
    
    match api_client.batch_scan(batch_addresses, Some(0.10)).await {
        Ok(response) => {
            let duration = start_time.elapsed();
            
            info!("✓ Batch scan completed successfully!");
            info!("Batch ID: {}", response.id);
            info!("Total wallets: {}", response.total_wallets);
            info!("Successful scans: {}", response.successful_scans);
            info!("Failed scans: {}", response.failed_scans);
            info!("Total recoverable SOL: {:.9}", response.total_recoverable_sol);
            info!("Estimated fee SOL: {:.9}", response.estimated_fee_sol);
            info!("Processing time: {:?}", duration);
            
            if let Some(duration_ms) = response.duration_ms {
                info!("Server processing time: {}ms", duration_ms);
            }
            
            // Show individual results
            println!("\n=== Individual Results ===");
            for (i, result) in response.results.iter().enumerate() {
                match result.status.as_str() {
                    "completed" => {
                        if let Some(wallet_info) = &result.result {
                            info!("OK {}: {:.9} SOL recoverable ({} accounts, {} empty)",
                                result.wallet_address,
                                wallet_info.recoverable_sol,
                                wallet_info.total_accounts,
                                wallet_info.empty_accounts
                            );
                        }
                    }
                    "failed" => {
                        error!("ERROR {}: Failed - {}",
                            result.wallet_address,
                            result.error.as_deref().unwrap_or("Unknown error")
                        );
                    }
                    _ => {
                        warn!("WARNING {}: Status: {}", result.wallet_address, result.status);
                    }
                }
            }
        }
        Err(e) => {
            error!("✗ Batch scan failed: {}", e);
        }
    }
    
    // Get API metrics
    info!("\n=== API Metrics ===");
    
    match api_client.get_metrics().await {
        Ok(metrics) => {
            info!("Total scans: {}", metrics.total_scans);
            info!("Successful scans: {}", metrics.successful_scans);
            info!("Failed scans: {}", metrics.failed_scans);
            info!("Total recoverable SOL: {:.9}", metrics.total_recoverable_sol);
            info!("Average scan time: {:.2} ms", metrics.average_scan_time_ms);
            info!("Active connections: {}", metrics.active_connections);
            
            if metrics.total_scans > 0 {
                let success_rate = (metrics.successful_scans as f64 / metrics.total_scans as f64) * 100.0;
                info!("Success rate: {:.1}%", success_rate);
            }
        }
        Err(e) => {
            warn!("Failed to get metrics: {}", e);
        }
    }
    
    // Error handling examples
    info!("\n=== Error Handling Examples ===");
    
    // Invalid wallet address
    info!("Testing invalid wallet address...");
    match api_client.scan_wallet("invalid_address", None).await {
        Ok(_) => {
            warn!("Expected error for invalid address, but got success");
        }
        Err(e) => {
            info!("✓ Correctly handled invalid address error: {}", e);
        }
    }
    
    // Invalid fee percentage
    info!("Testing invalid fee percentage...");
    match api_client.scan_wallet(wallet_addresses[0], Some(1.5)).await {
        Ok(_) => {
            warn!("Expected error for invalid fee percentage, but got success");
        }
        Err(e) => {
            info!("✓ Correctly handled invalid fee percentage error: {}", e);
        }
    }
    
    // Empty batch request
    info!("Testing empty batch request...");
    match api_client.batch_scan(vec![], None).await {
        Ok(_) => {
            warn!("Expected error for empty batch, but got success");
        }
        Err(e) => {
            info!("✓ Correctly handled empty batch error: {}", e);
        }
    }
    
    info!("\n=== Usage Tips ===");
    info!("1. Always check API health before making requests");
    info!("2. Handle network timeouts gracefully");
    info!("3. Validate wallet addresses before sending requests");
    info!("4. Use appropriate fee percentages (0.0 - 1.0)");
    info!("5. Monitor API metrics for performance insights");
    info!("6. Implement retry logic for failed requests");
    info!("7. Use batch scanning for multiple wallets to improve efficiency");
    
    info!("API client example completed");
    Ok(())
}
