//! # NFT Core Types
//!
//! Comprehensive type definitions for NFT operations with maximum
//! performance, security, and customization in mind.

use crate::nft::errors::{NftError, NftResult, RiskLevel};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

/// Comprehensive NFT information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftInfo {
    /// Unique identifier for this NFT record
    pub id: Uuid,
    
    /// Token mint address
    pub mint_address: String,
    
    /// Token account address
    pub token_account: String,
    
    /// Owner wallet address
    pub owner: String,
    
    /// NFT name from metadata
    pub name: Option<String>,
    
    /// NFT symbol from metadata
    pub symbol: Option<String>,
    
    /// NFT description
    pub description: Option<String>,
    
    /// Metadata URI
    pub metadata_uri: Option<String>,
    
    /// Image URI
    pub image_uri: Option<String>,
    
    /// Animation URI (for animated NFTs)
    pub animation_uri: Option<String>,
    
    /// External URL (project website, etc.)
    pub external_url: Option<String>,
    
    /// Collection information
    pub collection: Option<CollectionInfo>,
    
    /// Creator information
    pub creators: Vec<CreatorInfo>,
    
    /// Attributes/properties
    pub attributes: Vec<NftAttribute>,
    
    /// Token standard (e.g., "non-fungible", "semi-fungible")
    pub token_standard: Option<String>,
    
    /// Whether this is a master edition
    pub master_edition: bool,
    
    /// Edition number if this is a print
    pub edition_number: Option<u64>,
    
    /// Maximum supply for editions
    pub max_supply: Option<u64>,
    
    /// Current estimated value in lamports
    pub estimated_value_lamports: Option<u64>,
    
    /// Last valuation timestamp
    pub last_valuation: Option<chrono::DateTime<chrono::Utc>>,
    
    /// Security assessment
    pub security_assessment: SecurityAssessment,
    
    /// Rarity score (0-100, higher is rarer)
    pub rarity_score: Option<f64>,
    
    /// Quality score (0-100, higher is better quality)
    pub quality_score: Option<f64>,
    
    /// Metadata verification status
    pub metadata_verified: bool,
    
    /// Image verification status
    pub image_verified: bool,
    
    /// When this NFT was first discovered
    pub discovered_at: chrono::DateTime<chrono::Utc>,
    
    /// When this record was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
    
    /// Additional metadata not covered by other fields
    pub additional_metadata: HashMap<String, serde_json::Value>,
}

/// Collection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    /// Collection name
    pub name: String,
    
    /// Collection symbol
    pub symbol: Option<String>,
    
    /// Collection description
    pub description: Option<String>,
    
    /// Collection image
    pub image: Option<String>,
    
    /// Collection URI
    pub uri: Option<String>,
    
    /// Whether this collection is verified
    pub verified: bool,
    
    /// Collection mint address if it's a verified collection
    pub collection_mint_address: Option<String>,
    
    /// Collection security assessment
    pub security_assessment: SecurityAssessment,
    
    /// Floor price in lamports
    pub floor_price_lamports: Option<u64>,
    
    /// Total supply in collection
    pub total_supply: Option<u64>,
    
    /// Number of items in collection
    pub item_count: Option<u64>,
}

/// Creator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorInfo {
    /// Creator address
    pub address: String,
    
    /// Creator verified status
    pub verified: bool,
    
    /// Creator share percentage (basis points)
    pub share: u8,
    
    /// Creator name if known
    pub name: Option<String>,
    
    /// Creator Twitter handle if known
    pub twitter: Option<String>,
    
    /// Creator website if known
    pub website: Option<String>,
    
    /// Security assessment for this creator
    pub security_assessment: SecurityAssessment,
}

/// NFT attribute/property
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftAttribute {
    /// Attribute trait type
    pub trait_type: String,
    
    /// Attribute value
    pub value: serde_json::Value,
    
    /// Rarity of this attribute (optional)
    pub rarity: Option<f64>,
    
    /// Whether this is a rare attribute
    pub rare: bool,
    
    /// Attribute display type
    pub display_type: Option<String>,
}

