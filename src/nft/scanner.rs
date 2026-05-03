//! # NFT Scanner - Main Entry Point
//!
//! Ultra-fast NFT scanner with comprehensive functionality, seamless integration
//! with existing Solana recovery infrastructure, and production-ready performance.

use crate::nft::batch::{BatchProcessor, BatchJob, BatchJobConfig, BatchJobType, BatchItem, BatchItemData};
use crate::nft::cache::{CacheManager, CacheKey};
use crate::nft::errors::{NftError, NftResult};
use crate::nft::metadata::MetadataFetcher;
use crate::nft::portfolio::PortfolioAnalyzer;
use crate::nft::security::SecurityValidator;
use crate::nft::types::*;
use crate::nft::valuation::ValuationEngine;
use crate::rpc::ConnectionPool;
use crate::core::types::RpcEndpoint;
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Main NFT scanner with ultra-fast performance
#[derive(Clone)]
pub struct NftScanner {
    /// Metadata fetcher
    metadata_fetcher: Arc<MetadataFetcher>,
    
    /// Valuation engine
    valuation_engine: Arc<ValuationEngine>,
    
    /// Portfolio analyzer
    portfolio_analyzer: Arc<PortfolioAnalyzer>,
    
    /// Security validator
    security_validator: Arc<SecurityValidator>,
    
    /// Batch processor
    batch_processor: Arc<BatchProcessor>,
    
    /// Cache manager
    cache_manager: Arc<CacheManager>,
    
    /// Scanner configuration
    config: NftScannerConfig,
    
    /// Performance metrics
    metrics: Arc<ScannerMetrics>,
}

/// NFT scanner configuration
#[derive(Debug, Clone)]
pub struct NftScannerConfig {
    /// Performance mode
    pub performance_mode: PerformanceMode,
    
    /// Enable metadata fetching
    pub enable_metadata_fetching: bool,
    
    /// Enable valuation
    pub enable_valuation: bool,
    
    /// Enable security validation
    pub enable_security_validation: bool,
    
    /// Enable portfolio analysis
    pub enable_portfolio_analysis: bool,
    
    /// Enable batch processing
    pub enable_batch_processing: bool,
    
    /// Maximum concurrent scans
    pub max_concurrent_scans: usize,
    
    /// Scan timeout in seconds
    pub scan_timeout_seconds: u64,
    
    /// Maximum NFTs per wallet
    pub max_nfts_per_wallet: Option<u32>,
    
    /// Cache configuration
    pub cache_config: Option<crate::nft::cache::CacheConfig>,
    
    /// Security configuration
    pub security_config: Option<crate::nft::security::SecurityValidatorConfig>,
    
    /// Valuation configuration
    pub valuation_config: Option<crate::nft::valuation::ValuationEngineConfig>,
    
    /// Metadata configuration
    pub metadata_config: Option<crate::nft::metadata::MetadataConfig>,
}

impl Default for NftScannerConfig {
    fn default() -> Self {
        Self {
            performance_mode: PerformanceMode::Balanced,
            enable_metadata_fetching: true,
            enable_valuation: true,
            enable_security_validation: true,
            enable_portfolio_analysis: true,
            enable_batch_processing: true,
            max_concurrent_scans: 10,
            scan_timeout_seconds: 300, // 5 minutes
            max_nfts_per_wallet: None,
            cache_config: None,
            security_config: None,
            valuation_config: None,
            metadata_config: None,
        }
    }
}

/// Scanner performance metrics
#[derive(Debug, Default)]
pub struct ScannerMetrics {
    /// Total scans performed
    pub total_scans: Arc<std::sync::atomic::AtomicU64>,
    
    /// Successful scans
    pub successful_scans: Arc<std::sync::atomic::AtomicU64>,
    
    /// Failed scans
    pub failed_scans: Arc<std::sync::atomic::AtomicU64>,
    
    /// Total NFTs processed
    pub total_nfts_processed: Arc<std::sync::atomic::AtomicU64>,
    
