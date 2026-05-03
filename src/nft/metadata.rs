//! # NFT Metadata Fetching and Validation System
//!
//! Ultra-fast, secure, and highly customizable metadata fetching with
//! comprehensive validation, caching, and error handling.

use crate::nft::cache::{CacheManager, CacheKey};
use crate::nft::errors::{NftError, NftResult, RecoveryStrategy};
use crate::nft::types::*;
use crate::rpc::ConnectionPool;
use crate::core::types::RpcEndpoint;
use async_trait::async_trait;
use dashmap::DashMap;
use moka::future::Cache as MokaCache;
use rayon::prelude::*;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

/// Metadata fetcher with ultra-fast performance and comprehensive validation
#[derive(Clone)]
pub struct MetadataFetcher {
    /// RPC connection pool
    connection_pool: Arc<ConnectionPool>,
    
    /// Cache manager
    cache_manager: Arc<CacheManager>,
    
    /// Configuration
    config: MetadataConfig,
    
    /// Rate limiter
    rate_limiter: Arc<Semaphore>,
    
    /// Performance metrics
    metrics: Arc<MetadataMetrics>,
    
    /// HTTP client for metadata fetching
    http_client: reqwest::Client,
    
    /// Image processor
    image_processor: Arc<ImageProcessor>,
}

/// Metadata fetcher configuration
#[derive(Debug, Clone)]
pub struct MetadataConfig {
    /// Maximum concurrent metadata requests
    pub max_concurrent_requests: usize,
    
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    
    /// Maximum retry attempts
    pub max_retries: u32,
    
    /// Retry delay base in milliseconds
    pub retry_delay_ms: u64,
    
    /// Enable metadata validation
    pub enable_validation: bool,
    
    /// Enable image fetching
    pub enable_image_fetching: bool,
    
    /// Enable image validation
    pub enable_image_validation: bool,
    
    /// Maximum metadata size in bytes
    pub max_metadata_size_bytes: usize,
    
    /// Maximum image size in bytes
    pub max_image_size_bytes: usize,
    
    /// Allowed image formats
    pub allowed_image_formats: Vec<String>,
    
    /// Blocked domains
    pub blocked_domains: Vec<String>,
    
    /// Trusted domains
    pub trusted_domains: Vec<String>,
    
    /// User agent string
    pub user_agent: String,
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 20,
            request_timeout_ms: 10000,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_validation: true,
            enable_image_fetching: false,
            enable_image_validation: true,
            max_metadata_size_bytes: 1024 * 1024, // 1MB
            max_image_size_bytes: 10 * 1024 * 1024, // 10MB
            allowed_image_formats: vec![
                "image/png".to_string(),
                "image/jpeg".to_string(),
                "image/gif".to_string(),
                "image/webp".to_string(),
            ],
            blocked_domains: vec![],
            trusted_domains: vec![
                "arweave.net".to_string(),
                "nftstorage.link".to_string(),
                "ipfs.io".to_string(),
            ],
            user_agent: "Solana-Recover-NFT/1.0".to_string(),
        }
    }
}

/// Metadata performance metrics
#[derive(Debug, Default)]
pub struct MetadataMetrics {
    /// Total metadata fetches
    pub total_fetches: Arc<std::sync::atomic::AtomicU64>,
    
    /// Successful fetches
    pub successful_fetches: Arc<std::sync::atomic::AtomicU64>,
    
    /// Failed fetches
    pub failed_fetches: Arc<std::sync::atomic::AtomicU64>,
    
    /// Cache hits
    pub cache_hits: Arc<std::sync::atomic::AtomicU64>,
    
    /// Cache misses
    pub cache_misses: Arc<std::sync::atomic::AtomicU64>,
    
    /// Average fetch time in milliseconds
    pub avg_fetch_time_ms: Arc<std::sync::atomic::AtomicU64>,
    
    /// Total bytes fetched
    pub total_bytes_fetched: Arc<std::sync::atomic::AtomicU64>,
    
    /// Validation errors
    pub validation_errors: Arc<std::sync::atomic::AtomicU64>,
    
    /// Security issues found
    pub security_issues_found: Arc<std::sync::atomic::AtomicU64>,
}