/// Security assessment for NFTs, collections, and creators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAssessment {
    /// Overall risk level
    pub risk_level: RiskLevel,
    
    /// Security score (0-100, higher is more secure)
    pub security_score: u8,
    
    /// List of security issues found
    pub issues: Vec<SecurityIssue>,
    
    /// Whether this entity is verified
    pub verified: bool,
    
    /// When assessment was performed
    pub assessed_at: chrono::DateTime<chrono::Utc>,
    
    /// Assessment confidence (0-100)
    pub confidence: u8,
}

/// Individual security issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityIssue {
    /// Issue type
    pub issue_type: SecurityIssueType,
    
    /// Issue severity
    pub severity: RiskLevel,
    
    /// Issue description
    pub description: String,
    
    /// Recommended action
    pub recommendation: String,
    
    /// Whether issue is confirmed or suspected
    pub confirmed: bool,
    
    /// Additional context
    pub context: Option<String>,
}

/// Types of security issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityIssueType {
    /// Suspicious metadata URI
    SuspiciousMetadata,
    
    /// Malicious image detected
    MaliciousImage,
    
    /// Phishing attempt
    Phishing,
    
    /// Rug pull risk
    RugPullRisk,
    
    /// Copymint (duplicate NFT)
    Copymint,
    
    /// Spam NFT
    Spam,
    
    /// Unauthorized creator
    UnauthorizedCreator,
    
    /// Broken metadata
    BrokenMetadata,
    
    /// Expired domain
    ExpiredDomain,
    
    /// Suspicious contract
    SuspiciousContract,
    
    /// Honeypot
    Honeypot,
    
    /// Other security issue
    Other { issue_type: String },
}

/// NFT portfolio analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftPortfolio {
    /// Portfolio ID
    pub id: Uuid,
    
    /// Wallet address
    pub wallet_address: String,
    
    /// All NFTs in portfolio
    pub nfts: Vec<NftInfo>,
    
    /// Total estimated value in lamports
    pub total_value_lamports: u64,
    
    /// Total number of NFTs
    pub total_count: u32,
    
    /// Number of verified NFTs
    pub verified_count: u32,
    
    /// Number of high-risk NFTs
    pub high_risk_count: u32,
    
    /// Collection breakdown
    pub collection_breakdown: HashMap<String, CollectionBreakdown>,
    
    /// Value distribution
    pub value_distribution: ValueDistribution,
    
    /// Risk distribution
    pub risk_distribution: RiskDistribution,
    
    /// Quality metrics
    pub quality_metrics: PortfolioQualityMetrics,
    
    /// When portfolio was analyzed
    pub analyzed_at: chrono::DateTime<chrono::Utc>,
    
    /// Analysis duration in milliseconds
    pub analysis_duration_ms: u64,
    
    /// Analysis configuration used
    pub analysis_config: String,
}

/// Collection breakdown in portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionBreakdown {
    /// Collection name
    pub collection_name: String,
    
    /// Number of NFTs from this collection
    pub count: u32,
    
    /// Total value of NFTs from this collection
    pub total_value_lamports: u64,
    
    /// Average value per NFT in this collection
    pub average_value_lamports: u64,
    
    /// Percentage of portfolio value
    pub portfolio_percentage: f64,
    
    /// Collection verification status
    pub verified: bool,
}

/// Value distribution analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueDistribution {
    /// Highest value NFT
    pub highest_value: Option<u64>,
    
    /// Lowest value NFT
    pub lowest_value: Option<u64>,
    
    /// Median value
    pub median_value: Option<u64>,
    
    /// Average value
    pub average_value: f64,
    
    /// Value percentiles (25th, 50th, 75th, 90th, 95th, 99th)
    pub percentiles: HashMap<u8, u64>,
    
    /// Value concentration (Gini coefficient)
    pub concentration: f64,
}