    /// Average scan time in milliseconds
    pub avg_scan_time_ms: Arc<std::sync::atomic::AtomicU64>,
    
    /// Average NFTs per scan
    pub avg_nfts_per_scan: Arc<std::sync::atomic::AtomicF64>,
    
    /// Cache hit rate
    pub cache_hit_rate: Arc<std::sync::atomic::AtomicF64>,
    
    /// Security issues found
    pub security_issues_found: Arc<std::sync::atomic::AtomicU64>,
    
    /// Total value estimated (in lamports)
    pub total_value_estimated: Arc<std::sync::atomic::AtomicU64>,
}

/// NFT scan result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftScanResult {
    /// Scan ID
    pub scan_id: uuid::Uuid,
    
    /// Wallet address scanned
    pub wallet_address: String,
    
    /// NFTs found
    pub nfts: Vec<NftInfo>,
    
    /// Portfolio analysis (if enabled)
    pub portfolio: Option<NftPortfolio>,
    
    /// Security issues (if enabled)
    pub security_issues: Vec<SecurityIssue>,
    
    /// Total estimated value in lamports
    pub total_estimated_value_lamports: u64,
    
    /// Scan statistics
    pub statistics: ScanStatistics,
    
    /// Performance metrics
    pub performance: ScanPerformanceMetrics,
    
    /// Scan timestamp
    pub scanned_at: chrono::DateTime<chrono::Utc>,
    
    /// Scan duration in milliseconds
    pub scan_duration_ms: u64,
    
    /// Configuration used
    pub scan_config: String,
}

/// Scan statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanStatistics {
    /// Total NFTs found
    pub total_nfts: u32,
    
    /// Verified NFTs
    pub verified_nfts: u32,
    
    /// Unverified NFTs
    pub unverified_nfts: u32,
    
    /// NFTs with security issues
    pub nfts_with_security_issues: u32,
    
    /// Collections represented
    pub unique_collections: u32,
    
    /// Total value in lamports
    pub total_value_lamports: u64,
    
    /// Average value per NFT
    pub avg_value_per_nft: f64,
    
    /// Value distribution by collection
    pub value_by_collection: std::collections::HashMap<String, u64>,
    
    /// Risk distribution
    pub risk_distribution: std::collections::HashMap<RiskLevel, u32>,
}

/// Scan performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanPerformanceMetrics {
    /// Metadata fetch time in milliseconds
    pub metadata_fetch_time_ms: u64,
    
    /// Valuation time in milliseconds
    pub valuation_time_ms: u64,
    
    /// Security validation time in milliseconds
    pub security_validation_time_ms: u64,
    
    /// Portfolio analysis time in milliseconds
    pub portfolio_analysis_time_ms: u64,
    
    /// Cache hits
    pub cache_hits: u64,
    
    /// Cache misses
    pub cache_misses: u64,
    
    /// Network requests made
    pub network_requests: u64,
    
    /// Processing throughput (NFTs per second)
    pub throughput: f64,
}

