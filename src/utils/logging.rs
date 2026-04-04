use tracing::{info, warn, error, debug, trace};
use tracing_subscriber::{EnvFilter};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: LogFormat,
    pub output: LogOutput,
    pub file_path: Option<String>,
    pub json_fields: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum LogFormat {
    Pretty,
    Json,
    Compact,
}

#[derive(Debug, Clone)]
pub enum LogOutput {
    Stdout,
    Stderr,
    File(String),
    Both(String), // Both stdout and file
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Pretty,
            output: LogOutput::Stdout,
            file_path: None,
            json_fields: vec![
                "timestamp".to_string(),
                "level".to_string(),
                "target".to_string(),
                "message".to_string(),
                "span".to_string(),
            ],
        }
    }
}

pub struct Logger;

impl Logger {
    pub fn init(config: LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
        let level = tracing::Level::from_str(&config.level)
            .unwrap_or(tracing::Level::INFO);

        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&config.level));

        match config.format {
            LogFormat::Json => {
                tracing_subscriber::fmt()
                    .json()
                    .with_max_level(level)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_env_filter(env_filter)
                    .init();
            }
            LogFormat::Pretty => {
                tracing_subscriber::fmt()
                    .pretty()
                    .with_max_level(level)
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_thread_names(false)
                    .with_file(true)
                    .with_line_number(true)
                    .with_env_filter(env_filter)
                    .init();
            }
            LogFormat::Compact => {
                tracing_subscriber::fmt()
                    .compact()
                    .with_max_level(level)
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_thread_names(false)
                    .with_file(false)
                    .with_line_number(false)
                    .with_env_filter(env_filter)
                    .init();
            }
        }

        info!("Logging initialized with level: {}", config.level);
        Ok(())
    }
}

// Structured logging helpers
pub struct StructuredLogger;

impl StructuredLogger {
    pub fn log_wallet_scan_start(wallet_address: &str, scan_id: &str) {
        info!(
            wallet_address = wallet_address,
            scan_id = scan_id,
            event = "wallet_scan_started",
            "Starting wallet scan"
        );
    }

    pub fn log_wallet_scan_complete(wallet_address: &str, scan_id: &str, duration_ms: u64, recoverable_sol: f64) {
        info!(
            wallet_address = wallet_address,
            scan_id = scan_id,
            duration_ms = duration_ms,
            recoverable_sol = recoverable_sol,
            event = "wallet_scan_completed",
            "Wallet scan completed successfully"
        );
    }

    pub fn log_wallet_scan_error(wallet_address: &str, scan_id: &str, error: &str) {
        error!(
            wallet_address = wallet_address,
            scan_id = scan_id,
            error = error,
            event = "wallet_scan_failed",
            "Wallet scan failed"
        );
    }

    pub fn log_batch_scan_start(batch_id: &str, wallet_count: usize) {
        info!(
            batch_id = batch_id,
            wallet_count = wallet_count,
            event = "batch_scan_started",
            "Starting batch wallet scan"
        );
    }

    pub fn log_batch_scan_complete(batch_id: &str, duration_ms: u64, successful: usize, failed: usize, total_recoverable: f64) {
        info!(
            batch_id = batch_id,
            duration_ms = duration_ms,
            successful_scans = successful,
            failed_scans = failed,
            total_recoverable_sol = total_recoverable,
            event = "batch_scan_completed",
            "Batch scan completed"
        );
    }

    pub fn log_rpc_request(endpoint: &str, method: &str, duration_ms: u64) {
        debug!(
            rpc_endpoint = endpoint,
            method = method,
            duration_ms = duration_ms,
            event = "rpc_request_completed",
            "RPC request completed"
        );
    }

    pub fn log_rpc_error(endpoint: &str, method: &str, error: &str) {
        warn!(
            rpc_endpoint = endpoint,
            method = method,
            error = error,
            event = "rpc_request_failed",
            "RPC request failed"
        );
    }

    pub fn log_api_request(method: &str, path: &str, status_code: u16, duration_ms: u64) {
        info!(
            http_method = method,
            http_path = path,
            http_status_code = status_code,
            duration_ms = duration_ms,
            event = "api_request_completed",
            "API request completed"
        );
    }