/// Risk distribution analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskDistribution {
    /// Count by risk level
    pub counts: HashMap<RiskLevel, u32>,
    
    /// Value by risk level
    pub value_by_risk: HashMap<RiskLevel, u64>,
    
    /// Percentage by risk level
    pub percentages: HashMap<RiskLevel, f64>,
    
    /// Overall portfolio risk score
    pub overall_risk_score: f64,
}

/// Portfolio quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioQualityMetrics {
    /// Average rarity score
    pub average_rarity_score: Option<f64>,
    
    /// Average quality score
    pub average_quality_score: Option<f64>,
    
    /// Verification rate (0-100)
    pub verification_rate: f64,
    
    /// Metadata completeness (0-100)
    pub metadata_completeness: f64,
    
    /// Image availability (0-100)
    pub image_availability: f64,
    
    /// Unique collections count
    pub unique_collections: u32,
    
    /// Diversity score (0-100, higher is more diverse)
    pub diversity_score: f64,
}

/// NFT scan configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftScanConfig {
    /// Performance mode
    pub performance_mode: PerformanceMode,
    
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
    
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    
    /// Whether to fetch metadata
    pub fetch_metadata: bool,
    
    /// Whether to fetch images
    pub fetch_images: bool,
    
    /// Whether to perform security validation
    pub perform_security_validation: bool,
    
    /// Whether to calculate valuation
    pub calculate_valuation: bool,
    
    /// Whether to analyze rarity
    pub analyze_rarity: bool,
    
    /// Maximum NFTs to process per wallet
    pub max_nfts_per_wallet: Option<u32>,
    
    /// Cache configuration
    pub cache_config: CacheConfig,
    
    /// Security configuration
    pub security_config: SecurityConfig,
    
    /// Valuation configuration
    pub valuation_config: ValuationConfig,
}

/// Performance modes for NFT operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceMode {
    /// Ultra-fast mode - maximum speed, minimal validation
    UltraFast,
    /// Fast mode - good speed with basic validation
    Fast,
    /// Balanced mode - balanced speed and accuracy
    Balanced,
    /// Thorough mode - maximum accuracy, slower speed
    Thorough,
    /// Custom mode with specific settings
    Custom { settings: HashMap<String, serde_json::Value> },
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable metadata caching
    pub enable_metadata_cache: bool,
    
    /// Enable image caching
    pub enable_image_cache: bool,
    
    /// Enable valuation cache
    pub enable_valuation_cache: bool,
    
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    
    /// Maximum cache size in MB
    pub max_cache_size_mb: u64,
    
    /// Cache cleanup interval in seconds
    pub cleanup_interval_seconds: u64,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable security validation
    pub enable_validation: bool,
    
    /// Block high-risk NFTs
    pub block_high_risk: bool,
    
    /// Block unverified collections
    pub block_unverified_collections: bool,
    
    /// Strict validation mode
    pub strict_mode: bool,
    
    /// Custom security rules
    pub custom_rules: Vec<SecurityRule>,
}

/// Custom security rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRule {
    /// Rule name
    pub name: String,
    
    /// Rule description
    pub description: String,
    
    /// Rule condition (JSON expression)
    pub condition: serde_json::Value,
    
    /// Action to take when rule matches
    pub action: SecurityAction,
    
    /// Rule priority
    pub priority: u8,
}

/// Security actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityAction {
    /// Allow the NFT
    Allow,
    /// Block the NFT
    Block,
    /// Flag the NFT with warning
    Flag,
    /// Require additional validation
    RequireValidation,
}

/// Valuation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValuationConfig {
    /// Enable valuation
    pub enable_valuation: bool,
    
    /// Valuation methods to use
    pub methods: Vec<ValuationMethod>,
    
    /// Floor price weight in valuation
    pub floor_price_weight: f64,
    
    /// Recent sales weight in valuation
    pub recent_sales_weight: f64,
    
    /// Rarity weight in valuation
    pub rarity_weight: f64,
    
    /// Maximum age of sales data in days
    pub max_sales_age_days: u32,
    
    /// Minimum sales for reliable valuation
    pub min_sales_count: u32,
}