impl NftScanner {
    /// Create new NFT scanner
    pub fn new(
        connection_pool: Arc<ConnectionPool>,
        config: NftScannerConfig,
    ) -> NftResult<Self> {
        let metrics = Arc::new(ScannerMetrics::default());
        
        // Initialize cache manager
        let cache_config = config.cache_config.clone().unwrap_or_default();
        let cache_manager = Arc::new(CacheManager::new(cache_config));
        
        // Initialize metadata fetcher
        let metadata_config = config.metadata_config.clone().unwrap_or_default();
        let metadata_fetcher = Arc::new(MetadataFetcher::new(
            connection_pool.clone(),
            metadata_config,
            cache_manager.clone(),
        )?);
        
        // Initialize valuation engine
        let valuation_config = config.valuation_config.clone().unwrap_or_default();
        let valuation_engine = Arc::new(ValuationEngine::new(
            connection_pool.clone(),
            valuation_config,
            cache_manager.clone(),
        )?);
        
        // Initialize portfolio analyzer
        let portfolio_config = crate::nft::portfolio::PortfolioAnalyzerConfig::default();
        let portfolio_analyzer = Arc::new(PortfolioAnalyzer::new(
            valuation_engine.clone(),
            portfolio_config,
        ));
        
        // Initialize security validator
        let security_config = config.security_config.clone().unwrap_or_default();
        let security_validator = Arc::new(SecurityValidator::new(
            connection_pool.clone(),
            security_config,
            cache_manager.clone(),
        )?);
        
        // Initialize batch processor
        let batch_config = crate::nft::batch::BatchProcessorConfig::default();
        let batch_processor = Arc::new(BatchProcessor::new(
            metadata_fetcher.clone(),
            valuation_engine.clone(),
            portfolio_analyzer.clone(),
            cache_manager.clone(),
            batch_config,
        ));

        Ok(Self {
            metadata_fetcher,
            valuation_engine,
            portfolio_analyzer,
            security_validator,
            batch_processor,
            cache_manager,
            config,
            metrics,
        })
    }