    pub fn log_cache_hit(key: &str) {
        trace!(
            cache_key = key,
            event = "cache_hit",
            "Cache hit"
        );
    }

    pub fn log_cache_miss(key: &str) {
        trace!(
            cache_key = key,
            event = "cache_miss",
            "Cache miss"
        );
    }

    pub fn log_wallet_connection(wallet_type: &str, connection_id: &str) {
        info!(
            wallet_type = wallet_type,
            connection_id = connection_id,
            event = "wallet_connected",
            "Wallet connected successfully"
        );
    }

    pub fn log_wallet_disconnection(wallet_type: &str, connection_id: &str) {
        info!(
            wallet_type = wallet_type,
            connection_id = connection_id,
            event = "wallet_disconnected",
            "Wallet disconnected"
        );
    }

    pub fn log_fee_calculation(wallet_address: &str, recoverable: f64, fee: f64, net: f64) {
        info!(
            wallet_address = wallet_address,
            recoverable_sol = recoverable,
            fee_sol = fee,
            net_sol = net,
            event = "fee_calculated",
            "Fee calculated for wallet"
        );
    }

    pub fn log_security_event(event_type: &str, details: &str, user_id: Option<&str>) {
        warn!(
            security_event_type = event_type,
            details = details,
            user_id = user_id,
            event = "security_event",
            "Security event detected"
        );
    }

    pub fn log_performance_metric(metric_name: &str, value: f64, unit: &str) {
        info!(
            metric_name = metric_name,
            metric_value = value,
            metric_unit = unit,
            event = "performance_metric",
            "Performance metric recorded"
        );
    }
}

// Performance logging macro
#[macro_export]
macro_rules! log_performance {
    ($name:expr, $block:block) => {
        {
            let start = std::time::Instant::now();
            let result = $block;
            let duration = start.elapsed();
            
            $crate::utils::logging::StructuredLogger::log_performance_metric(
                $name,
                duration.as_millis() as f64,
                "ms"
            );
            
            result
        }
    };
}

// Error logging macro with context
#[macro_export]
macro_rules! log_error {
    ($error:expr, $context:expr) => {
        {
            error!(
                error = %$error,
                context = $context,
                event = "error_occurred",
                "Error occurred"
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert!(matches!(config.format, LogFormat::Pretty));
        assert!(matches!(config.output, LogOutput::Stdout));
    }

    #[test]
    fn test_log_format_creation() {
        let config = LoggingConfig {
            level: "debug".to_string(),
            format: LogFormat::Json,
            output: LogOutput::Stdout,
            file_path: None,
            json_fields: vec!["timestamp".to_string()],
        };

        assert_eq!(config.level, "debug");
        assert!(matches!(config.format, LogFormat::Json));
    }

    #[test]
    fn test_structured_logger_helpers() {
        // These tests just verify the functions compile and don't panic
        // In a real test environment, you'd capture the log output
        StructuredLogger::log_wallet_scan_start("test_address", "test_scan_id");
        StructuredLogger::log_wallet_scan_complete("test_address", "test_scan_id", 1000, 0.1);
        StructuredLogger::log_wallet_scan_error("test_address", "test_scan_id", "test error");
        StructuredLogger::log_batch_scan_start("test_batch", 10);
        StructuredLogger::log_batch_scan_complete("test_batch", 5000, 8, 2, 0.5);
        StructuredLogger::log_rpc_request("https://api.mainnet-beta.solana.com", "getAccountInfo", 100);
        StructuredLogger::log_rpc_error("https://api.mainnet-beta.solana.com", "getAccountInfo", "timeout");
        StructuredLogger::log_api_request("GET", "/api/v1/scan", 200, 50);
        StructuredLogger::log_cache_hit("test_key");
        StructuredLogger::log_cache_miss("test_key");
        StructuredLogger::log_wallet_connection("phantom", "conn_123");
        StructuredLogger::log_wallet_disconnection("phantom", "conn_123");
        StructuredLogger::log_fee_calculation("test_address", 0.1, 0.015, 0.085);
        StructuredLogger::log_security_event("login_attempt", "failed login", Some("user_123"));
        StructuredLogger::log_performance_metric("scan_duration", 1500.0, "ms");
    }
}