/// Valuation methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ValuationMethod {
    /// Floor price based valuation
    FloorPrice,
    /// Recent sales average
    RecentSales,
    /// Rarity-based valuation
    RarityBased,
    /// Machine learning model
    MlModel,
    /// Custom valuation method
    Custom { method_name: String, config: serde_json::Value },
}

impl Default for NftScanConfig {
    fn default() -> Self {
        Self {
            performance_mode: PerformanceMode::Balanced,
            max_concurrent_requests: 10,
            request_timeout_ms: 30000,
            fetch_metadata: true,
            fetch_images: false,
            perform_security_validation: true,
            calculate_valuation: true,
            analyze_rarity: true,
            max_nfts_per_wallet: None,
            cache_config: CacheConfig::default(),
            security_config: SecurityConfig::default(),
            valuation_config: ValuationConfig::default(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enable_metadata_cache: true,
            enable_image_cache: false,
            enable_valuation_cache: true,
            cache_ttl_seconds: 300, // 5 minutes
            max_cache_size_mb: 100,
            cleanup_interval_seconds: 60,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_validation: true,
            block_high_risk: false,
            block_unverified_collections: false,
            strict_mode: false,
            custom_rules: vec![],
        }
    }
}

impl Default for ValuationConfig {
    fn default() -> Self {
        Self {
            enable_valuation: true,
            methods: vec![
                ValuationMethod::FloorPrice,
                ValuationMethod::RecentSales,
            ],
            floor_price_weight: 0.4,
            recent_sales_weight: 0.4,
            rarity_weight: 0.2,
            max_sales_age_days: 30,
            min_sales_count: 3,
        }
    }
}

impl fmt::Display for PerformanceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PerformanceMode::UltraFast => write!(f, "UltraFast"),
            PerformanceMode::Fast => write!(f, "Fast"),
            PerformanceMode::Balanced => write!(f, "Balanced"),
            PerformanceMode::Thorough => write!(f, "Thorough"),
            PerformanceMode::Custom { settings } => write!(f, "Custom({:?})", settings),
        }
    }
}

impl fmt::Display for SecurityIssueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecurityIssueType::SuspiciousMetadata => write!(f, "SuspiciousMetadata"),
            SecurityIssueType::MaliciousImage => write!(f, "MaliciousImage"),
            SecurityIssueType::Phishing => write!(f, "Phishing"),
            SecurityIssueType::RugPullRisk => write!(f, "RugPullRisk"),
            SecurityIssueType::Copymint => write!(f, "Copymint"),
            SecurityIssueType::Spam => write!(f, "Spam"),
            SecurityIssueType::UnauthorizedCreator => write!(f, "UnauthorizedCreator"),
            SecurityIssueType::BrokenMetadata => write!(f, "BrokenMetadata"),
            SecurityIssueType::ExpiredDomain => write!(f, "ExpiredDomain"),
            SecurityIssueType::SuspiciousContract => write!(f, "SuspiciousContract"),
            SecurityIssueType::Honeypot => write!(f, "Honeypot"),
            SecurityIssueType::Other { issue_type } => write!(f, "Other({})", issue_type),
        }
    }
}

impl fmt::Display for SecurityAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecurityAction::Allow => write!(f, "Allow"),
            SecurityAction::Block => write!(f, "Block"),
            SecurityAction::Flag => write!(f, "Flag"),
            SecurityAction::RequireValidation => write!(f, "RequireValidation"),
        }
    }
}

impl fmt::Display for ValuationMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValuationMethod::FloorPrice => write!(f, "FloorPrice"),
            ValuationMethod::RecentSales => write!(f, "RecentSales"),
            ValuationMethod::RarityBased => write!(f, "RarityBased"),
            ValuationMethod::MlModel => write!(f, "MlModel"),
            ValuationMethod::Custom { method_name, .. } => write!(f, "Custom({})", method_name),
        }
    }
}