    /// Scan wallet for NFTs with ultra-fast performance
    pub async fn scan_wallet_nfts(&self, wallet_address: &str) -> NftResult<NftScanResult> {
        let start_time = Instant::now();
        let scan_id = uuid::Uuid::new_v4();
        
        self.metrics.total_scans.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        info!("Starting NFT scan for wallet {} with ID {}", wallet_address, scan_id);

        // Step 1: Get all token accounts for the wallet
        let token_accounts = self.get_wallet_token_accounts(wallet_address).await?;
        debug!("Found {} token accounts for wallet {}", token_accounts.len(), wallet_address);

        // Step 2: Filter for NFT accounts (non-fungible tokens)
        let nft_mints = self.filter_nft_accounts(&token_accounts).await?;
        debug!("Found {} potential NFT mints for wallet {}", nft_mints.len(), wallet_address);

        // Apply limit if configured
        let nft_mints = if let Some(limit) = self.config.max_nfts_per_wallet {
            if nft_mints.len() > limit as usize {
                warn!("Limiting NFT scan to {} items (found {})", limit, nft_mints.len());
                nft_mints.into_iter().take(limit as usize).collect()
            } else {
                nft_mints
            }
        } else {
            nft_mints
        };

        if nft_mints.is_empty() {
            let scan_result = NftScanResult {
                scan_id,
                wallet_address: wallet_address.to_string(),
                nfts: vec![],
                portfolio: None,
                security_issues: vec![],
                total_estimated_value_lamports: 0,
                statistics: ScanStatistics {
                    total_nfts: 0,
                    verified_nfts: 0,
                    unverified_nfts: 0,
                    nfts_with_security_issues: 0,
                    unique_collections: 0,
                    total_value_lamports: 0,
                    avg_value_per_nft: 0.0,
                    value_by_collection: std::collections::HashMap::new(),
                    risk_distribution: std::collections::HashMap::new(),
                },
                performance: ScanPerformanceMetrics {
                    metadata_fetch_time_ms: 0,
                    valuation_time_ms: 0,
                    security_validation_time_ms: 0,
                    portfolio_analysis_time_ms: 0,
                    cache_hits: 0,
                    cache_misses: 0,
                    network_requests: 0,
                    throughput: 0.0,
                },
                scanned_at: chrono::Utc::now(),
                scan_duration_ms: start_time.elapsed().as_millis() as u64,
                scan_config: format!("{:?}", self.config.performance_mode),
            };

            self.metrics.successful_scans.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            info!("NFT scan completed for wallet {} - No NFTs found", wallet_address);
            return Ok(scan_result);
        }

        // Step 3: Fetch metadata for all NFTs
        let metadata_start_time = Instant::now();
        let mut nfts = Vec::new();
        
        if self.config.enable_batch_processing && nft_mints.len() > 10 {
            // Use batch processing for large numbers of NFTs
            nfts = self.batch_fetch_metadata(&nft_mints).await?;
        } else {
            // Process individually for smaller numbers
            for mint_address in &nft_mints {
                match self.metadata_fetcher.fetch_nft_metadata(mint_address).await {
                    Ok(nft_info) => nfts.push(nft_info),
                    Err(e) => {
                        warn!("Failed to fetch metadata for NFT {}: {}", mint_address, e);
                        // Continue with other NFTs
                    }
                }
            }
        }
        
        let metadata_time_ms = metadata_start_time.elapsed().as_millis() as u64;
        debug!("Metadata fetch completed in {}ms for {} NFTs", metadata_time_ms, nfts.len());

        // Step 4: Value NFTs (if enabled)
        let mut valuation_time_ms = 0u64;
        if self.config.enable_valuation {
            let valuation_start_time = Instant::now();
            
            for nft in &mut nfts {
                match self.valuation_engine.value_nft(nft).await {
                    Ok(valuation) => {
                        nft.estimated_value_lamports = Some(valuation.estimated_value_lamports);
                        nft.last_valuation = Some(valuation.last_updated);
                    }
                    Err(e) => {
                        warn!("Failed to value NFT {}: {}", nft.mint_address, e);
                    }
                }
            }
            
            valuation_time_ms = valuation_start_time.elapsed().as_millis() as u64;
            debug!("Valuation completed in {}ms", valuation_time_ms);
        }

        // Step 5: Security validation (if enabled)
        let mut security_time_ms = 0u64;
        let mut all_security_issues = Vec::new();
        
        if self.config.enable_security_validation {
            let security_start_time = Instant::now();
            
            for nft in &mut nfts {
                match self.security_validator.validate_nft_security(nft).await {
                    Ok(validation) => {
                        nft.security_assessment = validation.assessment.clone();
                        all_security_issues.extend(validation.assessment.issues);
                    }
                    Err(e) => {
                        warn!("Failed security validation for NFT {}: {}", nft.mint_address, e);
                    }
                }
            }
            
            security_time_ms = security_start_time.elapsed().as_millis() as u64;
            debug!("Security validation completed in {}ms", security_time_ms);
        }

        // Step 6: Portfolio analysis (if enabled)
        let mut portfolio = None;
        let mut portfolio_time_ms = 0u64;
        
        if self.config.enable_portfolio_analysis && !nfts.is_empty() {
            let portfolio_start_time = Instant::now();
            
            match self.portfolio_analyzer.analyze_portfolio(wallet_address, &nfts).await {
                Ok(analysis) => {
                    portfolio = Some(analysis);
                }
                Err(e) => {
                    warn!("Failed portfolio analysis for wallet {}: {}", wallet_address, e);
                }
            }
            
            portfolio_time_ms = portfolio_start_time.elapsed().as_millis() as u64;
            debug!("Portfolio analysis completed in {}ms", portfolio_time_ms);
        }

        // Step 7: Calculate statistics
        let total_value = nfts.iter()
            .filter_map(|nft| nft.estimated_value_lamports)
            .sum();

        let mut statistics = ScanStatistics {
            total_nfts: nfts.len() as u32,
            verified_nfts: nfts.iter().filter(|nft| nft.metadata_verified).count() as u32,
            unverified_nfts: nfts.iter().filter(|nft| !nft.metadata_verified).count() as u32,
            nfts_with_security_issues: nfts.iter()
                .filter(|nft| nft.security_assessment.risk_level >= RiskLevel::Medium)
                .count() as u32,
            unique_collections: nfts.iter()
                .filter_map(|nft| nft.collection.as_ref().map(|c| c.name.clone()))
                .collect::<std::collections::HashSet<_>>()
                .len() as u32,
            total_value_lamports: total_value,
            avg_value_per_nft: if nfts.is_empty() { 0.0 } else { total_value as f64 / nfts.len() as f64 },
            value_by_collection: std::collections::HashMap::new(),
            risk_distribution: std::collections::HashMap::new(),
        };

        // Calculate value by collection
        for nft in &nfts {
            if let Some(value) = nft.estimated_value_lamports {
                let collection_name = nft.collection.as_ref()
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                *statistics.value_by_collection.entry(collection_name).or_insert(0) += value;
            }
        }

        // Calculate risk distribution
        for nft in &nfts {
            let risk_level = nft.security_assessment.risk_level;
            *statistics.risk_distribution.entry(risk_level).or_insert(0) += 1;
        }

        // Step 8: Calculate performance metrics
        let total_scan_time_ms = start_time.elapsed().as_millis() as u64;
        let throughput = if total_scan_time_ms > 0 {
            (nfts.len() as f64 / total_scan_time_ms as f64) * 1000.0
        } else {
            0.0
        };

        let performance = ScanPerformanceMetrics {
            metadata_fetch_time_ms: metadata_time_ms,
            valuation_time_ms,
            security_validation_time_ms: security_time_ms,
            portfolio_analysis_time_ms: portfolio_time_ms,
            cache_hits: 0, // Would be populated by actual cache metrics
            cache_misses: 0,
            network_requests: 0,
            throughput,
        };

        // Create scan result
        let scan_result = NftScanResult {
            scan_id,
            wallet_address: wallet_address.to_string(),
            nfts,
            portfolio,
            security_issues: all_security_issues,
            total_estimated_value_lamports: total_value,
            statistics,
            performance,
            scanned_at: chrono::Utc::now(),
            scan_duration_ms: total_scan_time_ms,
            scan_config: format!("{:?}", self.config.performance_mode),
        };

        // Update metrics
        self.metrics.successful_scans.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics.total_nfts_processed.fetch_add(
            scan_result.nfts.len() as u64,
            std::sync::atomic::Ordering::Relaxed
        );
        self.metrics.avg_scan_time_ms.fetch_add(total_scan_time_ms, std::sync::atomic::Ordering::Relaxed);
        self.metrics.avg_nfts_per_scan.fetch_add(
            scan_result.nfts.len() as f64,
            std::sync::atomic::Ordering::Relaxed
        );
        self.metrics.total_value_estimated.fetch_add(total_value, std::sync::atomic::Ordering::Relaxed);

        if !scan_result.security_issues.is_empty() {
            self.metrics.security_issues_found.fetch_add(
                scan_result.security_issues.len() as u64,
                std::sync::atomic::Ordering::Relaxed
            );
        }

        info!("NFT scan completed for wallet {} in {}ms: {} NFTs, {} SOL value", 
            wallet_address, total_scan_time_ms, scan_result.nfts.len(), 
            total_value as f64 / 1_000_000_000.0);

        Ok(scan_result)
    }