/// Image processor for NFT images
#[derive(Clone)]
pub struct ImageProcessor {
    /// Configuration
    config: ImageProcessorConfig,
    
    /// HTTP client
    http_client: reqwest::Client,
}

/// Image processor configuration
#[derive(Debug, Clone)]
pub struct ImageProcessorConfig {
    /// Maximum image size
    pub max_size_bytes: usize,
    
    /// Allowed formats
    pub allowed_formats: Vec<String>,
    
    /// Enable EXIF data extraction
    pub enable_exif_extraction: bool,
    
    /// Enable image analysis
    pub enable_analysis: bool,
    
    /// Timeout in milliseconds
    pub timeout_ms: u64,
}

impl Default for ImageProcessorConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 10 * 1024 * 1024, // 10MB
            allowed_formats: vec![
                "image/png".to_string(),
                "image/jpeg".to_string(),
                "image/gif".to_string(),
                "image/webp".to_string(),
            ],
            enable_exif_extraction: true,
            enable_analysis: false,
            timeout_ms: 15000,
        }
    }
}

/// Metadata validation result
#[derive(Debug, Clone)]
pub struct MetadataValidationResult {
    /// Is metadata valid
    pub is_valid: bool,
    
    /// Validation errors
    pub errors: Vec<String>,
    
    /// Security issues
    pub security_issues: Vec<SecurityIssue>,
    
    /// Warnings
    pub warnings: Vec<String>,
    
    /// Validation score (0-100)
    pub validation_score: u8,
    
    /// Metadata completeness score (0-100)
    pub completeness_score: u8,
}

/// Trait for metadata fetching strategies
#[async_trait]
pub trait MetadataFetchStrategy: Send + Sync {
    /// Fetch metadata for a given URI
    async fn fetch_metadata(&self, uri: &str) -> NftResult<Value>;
    
    /// Get strategy name
    fn name(&self) -> &'static str;
    
    /// Check if strategy can handle the URI
    fn can_handle(&self, uri: &str) -> bool;
}

/// HTTP metadata fetcher
pub struct HttpMetadataFetcher {
    http_client: reqwest::Client,
    config: MetadataConfig,
}

/// IPFS metadata fetcher
pub struct IpfsMetadataFetcher {
    http_client: reqwest::Client,
    config: MetadataConfig,
    ipfs_gateways: Vec<String>,
}

/// Arweave metadata fetcher
pub struct ArweaveMetadataFetcher {
    http_client: reqwest::Client,
    config: MetadataConfig,
}