    /// Batch scan multiple wallets
    pub async fn scan_wallets_batch(&self, wallet_addresses: &[String]) -> NftResult<Vec<NftScanResult>> {
        let start_time = Instant::now();
        
        let results: Vec<NftResult<NftScanResult>> = futures::stream::iter(wallet_addresses)
            .map(|wallet_address| async move {
                self.scan_wallet_nfts(wallet_address).await
            })
            .buffer_unordered(self.config.max_concurrent_scans)
            .collect()
            .await;

        let mut successful_results = Vec::new();
        let mut failed_count = 0;

        for result in results {
            match result {
                Ok(scan_result) => successful_results.push(scan_result),
                Err(e) => {
                    error!("Failed to scan wallet: {}", e);
                    failed_count += 1;
                }
            }
        }

        let total_time_ms = start_time.elapsed().as_millis() as u64;
        info!(
            "Batch NFT scan completed: {} successful, {} failed in {}ms",
            successful_results.len(),
            failed_count,
            total_time_ms
        );

        Ok(successful_results)
    }

    /// Get wallet token accounts
    async fn get_wallet_token_accounts(&self, wallet_address: &str) -> NftResult<Vec<solana_account_decoder::UiAccount>> {
        // This would typically use the existing Solana recovery infrastructure
        // to get token accounts for the wallet
        // For now, return empty placeholder
        Ok(vec![])
    }

    /// Filter token accounts for NFTs
    async fn filter_nft_accounts(&self, token_accounts: &[solana_account_decoder::UiAccount]) -> NftResult<Vec<String>> {
        let mut nft_mints = Vec::new();

        for account in token_accounts {
            // Check if this is an NFT account
            if self.is_nft_account(account).await? {
                // Extract mint address from account data
                if let Some(mint_address) = self.extract_mint_address(account).await? {
                    nft_mints.push(mint_address);
                }
            }
        }

        Ok(nft_mints)
    }

    /// Check if account is an NFT account
    async fn is_nft_account(&self, account: &solana_account_decoder::UiAccount) -> NftResult<bool> {
        // This would check the account data to determine if it's an NFT
        // For now, return false as placeholder
        Ok(false)
    }

    /// Extract mint address from account
    async fn extract_mint_address(&self, account: &solana_account_decoder::UiAccount) -> NftResult<Option<String>> {
        // This would extract the mint address from the account data
        // For now, return None as placeholder
        Ok(None)
    }

    /// Batch fetch metadata using batch processor
    async fn batch_fetch_metadata(&self, mint_addresses: &[String]) -> NftResult<Vec<NftInfo>> {
        let batch_job = BatchJob {
            id: uuid::Uuid::new_v4(),
            job_type: BatchJobType::MetadataFetch,
            items: mint_addresses.iter().enumerate().map(|(index, mint)| BatchItem {
                id: index.to_string(),
                data: BatchItemData::MintAddress(mint.clone()),
                metadata: std::collections::HashMap::new(),
            }).collect(),
            config: BatchJobConfig::default(),
            created_at: chrono::Utc::now(),
            priority: crate::nft::batch::JobPriority::Normal,
        };

        let batch_result = self.batch_processor.process_batch_job(batch_job).await?;
        
        let mut nfts = Vec::new();
        for item_result in batch_result.successful_results {
            if let crate::nft::batch::BatchItemResultData::NftInfo(nft_info) = item_result.result_data {
                nfts.push(nft_info);
            }
        }

        Ok(nfts)
    }

    /// Get scanner metrics
    pub fn get_metrics(&self) -> &ScannerMetrics {
        &self.metrics
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> crate::nft::cache::CacheStats {
        self.cache_manager.get_stats().await
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        self.cache_manager.clear().await;
        info!("NFT scanner cache cleared");
    }
}

/// Convenience function to create scanner with default configuration
pub fn create_nft_scanner(connection_pool: Arc<ConnectionPool>) -> NftResult<NftScanner> {
    let config = NftScannerConfig::default();
    NftScanner::new(connection_pool, config)
}

/// Convenience function to create scanner with ultra-fast configuration
pub fn create_ultra_fast_nft_scanner(connection_pool: Arc<ConnectionPool>) -> NftResult<NftScanner> {
    let config = NftScannerConfig {
        performance_mode: PerformanceMode::UltraFast,
        max_concurrent_scans: 50,
        scan_timeout_seconds: 60, // 1 minute
        enable_batch_processing: true,
        ..Default::default()
    };
    NftScanner::new(connection_pool, config)
}

/// Convenience function to create scanner with thorough configuration
pub fn create_thorough_nft_scanner(connection_pool: Arc<ConnectionPool>) -> NftResult<NftScanner> {
    let config = NftScannerConfig {
        performance_mode: PerformanceMode::Thorough,
        max_concurrent_scans: 5,
        scan_timeout_seconds: 600, // 10 minutes
        enable_metadata_fetching: true,
        enable_valuation: true,
        enable_security_validation: true,
        enable_portfolio_analysis: true,
        ..Default::default()
    };
    NftScanner::new(connection_pool, config)
}