impl MetadataFetcher {
    /// Create new metadata fetcher
    pub fn new(
        connection_pool: Arc<ConnectionPool>,
        config: MetadataConfig,
        cache_manager: Arc<CacheManager>,
    ) -> NftResult<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.request_timeout_ms))
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| NftError::Configuration {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        let rate_limiter = Arc::new(Semaphore::new(config.max_concurrent_requests));
        let metrics = Arc::new(MetadataMetrics::default());
        let image_processor = Arc::new(ImageProcessor::new(config.clone().into()));

        Ok(Self {
            connection_pool,
            cache_manager,
            config,
            rate_limiter,
            metrics,
            http_client,
            image_processor,
        })
    }

    /// Fetch NFT metadata with ultra-fast performance
    pub async fn fetch_nft_metadata(&self, mint_address: &str) -> NftResult<NftInfo> {
        let start_time = Instant::now();
        self.metrics.total_fetches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Check cache first
        let cache_key = CacheKey::metadata(mint_address);
        if let Some(cached_nft) = self.cache_manager.get_nft(&cache_key).await {
            self.metrics.cache_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            debug!("Cache hit for NFT metadata: {}", mint_address);
            return Ok(cached_nft);
        }

        self.metrics.cache_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Acquire rate limiter
        let _permit = self.rate_limiter.acquire().await.map_err(|e| {
            NftError::ResourceExhausted {
                message: format!("Failed to acquire rate limiter: {}", e),
                resource_type: "rate_limiter".to_string(),
                current_usage: None,
                limit: Some(self.config.max_concurrent_requests as u64),
            }
        })?;

        // Fetch account info from blockchain
        let account_info = self.fetch_account_info(mint_address).await?;
        
        // Parse metadata
        let mut nft_info = self.parse_account_metadata(mint_address, &account_info).await?;
        
        // Fetch off-chain metadata if URI is available
        if let Some(metadata_uri) = &nft_info.metadata_uri {
            match self.fetch_offchain_metadata(metadata_uri).await {
                Ok(offchain_metadata) => {
                    self.merge_offchain_metadata(&mut nft_info, offchain_metadata).await?;
                }
                Err(e) => {
                    warn!("Failed to fetch off-chain metadata for {}: {}", mint_address, e);
                    // Don't fail the entire operation if off-chain metadata fails
                }
            }
        }

        // Validate metadata
        if self.config.enable_validation {
            let validation_result = self.validate_metadata(&nft_info).await?;
            nft_info.security_assessment = validation_result.security_assessment;
            
            if !validation_result.is_valid {
                self.metrics.validation_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        // Process image if enabled
        if self.config.enable_image_fetching {
            if let Some(image_uri) = &nft_info.image_uri {
                match self.process_image(image_uri).await {
                    Ok(image_result) => {
                        nft_info.image_verified = image_result.verified;
                        // Store additional image metadata if needed
                    }
                    Err(e) => {
                        warn!("Failed to process image for {}: {}", mint_address, e);
                        nft_info.image_verified = false;
                    }
                }
            }
        }

        // Update timestamps
        nft_info.updated_at = chrono::Utc::now();

        // Cache the result
        self.cache_manager.set_nft(&cache_key, &nft_info).await;

        // Update metrics
        self.metrics.successful_fetches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let fetch_time_ms = start_time.elapsed().as_millis() as u64;
        self.metrics.total_bytes_fetched.fetch_add(
            serde_json::to_string(&nft_info).unwrap_or_default().len() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );

        info!("Successfully fetched metadata for {} in {}ms", mint_address, fetch_time_ms);

        Ok(nft_info)
    }

    /// Batch fetch metadata for multiple NFTs with parallel processing
    pub async fn batch_fetch_metadata(&self, mint_addresses: &[String]) -> NftResult<Vec<NftInfo>> {
        let start_time = Instant::now();
        
        let results: Vec<NftResult<NftInfo>> = futures::stream::iter(mint_addresses)
            .map(|mint_address| async move {
                self.fetch_nft_metadata(mint_address).await
            })
            .buffer_unordered(self.config.max_concurrent_requests)
            .collect()
            .await;

        let mut successful_results = Vec::new();
        let mut failed_count = 0;

        for result in results {
            match result {
                Ok(nft_info) => successful_results.push(nft_info),
                Err(e) => {
                    error!("Failed to fetch metadata: {}", e);
                    failed_count += 1;
                }
            }
        }

        let total_time_ms = start_time.elapsed().as_millis() as u64;
        info!(
            "Batch metadata fetch completed: {} successful, {} failed in {}ms",
            successful_results.len(),
            failed_count,
            total_time_ms
        );

        Ok(successful_results)
    }

    /// Fetch account information from blockchain
    async fn fetch_account_info(&self, mint_address: &str) -> NftResult<solana_account_decoder::UiAccount> {
        let mint_pubkey = mint_address.parse::<solana_sdk::pubkey::Pubkey>()
            .map_err(|e| NftError::Validation {
                message: format!("Invalid mint address: {}", e),
                field: Some("mint_address".to_string()),
                value: Some(mint_address.to_string()),
            })?;

        let connection = self.connection_pool.get_client_internal().await
            .ok_or_else(|| NftError::Network {
                message: "No available RPC connections".to_string(),
                source_info: "connection_pool".to_string(),
            })?;

        let account = connection.get_account(&mint_pubkey).await
            .map_err(|e| NftError::Network {
                message: format!("Failed to fetch account: {}", e),
                source_info: "rpc".to_string(),
            })?;

        // Convert to UiAccount for compatibility
        let ui_account = solana_account_decoder::UiAccount::encode(
            &mint_pubkey,
            &account,
            solana_account_decoder::UiAccountEncoding::Base64,
            None,
            solana_account_decoder::UiDataSliceConfig::default(),
        ).map_err(|e| NftError::Serialization {
            message: format!("Failed to encode account: {}", e),
            format: Some("UiAccount".to_string()),
            data_type: Some("account".to_string()),
        })?;

        Ok(ui_account)
    }

    /// Parse on-chain metadata
    async fn parse_account_metadata(&self, mint_address: &str, account: &solana_account_decoder::UiAccount) -> NftResult<NftInfo> {
        let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &account.data[0])
            .map_err(|e| NftError::Serialization {
                message: format!("Failed to decode account data: {}", e),
                format: Some("base64".to_string()),
                data_type: Some("account_data".to_string()),
            })?;

        // Parse Token metadata
        let metadata = match spl_token_metadata::state::Metadata::deserialize(&mut &data[..]) {
            Ok(metadata) => metadata,
            Err(e) => {
                // If it's not a metadata account, create basic NFT info
                return Ok(NftInfo {
                    id: Uuid::new_v4(),
                    mint_address: mint_address.to_string(),
                    token_account: account.pubkey.clone(),
                    owner: account.owner.clone(),
                    name: None,
                    symbol: None,
                    description: None,
                    metadata_uri: None,
                    image_uri: None,
                    animation_uri: None,
                    external_url: None,
                    collection: None,
                    creators: vec![],
                    attributes: vec![],
                    token_standard: None,
                    master_edition: false,
                    edition_number: None,
                    max_supply: None,
                    estimated_value_lamports: None,
                    last_valuation: None,
                    security_assessment: SecurityAssessment::default(),
                    rarity_score: None,
                    quality_score: None,
                    metadata_verified: false,
                    image_verified: false,
                    discovered_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    additional_metadata: HashMap::new(),
                });
            }
        };

        let mut nft_info = NftInfo {
            id: Uuid::new_v4(),
            mint_address: mint_address.to_string(),
            token_account: account.pubkey.clone(),
            owner: account.owner.clone(),
            name: metadata.name.map(|s| s.replace('\0', "").trim().to_string()),
            symbol: metadata.symbol.map(|s| s.replace('\0', "").trim().to_string()),
            description: None, // Will be filled from off-chain metadata
            metadata_uri: metadata.uri.map(|s| s.replace('\0', "").trim().to_string()),
            image_uri: None, // Will be filled from off-chain metadata
            animation_uri: None,
            external_url: None,
            collection: None,
            creators: metadata.creators.iter().map(|c| CreatorInfo {
                address: c.address.to_string(),
                verified: c.verified,
                share: c.share as u8,
                name: None,
                twitter: None,
                website: None,
                security_assessment: SecurityAssessment::default(),
            }).collect(),
            attributes: vec![],
            token_standard: Some("non-fungible".to_string()),
            master_edition: false, // Will be determined from master edition account
            edition_number: None,
            max_supply: None,
            estimated_value_lamports: None,
            last_valuation: None,
            security_assessment: SecurityAssessment::default(),
            rarity_score: None,
            quality_score: None,
            metadata_verified: false,
            image_verified: false,
            discovered_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            additional_metadata: HashMap::new(),
        };

        // Parse collection information if present
        if let Some(collection) = &metadata.collection {
            nft_info.collection = Some(CollectionInfo {
                name: collection.name.replace('\0', "").trim().to_string(),
                symbol: None,
                description: None,
                image: None,
                uri: None,
                verified: collection.verified,
                collection_mint_address: None,
                security_assessment: SecurityAssessment::default(),
                floor_price_lamports: None,
                total_supply: None,
                item_count: None,
            });
        }

        Ok(nft_info)
    }

    /// Fetch off-chain metadata from URI
    async fn fetch_offchain_metadata(&self, uri: &str) -> NftResult<Value> {
        // Check if URI is blocked
        if self.is_uri_blocked(uri) {
            return Err(NftError::Security {
                message: format!("Metadata URI is blocked: {}", uri),
                risk_level: RiskLevel::High,
                details: Some("URI is in blocked list".to_string()),
            });
        }

        let strategy = self.get_fetch_strategy(uri)?;
        strategy.fetch_metadata(uri).await
    }

    /// Merge off-chain metadata into NFT info
    async fn merge_offchain_metadata(&self, nft_info: &mut NftInfo, metadata: Value) -> NftResult<()> {
        // Extract basic fields
        if let Some(name) = metadata.get("name").and_then(|v| v.as_str()) {
            nft_info.name = Some(name.to_string());
        }

        if let Some(description) = metadata.get("description").and_then(|v| v.as_str()) {
            nft_info.description = Some(description.to_string());
        }

        if let Some(image) = metadata.get("image").and_then(|v| v.as_str()) {
            nft_info.image_uri = Some(image.to_string());
        }

        if let Some(animation_url) = metadata.get("animation_url").and_then(|v| v.as_str()) {
            nft_info.animation_uri = Some(animation_url.to_string());
        }

        if let Some(external_url) = metadata.get("external_url").and_then(|v| v.as_str()) {
            nft_info.external_url = Some(external_url.to_string());
        }

        // Extract attributes
        if let Some(attributes) = metadata.get("attributes").and_then(|v| v.as_array()) {
            for attr in attributes {
                if let (Some(trait_type), Some(value)) = (
                    attr.get("trait_type").and_then(|v| v.as_str()),
                    attr.get("value")
                ) {
                    nft_info.attributes.push(NftAttribute {
                        trait_type: trait_type.to_string(),
                        value: value.clone(),
                        rarity: None,
                        rare: false,
                        display_type: attr.get("display_type").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    });
                }
            }
        }

        // Extract properties
        if let Some(properties) = metadata.get("properties") {
            if let Some(files) = properties.get("files").and_then(|v| v.as_array()) {
                for file in files {
                    if let Some(uri) = file.get("uri").and_then(|v| v.as_str()) {
                        if let Some(file_type) = file.get("type").and_then(|v| v.as_str()) {
                            if file_type.starts_with("image/") && nft_info.image_uri.is_none() {
                                nft_info.image_uri = Some(uri.to_string());
                            } else if file_type.starts_with("video/") || file_type.starts_with("audio/") {
                                nft_info.animation_uri = Some(uri.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Store additional metadata
        for (key, value) in metadata.as_object().unwrap_or(&serde_json::Map::new()) {
            if !matches!(key, "name" | "description" | "image" | "animation_url" | "external_url" | "attributes" | "properties") {
                nft_info.additional_metadata.insert(key.clone(), value.clone());
            }
        }

        Ok(())
    }

    /// Validate metadata
    async fn validate_metadata(&self, nft_info: &NftInfo) -> NftResult<MetadataValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut security_issues = Vec::new();
        let mut validation_score = 100u8;
        let mut completeness_score = 0u8;

        // Check required fields
        let mut required_fields = 0;
        let mut present_fields = 0;

        // Name validation
        required_fields += 1;
        if nft_info.name.is_some() {
            present_fields += 1;
            if let Some(name) = &nft_info.name {
                if name.len() > 100 {
                    errors.push("Name too long (max 100 characters)".to_string());
                    validation_score = validation_score.saturating_sub(10);
                } else if name.is_empty() {
                    errors.push("Name cannot be empty".to_string());
                    validation_score = validation_score.saturating_sub(5);
                }
            }
        } else {
            warnings.push("Missing name field".to_string());
            validation_score = validation_score.saturating_sub(5);
        }

        // Symbol validation
        required_fields += 1;
        if nft_info.symbol.is_some() {
            present_fields += 1;
            if let Some(symbol) = &nft_info.symbol {
                if symbol.len() > 10 {
                    errors.push("Symbol too long (max 10 characters)".to_string());
                    validation_score = validation_score.saturating_sub(5);
                }
            }
        }

        // Description validation
        required_fields += 1;
        if nft_info.description.is_some() {
            present_fields += 1;
            if let Some(description) = &nft_info.description {
                if description.len() > 1000 {
                    warnings.push("Description very long (consider shortening)".to_string());
                    validation_score = validation_score.saturating_sub(2);
                }
            }
        }

        // Image URI validation
        required_fields += 1;
        if nft_info.image_uri.is_some() {
            present_fields += 1;
            if let Some(image_uri) = &nft_info.image_uri {
                if self.is_uri_blocked(image_uri) {
                    security_issues.push(SecurityIssue {
                        issue_type: SecurityIssueType::SuspiciousMetadata,
                        severity: RiskLevel::High,
                        description: "Image URI is in blocked list".to_string(),
                        recommendation: "Avoid this NFT or verify the source".to_string(),
                        confirmed: true,
                        context: Some(format!("Blocked URI: {}", image_uri)),
                    });
                    validation_score = validation_score.saturating_sub(20);
                }
            }
        }

        // Metadata URI validation
        if let Some(metadata_uri) = &nft_info.metadata_uri {
            if self.is_uri_blocked(metadata_uri) {
                security_issues.push(SecurityIssue {
                    issue_type: SecurityIssueType::SuspiciousMetadata,
                    severity: RiskLevel::High,
                    description: "Metadata URI is in blocked list".to_string(),
                    recommendation: "Avoid this NFT or verify the source".to_string(),
                    confirmed: true,
                    context: Some(format!("Blocked URI: {}", metadata_uri)),
                });
                validation_score = validation_score.saturating_sub(20);
            }
        }

        // Creator validation
        if nft_info.creators.is_empty() {
            warnings.push("No creators specified".to_string());
            validation_score = validation_score.saturating_sub(5);
        } else {
            let verified_creators = nft_info.creators.iter().filter(|c| c.verified).count();
            if verified_creators == 0 {
                warnings.push("No verified creators".to_string());
                validation_score = validation_score.saturating_sub(10);
            }
        }

        // Calculate completeness score
        completeness_score = if required_fields > 0 {
            ((present_fields as f64 / required_fields as f64) * 100.0) as u8
        } else {
            0
        };

        // Security assessment
        let security_assessment = SecurityAssessment {
            risk_level: if security_issues.is_empty() {
                RiskLevel::None
            } else {
                security_issues.iter().map(|i| i.severity).max().unwrap_or(RiskLevel::Low)
            },
            security_score: validation_score,
            issues: security_issues,
            verified: nft_info.collection.as_ref().map(|c| c.verified).unwrap_or(false),
            assessed_at: chrono::Utc::now(),
            confidence: 90,
        };

        Ok(MetadataValidationResult {
            is_valid: errors.is_empty() && validation_score >= 50,
            errors,
            security_issues: security_assessment.issues.clone(),
            warnings,
            validation_score,
            completeness_score,
        })
    }

    /// Process image
    async fn process_image(&self, image_uri: &str) -> NftResult<ImageProcessResult> {
        self.image_processor.process_image(image_uri).await
    }

    /// Check if URI is blocked
    fn is_uri_blocked(&self, uri: &str) -> bool {
        // Extract domain from URI
        if let Ok(url) = url::Url::parse(uri) {
            let domain = url.host_str().unwrap_or("");
            
            // Check blocked domains
            if self.config.blocked_domains.iter().any(|blocked| domain.contains(blocked)) {
                return true;
            }
            
            // Check for suspicious patterns
            if uri.contains("bit.ly") || uri.contains("tinyurl.com") || uri.contains("t.co") {
                return true; // URL shorteners are often used for phishing
            }
        }
        
        false
    }

    /// Get appropriate fetch strategy for URI
    fn get_fetch_strategy(&self, uri: &str) -> NftResult<Box<dyn MetadataFetchStrategy>> {
        if uri.starts_with("http://") || uri.starts_with("https://") {
            if uri.contains("arweave.net") {
                Ok(Box::new(ArweaveMetadataFetcher::new(self.config.clone())?))
            } else if uri.contains("ipfs.io") || uri.contains("ipfs.") || uri.contains("/ipfs/") {
                Ok(Box::new(IpfsMetadataFetcher::new(self.config.clone())?))
            } else {
                Ok(Box::new(HttpMetadataFetcher::new(self.config.clone())?))
            }
        } else if uri.starts_with("ipfs://") {
            Ok(Box::new(IpfsMetadataFetcher::new(self.config.clone())?))
        } else {
            Err(NftError::Validation {
                message: format!("Unsupported URI scheme: {}", uri),
                field: Some("metadata_uri".to_string()),
                value: Some(uri.to_string()),
            })
        }
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> &MetadataMetrics {
        &self.metrics
    }
}

/// Image processing result
#[derive(Debug, Clone)]
pub struct ImageProcessResult {
    /// Is image verified
    pub verified: bool,
    
    /// Image format
    pub format: Option<String>,
    
    /// Image dimensions
    pub dimensions: Option<(u32, u32)>,
    
    /// File size in bytes
    pub size_bytes: Option<usize>,
    
    /// EXIF data
    pub exif_data: Option<HashMap<String, String>>,
    
    /// Security issues found
    pub security_issues: Vec<SecurityIssue>,
}

impl ImageProcessor {
    /// Create new image processor
    pub fn new(config: ImageProcessorConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .expect("Failed to create HTTP client for image processor");

        Self { config, http_client }
    }

    /// Process image from URI
    pub async fn process_image(&self, image_uri: &str) -> NftResult<ImageProcessResult> {
        // Fetch image data
        let response = self.http_client.get(image_uri).send().await
            .map_err(|e| NftError::Network {
                message: format!("Failed to fetch image: {}", e),
                source_info: Some("http".to_string()),
            })?;

        if !response.status().is_success() {
            return Err(NftError::Network {
                message: format!("HTTP error fetching image: {}", response.status()),
                source_info: Some("http".to_string()),
            });
        }

        let content_type = response.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream");

        // Check if format is allowed
        if !self.config.allowed_formats.contains(&content_type.to_string()) {
            return Err(NftError::Validation {
                message: format!("Image format not allowed: {}", content_type),
                field: Some("image_format".to_string()),
                value: Some(content_type.to_string()),
            });
        }

        let image_data = response.bytes().await
            .map_err(|e| NftError::Network {
                message: format!("Failed to read image data: {}", e),
                source_info: Some("http".to_string()),
            })?;

        // Check size limit
        if image_data.len() > self.config.max_size_bytes {
            return Err(NftError::Validation {
                message: format!("Image too large: {} bytes (max: {})", 
                    image_data.len(), self.config.max_size_bytes),
                field: Some("image_size".to_string()),
                value: Some(image_data.len().to_string()),
            });
        }

        let mut security_issues = Vec::new();
        let mut verified = true;

        // Basic image validation using image crate
        let dimensions = match image::load_from_memory(&image_data) {
            Ok(img) => Some((img.width(), img.height())),
            Err(e) => {
                security_issues.push(SecurityIssue {
                    issue_type: SecurityIssueType::MaliciousImage,
                    severity: RiskLevel::High,
                    description: format!("Invalid image format: {}", e),
                    recommendation: "Image may be corrupted or malicious".to_string(),
                    confirmed: true,
                    context: None,
                });
                verified = false;
                None
            }
        };

        // Extract EXIF data if enabled
        let exif_data = if self.config.enable_exif_extraction {
            self.extract_exif_data(&image_data).unwrap_or(None)
        } else {
            None
        };

        Ok(ImageProcessResult {
            verified,
            format: Some(content_type.to_string()),
            dimensions,
            size_bytes: Some(image_data.len()),
            exif_data,
            security_issues,
        })
    }

    /// Extract EXIF data from image
    fn extract_exif_data(&self, image_data: &[u8]) -> NftResult<Option<HashMap<String, String>>> {
        // This is a placeholder for EXIF extraction
        // In a real implementation, you would use a library like `kamadak-exif`
        // to extract EXIF data from JPEG images
        Ok(None)
    }
}

// Implement metadata fetch strategies

impl HttpMetadataFetcher {
    pub fn new(config: MetadataConfig) -> NftResult<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.request_timeout_ms))
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| NftError::Configuration {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self { http_client, config })
    }
}

#[async_trait]
impl MetadataFetchStrategy for HttpMetadataFetcher {
    async fn fetch_metadata(&self, uri: &str) -> NftResult<Value> {
        let response = self.http_client.get(uri).send().await
            .map_err(|e| NftError::Network {
                message: format!("Failed to fetch metadata: {}", e),
                source_info: Some("http".to_string()),
            })?;

        if !response.status().is_success() {
            return Err(NftError::Network {
                message: format!("HTTP error: {}", response.status()),
                source_info: Some("http".to_string()),
            });
        }

        let metadata: Value = response.json().await
            .map_err(|e| NftError::Serialization {
                message: format!("Failed to parse JSON metadata: {}", e),
                format: Some("json".to_string()),
                data_type: Some("metadata".to_string()),
            })?;

        Ok(metadata)
    }

    fn name(&self) -> &'static str {
        "http"
    }

    fn can_handle(&self, uri: &str) -> bool {
        uri.starts_with("http://") || uri.starts_with("https://")
    }
}

impl IpfsMetadataFetcher {
    pub fn new(config: MetadataConfig) -> NftResult<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.request_timeout_ms))
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| NftError::Configuration {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        let ipfs_gateways = vec![
            "https://ipfs.io/ipfs/".to_string(),
            "https://cloudflare-ipfs.com/ipfs/".to_string(),
            "https://dweb.link/ipfs/".to_string(),
        ];

        Ok(Self { http_client, config, ipfs_gateways })
    }
}

#[async_trait]
impl MetadataFetchStrategy for IpfsMetadataFetcher {
    async fn fetch_metadata(&self, uri: &str) -> NftResult<Value> {
        // Extract IPFS hash from URI
        let ipfs_hash = if uri.starts_with("ipfs://") {
            uri.strip_prefix("ipfs://").unwrap_or("")
        } else if uri.contains("/ipfs/") {
            uri.split("/ipfs/").nth(1).unwrap_or("")
        } else {
            return Err(NftError::Validation {
                message: format!("Invalid IPFS URI: {}", uri),
                field: Some("metadata_uri".to_string()),
                value: Some(uri.to_string()),
            });
        };

        // Try different gateways
        for gateway in &self.ipfs_gateways {
            let full_uri = format!("{}{}", gateway, ipfs_hash);
            
            match self.http_client.get(&full_uri).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        let metadata: Value = response.json().await
                            .map_err(|e| NftError::Serialization {
                                message: format!("Failed to parse JSON metadata: {}", e),
                                format: Some("json".to_string()),
                                data_type: Some("metadata".to_string()),
                            })?;
                        return Ok(metadata);
                    }
                }
                Err(_) => continue; // Try next gateway
            }
        }

        Err(NftError::Network {
            message: "All IPFS gateways failed".to_string(),
            source_info: "ipfs".to_string(),
        })
    }

    fn name(&self) -> &'static str {
        "ipfs"
    }

    fn can_handle(&self, uri: &str) -> bool {
        uri.starts_with("ipfs://") || uri.contains("/ipfs/") || uri.contains("ipfs.io")
    }
}

impl ArweaveMetadataFetcher {
    pub fn new(config: MetadataConfig) -> NftResult<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.request_timeout_ms))
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| NftError::Configuration {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self { http_client, config })
    }
}

#[async_trait]
impl MetadataFetchStrategy for ArweaveMetadataFetcher {
    async fn fetch_metadata(&self, uri: &str) -> NftResult<Value> {
        let response = self.http_client.get(uri).send().await
            .map_err(|e| NftError::Network {
                message: format!("Failed to fetch Arweave metadata: {}", e),
                source_info: Some("arweave".to_string()),
            })?;

        if !response.status().is_success() {
            return Err(NftError::Network {
                message: format!("Arweave HTTP error: {}", response.status()),
                source_info: Some("arweave".to_string()),
            });
        }

        let metadata: Value = response.json().await
            .map_err(|e| NftError::Serialization {
                message: format!("Failed to parse Arweave JSON metadata: {}", e),
                format: Some("json".to_string()),
                data_type: Some("metadata".to_string()),
            })?;

        Ok(metadata)
    }

    fn name(&self) -> &'static str {
        "arweave"
    }

    fn can_handle(&self, uri: &str) -> bool {
        uri.contains("arweave.net")
    }
}

impl From<MetadataConfig> for ImageProcessorConfig {
    fn from(metadata_config: MetadataConfig) -> Self {
        Self {
            max_size_bytes: metadata_config.max_image_size_bytes,
            allowed_formats: metadata_config.allowed_image_formats,
            enable_exif_extraction: true,
            enable_analysis: false,
            timeout_ms: metadata_config.request_timeout_ms,
        }
    }
}

impl Default for SecurityAssessment {
    fn default() -> Self {
        Self {
            risk_level: RiskLevel::None,
            security_score: 100,
            issues: vec![],
            verified: false,
            assessed_at: chrono::Utc::now(),
            confidence: 0,
        }
    }
}
