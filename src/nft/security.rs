//! # NFT Security Validation System
//!
//! Comprehensive security validation with advanced threat detection,
//! risk assessment, and protective measures for NFT operations.

use crate::nft::cache::{CacheManager, CacheKey};
use crate::nft::errors::{NftError, NftResult, RiskLevel};
use crate::nft::types::*;
use crate::rpc::ConnectionPool;
use async_trait::async_trait;
use dashmap::DashMap;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

/// Comprehensive NFT security validator
#[derive(Clone)]
pub struct SecurityValidator {
    /// RPC connection pool
    connection_pool: Arc<ConnectionPool>,
    
    /// Cache manager
    cache_manager: Arc<CacheManager>,
    
    /// Configuration
    config: SecurityValidatorConfig,
    
    /// Security rules engine
    rules_engine: Arc<RulesEngine>,
    
    /// Threat intelligence provider
    threat_intel: Arc<dyn ThreatIntelligenceProvider>,
    
    /// Rate limiter
    rate_limiter: Arc<Semaphore>,
    
    /// Performance metrics
    metrics: Arc<SecurityMetrics>,
    
    /// Blacklist manager
    blacklist_manager: Arc<BlacklistManager>,
}

/// Security validator configuration
#[derive(Debug, Clone)]
pub struct SecurityValidatorConfig {
    /// Enable comprehensive validation
    pub enable_comprehensive_validation: bool,
    
    /// Enable metadata validation
    pub enable_metadata_validation: bool,
    
    /// Enable image validation
    pub enable_image_validation: bool,
    
    /// Enable creator validation
    pub enable_creator_validation: bool,
    
    /// Enable collection validation
    pub enable_collection_validation: bool,
    
    /// Enable transaction validation
    pub enable_transaction_validation: bool,
    
    /// Maximum concurrent validations
    pub max_concurrent_validations: usize,
    
    /// Validation timeout in milliseconds
    pub validation_timeout_ms: u64,
    
    /// Strict validation mode
    pub strict_mode: bool,
    
    /// Block high-risk NFTs
    pub block_high_risk: bool,
    
    /// Block unverified collections
    pub block_unverified_collections: bool,
    
    /// Minimum confidence threshold
    pub min_confidence_threshold: f64,
    
    /// Enable threat intelligence
    pub enable_threat_intel: bool,
    
    /// Cache TTL for security results in seconds
    pub cache_ttl_seconds: u64,
}

impl Default for SecurityValidatorConfig {
    fn default() -> Self {
        Self {
            enable_comprehensive_validation: true,
            enable_metadata_validation: true,
            enable_image_validation: true,
            enable_creator_validation: true,
            enable_collection_validation: true,
            enable_transaction_validation: false,
            max_concurrent_validations: 20,
            validation_timeout_ms: 15000,
            strict_mode: false,
            block_high_risk: false,
            block_unverified_collections: false,
            min_confidence_threshold: 0.7,
            enable_threat_intel: true,
            cache_ttl_seconds: 600, // 10 minutes
        }
    }
}

/// Security validation metrics
#[derive(Debug, Default)]
pub struct SecurityMetrics {
    /// Total validations performed
    pub total_validations: Arc<std::sync::atomic::AtomicU64>,
    
    /// Successful validations
    pub successful_validations: Arc<std::sync::atomic::AtomicU64>,
    
    /// Failed validations
    pub failed_validations: Arc<std::sync::atomic::AtomicU64>,
    
    /// Security issues found
    pub security_issues_found: Arc<std::sync::atomic::AtomicU64>,
    
    /// High-risk NFTs blocked
    pub high_risk_blocked: Arc<std::sync::atomic::AtomicU64>,
    
    /// Cache hits
    pub cache_hits: Arc<std::sync::atomic::AtomicU64>,
    
    /// Cache misses
    pub cache_misses: Arc<std::sync::atomic::AtomicU64>,
    
    /// Average validation time in milliseconds
    pub avg_validation_time_ms: Arc<std::sync::atomic::AtomicU64>,
    
    /// Threat intelligence lookups
    pub threat_intel_lookups: Arc<std::sync::atomic::AtomicU64>,
    
    /// Blacklist hits
    pub blacklist_hits: Arc<std::sync::atomic::AtomicU64>,
    
    /// Validations by risk level
    pub validations_by_risk: Arc<DashMap<RiskLevel, u64>>,
}

/// Security validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityValidationResult {
    /// Overall security assessment
    pub assessment: SecurityAssessment,
    
    /// Individual validation results
    pub validation_results: Vec<ValidationCheckResult>,
    
    /// Recommended actions
    pub recommendations: Vec<SecurityRecommendation>,
    
    /// Validation timestamp
    pub validated_at: chrono::DateTime<chrono::Utc>,
    
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    
    /// Validation version
    pub validation_version: String,
}

/// Individual validation check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheckResult {
    /// Check name
    pub check_name: String,
    
    /// Check status
    pub status: CheckStatus,
    
    /// Risk level
    pub risk_level: RiskLevel,
    
    /// Confidence score (0-1)
    pub confidence: f64,
    
    /// Details about the check
    pub details: String,
    
    /// Evidence supporting the result
    pub evidence: Vec<String>,
    
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// Validation check status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Passed,
    Failed,
    Warning,
    Skipped,
    Error,
}

/// Security recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRecommendation {
    /// Recommendation type
    pub recommendation_type: RecommendationType,
    
    /// Recommendation title
    pub title: String,
    
    /// Recommendation description
    pub description: String,
    
    /// Priority level
    pub priority: Priority,
    
    /// Actionable steps
    pub action_steps: Vec<String>,
    
    /// Expected impact
    pub expected_impact: String,
}

/// Recommendation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    Avoid,
    Investigate,
    Verify,
    ExerciseCaution,
    AdditionalValidation,
    Report,
}

/// Priority levels (reused from portfolio module)
pub use crate::nft::portfolio::Priority;

/// Security rules engine
#[derive(Clone)]
pub struct RulesEngine {
    /// Security rules
    rules: Vec<Arc<dyn SecurityRule>>,
    
    /// Rule cache
    rule_cache: Arc<DashMap<String, bool>>,
}

/// Trait for security rules
#[async_trait]
pub trait SecurityRule: Send + Sync {
    /// Get rule name
    fn name(&self) -> &str;
    
    /// Get rule priority
    fn priority(&self) -> u8;
    
    /// Check if rule applies to the NFT
    fn applies_to(&self, nft: &NftInfo) -> bool;
    
    /// Execute the rule
    async fn execute(&self, nft: &NftInfo) -> NftResult<ValidationCheckResult>;
    
    /// Get required data fields
    fn required_fields(&self) -> Vec<&'static str>;
}

/// Trait for threat intelligence providers
#[async_trait]
pub trait ThreatIntelligenceProvider: Send + Sync {
    /// Check if address is suspicious
    async fn is_address_suspicious(&self, address: &str) -> NftResult<ThreatIntelResult>;
    
    /// Check if domain is malicious
    async fn is_domain_malicious(&self, domain: &str) -> NftResult<ThreatIntelResult>;
    
    /// Check if URI contains threats
    async fn check_uri_threats(&self, uri: &str) -> NftResult<ThreatIntelResult>;
    
    /// Get threat intelligence for collection
    async fn get_collection_threats(&self, collection_id: &str) -> NftResult<CollectionThreatInfo>;
}

/// Threat intelligence result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatIntelResult {
    /// Is threat detected
    pub is_threat: bool,
    
    /// Threat level
    pub threat_level: RiskLevel,
    
    /// Threat types
    pub threat_types: Vec<ThreatType>,
    
    /// Confidence score
    pub confidence: f64,
    
    /// Details about the threat
    pub details: String,
    
    /// Source of intelligence
    pub source: String,
    
    /// First seen timestamp
    pub first_seen: Option<chrono::DateTime<chrono::Utc>>,
    
    /// Last seen timestamp
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

/// Threat types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreatType {
    Phishing,
    Malware,
    Scam,
    RugPull,
    Honeypot,
    Spam,
    SuspiciousActivity,
    Blacklisted,
    Other { threat_type: String },
}

/// Collection threat information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionThreatInfo {
    /// Collection ID
    pub collection_id: String,
    
    /// Overall threat level
    pub threat_level: RiskLevel,
    
    /// Known threats
    pub threats: Vec<ThreatIntelResult>,
    
    /// Suspicious addresses in collection
    pub suspicious_addresses: Vec<String>,
    
    /// Security incidents
    pub incidents: Vec<SecurityIncident>,
    
    /// Community reports
    pub community_reports: u32,
    
    /// Last updated
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Security incident
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityIncident {
    /// Incident ID
    pub incident_id: String,
    
    /// Incident type
    pub incident_type: SecurityIncidentType,
    
    /// Incident description
    pub description: String,
    
    /// Incident severity
    pub severity: RiskLevel,
    
    /// Incident timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Affected addresses
    pub affected_addresses: Vec<String>,
    
    /// Resolution status
    pub resolution_status: ResolutionStatus,
}

/// Security incident types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityIncidentType {
    RugPull,
    PhishingAttack,
    SmartContractExploit,
    MetadataManipulation,
    CreatorImpersonation,
    Other { incident_type: String },
}

/// Resolution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionStatus {
    Unresolved,
    Investigating,
    Resolved,
    FalsePositive,
}

/// Blacklist manager
#[derive(Clone)]
pub struct BlacklistManager {
    /// Blacklisted addresses
    blacklisted_addresses: Arc<DashMap<String, BlacklistEntry>>,
    
    /// Blacklisted domains
    blacklisted_domains: Arc<DashMap<String, BlacklistEntry>>,
    
    /// Blacklisted collections
    blacklisted_collections: Arc<DashMap<String, BlacklistEntry>>,
    
    /// Auto-update enabled
    auto_update: bool,
}

/// Blacklist entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlacklistEntry {
    /// Entry value
    pub value: String,
    
    /// Entry type
    pub entry_type: BlacklistType,
    
    /// Reason for blacklisting
    pub reason: String,
    
    /// Source of blacklist
    pub source: String,
    
    /// Added timestamp
    pub added_at: chrono::DateTime<chrono::Utc>,
    
    /// Expiry timestamp (optional)
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    
    /// Severity level
    pub severity: RiskLevel,
    
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Blacklist types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlacklistType {
    Address,
    Domain,
    Collection,
    Creator,
    MetadataUri,
}

// Security rule implementations

/// Suspicious metadata URI rule
pub struct SuspiciousMetadataRule {
    config: SecurityValidatorConfig,
}

/// Unverified creator rule
pub struct UnverifiedCreatorRule {
    config: SecurityValidatorConfig,
}

/// Broken metadata rule
pub struct BrokenMetadataRule {
    config: SecurityValidatorConfig,
}

/// Suspicious domain rule
pub struct SuspiciousDomainRule {
    config: SecurityValidatorConfig,
}

/// Copymint detection rule
pub struct CopymintDetectionRule {
    config: SecurityValidatorConfig,
}

/// Mock threat intelligence provider
pub struct MockThreatIntelligenceProvider {
    cache: Arc<DashMap<String, ThreatIntelResult>>,
}

impl SecurityValidator {
    /// Create new security validator
    pub fn new(
        connection_pool: Arc<ConnectionPool>,
        config: SecurityValidatorConfig,
        cache_manager: Arc<CacheManager>,
    ) -> NftResult<Self> {
        let rate_limiter = Arc::new(Semaphore::new(config.max_concurrent_validations));
        let metrics = Arc::new(SecurityMetrics::default());
        
        // Initialize rules engine
        let rules: Vec<Arc<dyn SecurityRule>> = vec![
            Arc::new(SuspiciousMetadataRule::new(config.clone())) as Arc<dyn SecurityRule>,
            Arc::new(UnverifiedCreatorRule::new(config.clone())) as Arc<dyn SecurityRule>,
            Arc::new(BrokenMetadataRule::new(config.clone())) as Arc<dyn SecurityRule>,
            Arc::new(SuspiciousDomainRule::new(config.clone())) as Arc<dyn SecurityRule>,
            Arc::new(CopymintDetectionRule::new(config.clone())) as Arc<dyn SecurityRule>,
        ];
        let rules_engine = Arc::new(RulesEngine::new(rules));
        
        // Initialize threat intelligence
        let threat_intel: Arc<dyn ThreatIntelligenceProvider> = Arc::new(MockThreatIntelligenceProvider::new());
        
        // Initialize blacklist manager
        let blacklist_manager = Arc::new(BlacklistManager::new());

        Ok(Self {
            connection_pool,
            cache_manager,
            config,
            rules_engine,
            threat_intel,
            rate_limiter,
            metrics,
            blacklist_manager,
        })
    }

    /// Validate NFT security
    pub async fn validate_nft_security(&self, nft: &NftInfo) -> NftResult<SecurityValidationResult> {
        let start_time = Instant::now();
        self.metrics.total_validations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Check cache first
        let cache_key = CacheKey::security(&nft.mint_address);
        if let Some(cached_result) = self.cache_manager.get_security_validation(&cache_key).await {
            self.metrics.cache_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            debug!("Cache hit for security validation: {}", nft.mint_address);
            return Ok(cached_result);
        }

        self.metrics.cache_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Acquire rate limiter
        let _permit = self.rate_limiter.acquire().await.map_err(|e| {
            NftError::ResourceExhausted {
                message: format!("Failed to acquire rate limiter: {}", e),
                resource_type: "rate_limiter".to_string(),
                current_usage: None,
                limit: Some(self.config.max_concurrent_validations as u64),
            }
        })?;

        let mut validation_results = Vec::new();
        let mut security_issues = Vec::new();
        let mut total_security_score = 100u8;

        // Check blacklist first
        if self.is_blacklisted(nft).await? {
            security_issues.push(SecurityIssue {
                issue_type: SecurityIssueType::SuspiciousMetadata,
                severity: RiskLevel::Critical,
                description: "NFT is blacklisted".to_string(),
                recommendation: "Avoid this NFT completely".to_string(),
                confirmed: true,
                context: Some("Found in blacklist".to_string()),
            });
            total_security_score = 0;
        }

        // Run security rules
        for rule in &self.rules_engine.rules {
            if rule.applies_to(nft) {
                match rule.execute(nft).await {
                    Ok(result) => {
                        if result.status == CheckStatus::Failed {
                            total_security_score = total_security_score.saturating_sub(20);
                            
                            let severity = match result.risk_level {
                                RiskLevel::Critical => RiskLevel::Critical,
                                RiskLevel::High => RiskLevel::High,
                                RiskLevel::Medium => RiskLevel::Medium,
                                RiskLevel::Low => RiskLevel::Low,
                                RiskLevel::None => RiskLevel::Low,
                            };
                            
                            security_issues.push(SecurityIssue {
                                issue_type: SecurityIssueType::SuspiciousMetadata,
                                severity,
                                description: result.details.clone(),
                                recommendation: "Investigate further".to_string(),
                                confirmed: result.confidence > 0.8,
                                context: Some(format!("Rule: {}", rule.name())),
                            });
                        } else if result.status == CheckStatus::Warning {
                            total_security_score = total_security_score.saturating_sub(5);
                        }
                        
                        validation_results.push(result);
                    }
                    Err(e) => {
                        warn!("Security rule {} failed: {}", rule.name(), e);
                    }
                }
            }
        }

        // Threat intelligence check
        if self.config.enable_threat_intel {
            if let Some(threat_result) = self.check_threat_intelligence(nft).await? {
                if threat_result.is_threat {
                    security_issues.push(SecurityIssue {
                        issue_type: SecurityIssueType::SuspiciousMetadata,
                        severity: threat_result.threat_level,
                        description: threat_result.details,
                        recommendation: "Exercise extreme caution".to_string(),
                        confirmed: threat_result.confidence > 0.8,
                        context: Some(format!("Threat intel: {}", threat_result.source)),
                    });
                    total_security_score = total_security_score.saturating_sub(30);
                }
            }
        }

        // Calculate overall risk level
        let overall_risk_level = if security_issues.is_empty() {
            RiskLevel::None
        } else {
            security_issues.iter().map(|i| i.severity).max().unwrap_or(RiskLevel::Low)
        };

        // Generate recommendations
        let recommendations = self.generate_recommendations(&security_issues, overall_risk_level);

        let assessment = SecurityAssessment {
            risk_level: overall_risk_level,
            security_score: total_security_score,
            issues: security_issues,
            verified: nft.collection.as_ref().map(|c| c.verified).unwrap_or(false),
            assessed_at: chrono::Utc::now(),
            confidence: 0.85,
        };

        let validation_result = SecurityValidationResult {
            assessment,
            validation_results,
            recommendations,
            validated_at: chrono::Utc::now(),
            processing_time_ms: start_time.elapsed().as_millis() as u64,
            validation_version: "1.0.0".to_string(),
        };

        // Cache the result
        self.cache_manager.set_security_validation(&cache_key, &validation_result).await;

        // Update metrics
        self.metrics.successful_validations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let validation_time_ms = start_time.elapsed().as_millis() as u64;
        self.metrics.avg_validation_time_ms.fetch_add(validation_time_ms, std::sync::atomic::Ordering::Relaxed);
        
        if !validation_result.assessment.issues.is_empty() {
            self.metrics.security_issues_found.fetch_add(
                validation_result.assessment.issues.len() as u64,
                std::sync::atomic::Ordering::Relaxed
            );
        }

        self.metrics.validations_by_risk
            .entry(overall_risk_level)
            .or_insert(0)
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        info!("Security validation completed for {} in {}ms - Risk: {}, Score: {}", 
            nft.mint_address, validation_time_ms, overall_risk_level, total_security_score);

        Ok(validation_result)
    }

    /// Batch validate NFT security
    pub async fn batch_validate_security(&self, nfts: &[NftInfo]) -> NftResult<Vec<SecurityValidationResult>> {
        let start_time = Instant::now();
        
        let results: Vec<NftResult<SecurityValidationResult>> = futures::stream::iter(nfts)
            .map(|nft| async move {
                self.validate_nft_security(nft).await
            })
            .buffer_unordered(self.config.max_concurrent_validations)
            .collect()
            .await;

        let mut successful_results = Vec::new();
        let mut failed_count = 0;

        for result in results {
            match result {
                Ok(validation) => successful_results.push(validation),
                Err(e) => {
                    error!("Security validation failed: {}", e);
                    failed_count += 1;
                }
            }
        }

        let total_time_ms = start_time.elapsed().as_millis() as u64;
        info!(
            "Batch security validation completed: {} successful, {} failed in {}ms",
            successful_results.len(),
            failed_count,
            total_time_ms
        );

        Ok(successful_results)
    }

    /// Check if NFT is blacklisted
    async fn is_blacklisted(&self, nft: &NftInfo) -> NftResult<bool> {
        // Check mint address
        if self.blacklist_manager.is_address_blacklisted(&nft.mint_address).await {
            self.metrics.blacklist_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(true);
        }

        // Check metadata URI
        if let Some(metadata_uri) = &nft.metadata_uri {
            if let Ok(url) = url::Url::parse(metadata_uri) {
                if let Some(domain) = url.host_str() {
                    if self.blacklist_manager.is_domain_blacklisted(domain).await {
                        self.metrics.blacklist_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return Ok(true);
                    }
                }
            }
        }

        // Check collection
        if let Some(collection) = &nft.collection {
            if self.blacklist_manager.is_collection_blacklisted(&collection.name).await {
                self.metrics.blacklist_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check threat intelligence
    async fn check_threat_intelligence(&self, nft: &NftInfo) -> NftResult<Option<ThreatIntelResult>> {
        self.metrics.threat_intel_lookups.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Check mint address
        if let Ok(result) = self.threat_intel.is_address_suspicious(&nft.mint_address).await {
            if result.is_threat {
                return Ok(Some(result));
            }
        }

        // Check metadata URI
        if let Some(metadata_uri) = &nft.metadata_uri {
            if let Ok(result) = self.threat_intel.check_uri_threats(metadata_uri).await {
                if result.is_threat {
                    return Ok(Some(result));
                }
            }
        }

        // Check collection
        if let Some(collection) = &nft.collection {
            if let Ok(collection_threats) = self.threat_intel.get_collection_threats(&collection.name).await {
                if collection_threats.threat_level >= RiskLevel::Medium {
                    return Ok(Some(ThreatIntelResult {
                        is_threat: true,
                        threat_level: collection_threats.threat_level,
                        threat_types: vec![ThreatType::Scam],
                        confidence: 0.8,
                        details: format!("Collection has {} security incidents", collection_threats.incidents.len()),
                        source: "collection_threat_intel".to_string(),
                        first_seen: None,
                        last_seen: Some(collection_threats.last_updated),
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Generate security recommendations
    fn generate_recommendations(&self, issues: &[SecurityIssue], risk_level: RiskLevel) -> Vec<SecurityRecommendation> {
        let mut recommendations = Vec::new();

        if risk_level == RiskLevel::Critical {
            recommendations.push(SecurityRecommendation {
                recommendation_type: RecommendationType::Avoid,
                title: "Critical Security Risk Detected".to_string(),
                description: "This NFT poses a critical security risk to your wallet".to_string(),
                priority: Priority::Critical,
                action_steps: vec![
                    "Do not interact with this NFT".to_string(),
                    "Report the NFT to relevant authorities".to_string(),
                    "Scan your wallet for any unauthorized transactions".to_string(),
                ],
                expected_impact: "Prevents potential loss of assets and protects wallet security".to_string(),
            });
        } else if risk_level == RiskLevel::High {
            recommendations.push(SecurityRecommendation {
                recommendation_type: RecommendationType::ExerciseCaution,
                title: "High Security Risk Detected".to_string(),
                description: "This NFT has significant security concerns".to_string(),
                priority: Priority::High,
                action_steps: vec![
                    "Exercise extreme caution".to_string(),
                    "Verify all details before any interaction".to_string(),
                    "Consider using a separate wallet for testing".to_string(),
                ],
                expected_impact: "Reduces risk of potential security incidents".to_string(),
            });
        } else if risk_level == RiskLevel::Medium {
            recommendations.push(SecurityRecommendation {
                recommendation_type: RecommendationType::Investigate,
                title: "Medium Security Risk Detected".to_string(),
                description: "This NFT has some security concerns that warrant investigation".to_string(),
                priority: Priority::Medium,
                action_steps: vec![
                    "Research the collection and creators".to_string(),
                    "Verify metadata authenticity".to_string(),
                    "Check community feedback".to_string(),
                ],
                expected_impact: "Ensures informed decision-making".to_string(),
            });
        }

        // Add specific recommendations based on issue types
        for issue in issues {
            match issue.issue_type {
                SecurityIssueType::SuspiciousMetadata => {
                    recommendations.push(SecurityRecommendation {
                        recommendation_type: RecommendationType::AdditionalValidation,
                        title: "Suspicious Metadata Detected".to_string(),
                        description: "The metadata contains suspicious elements".to_string(),
                        priority: Priority::Medium,
                        action_steps: vec![
                            "Verify metadata URI authenticity".to_string(),
                            "Check for metadata manipulation".to_string(),
                            "Cross-reference with official sources".to_string(),
                        ],
                        expected_impact: "Ensures metadata integrity".to_string(),
                    });
                }
                SecurityIssueType::UnauthorizedCreator => {
                    recommendations.push(SecurityRecommendation {
                        recommendation_type: RecommendationType::Verify,
                        title: "Unauthorized Creator Detected".to_string(),
                        description: "The creator information appears suspicious".to_string(),
                        priority: Priority::High,
                        action_steps: vec![
                            "Verify creator identity".to_string(),
                            "Check creator's other works".to_string(),
                            "Look for official verification".to_string(),
                        ],
                        expected_impact: "Prevents supporting unauthorized content".to_string(),
                    });
                }
                _ => {}
            }
        }

        recommendations
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> &SecurityMetrics {
        &self.metrics
    }

    /// Get blacklist manager
    pub fn get_blacklist_manager(&self) -> &BlacklistManager {
        &self.blacklist_manager
    }
}

// Implement security rules

impl SuspiciousMetadataRule {
    pub fn new(config: SecurityValidatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl SecurityRule for SuspiciousMetadataRule {
    fn name(&self) -> &str {
        "SuspiciousMetadata"
    }

    fn priority(&self) -> u8 {
        90 // High priority
    }

    fn applies_to(&self, nft: &NftInfo) -> bool {
        nft.metadata_uri.is_some()
    }

    async fn execute(&self, nft: &NftInfo) -> NftResult<ValidationCheckResult> {
        let start_time = Instant::now();
        
        if let Some(metadata_uri) = &nft.metadata_uri {
            // Check for suspicious patterns
            let suspicious_patterns = vec![
                "bit.ly", "tinyurl.com", "t.co", "goo.gl", // URL shorteners
                "discord.gg", "telegram.org", // Social media links
                "0x0000000000000000000000000000000000000000", // Zero address
            ];

            let mut suspicious_found = false;
            let mut evidence = Vec::new();

            for pattern in &suspicious_patterns {
                if metadata_uri.contains(pattern) {
                    suspicious_found = true;
                    evidence.push(format!("Contains suspicious pattern: {}", pattern));
                }
            }

            // Check for IPFS hash manipulation
            if metadata_uri.contains("ipfs") {
                if let Some(hash) = metadata_uri.split('/').last() {
                    if hash.len() != 46 || !hash.chars().all(|c| c.is_ascii_alphanumeric()) {
                        suspicious_found = true;
                        evidence.push("Invalid IPFS hash format".to_string());
                    }
                }
            }

            let status = if suspicious_found {
                CheckStatus::Failed
            } else {
                CheckStatus::Passed
            };

            let risk_level = if suspicious_found {
                RiskLevel::High
            } else {
                RiskLevel::None
            };

            Ok(ValidationCheckResult {
                check_name: self.name().to_string(),
                status,
                risk_level,
                confidence: 0.8,
                details: if suspicious_found {
                    "Suspicious patterns detected in metadata URI".to_string()
                } else {
                    "Metadata URI appears legitimate".to_string()
                },
                evidence,
                processing_time_ms: start_time.elapsed().as_millis() as u64,
            })
        } else {
            Ok(ValidationCheckResult {
                check_name: self.name().to_string(),
                status: CheckStatus::Skipped,
                risk_level: RiskLevel::None,
                confidence: 0.0,
                details: "No metadata URI to check".to_string(),
                evidence: vec![],
                processing_time_ms: start_time.elapsed().as_millis() as u64,
            })
        }
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["metadata_uri"]
    }
}

impl UnverifiedCreatorRule {
    pub fn new(config: SecurityValidatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl SecurityRule for UnverifiedCreatorRule {
    fn name(&self) -> &str {
        "UnverifiedCreator"
    }

    fn priority(&self) -> u8 {
        70
    }

    fn applies_to(&self, nft: &NftInfo) -> bool {
        !nft.creators.is_empty()
    }

    async fn execute(&self, nft: &NftInfo) -> NftResult<ValidationCheckResult> {
        let start_time = Instant::now();
        
        let verified_creators = nft.creators.iter().filter(|c| c.verified).count();
        let total_creators = nft.creators.len();

        let status = if verified_creators == 0 {
            CheckStatus::Warning
        } else if verified_creators < total_creators {
            CheckStatus::Warning
        } else {
            CheckStatus::Passed
        };

        let risk_level = if verified_creators == 0 {
            RiskLevel::Medium
        } else if verified_creators < total_creators {
            RiskLevel::Low
        } else {
            RiskLevel::None
        };

        let confidence = if verified_creators == 0 {
            0.7
        } else if verified_creators < total_creators {
            0.5
        } else {
            0.9
        };

        Ok(ValidationCheckResult {
            check_name: self.name().to_string(),
            status,
            risk_level,
            confidence,
            details: format!("{}/{} creators are verified", verified_creators, total_creators),
            evidence: vec![format!("Verified creators: {}/{}", verified_creators, total_creators)],
            processing_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["creators"]
    }
}

impl BrokenMetadataRule {
    pub fn new(config: SecurityValidatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl SecurityRule for BrokenMetadataRule {
    fn name(&self) -> &str {
        "BrokenMetadata"
    }

    fn priority(&self) -> u8 {
        80
    }

    fn applies_to(&self, nft: &NftInfo) -> bool {
        nft.name.is_none() || nft.metadata_uri.is_none()
    }

    async fn execute(&self, nft: &NftInfo) -> NftResult<ValidationCheckResult> {
        let start_time = Instant::now();
        
        let mut issues = Vec::new();
        
        if nft.name.is_none() {
            issues.push("Missing name");
        }
        
        if nft.metadata_uri.is_none() {
            issues.push("Missing metadata URI");
        }
        
        if nft.symbol.is_none() {
            issues.push("Missing symbol");
        }

        let status = if issues.is_empty() {
            CheckStatus::Passed
        } else if issues.len() == 1 {
            CheckStatus::Warning
        } else {
            CheckStatus::Failed
        };

        let risk_level = if issues.len() >= 2 {
            RiskLevel::Medium
        } else if !issues.is_empty() {
            RiskLevel::Low
        } else {
            RiskLevel::None
        };

        Ok(ValidationCheckResult {
            check_name: self.name().to_string(),
            status,
            risk_level,
            confidence: 0.8,
            details: format!("Metadata issues: {}", issues.join(", ")),
            evidence: issues.iter().map(|issue| format!("Missing: {}", issue)).collect(),
            processing_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["name", "metadata_uri", "symbol"]
    }
}

impl SuspiciousDomainRule {
    pub fn new(config: SecurityValidatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl SecurityRule for SuspiciousDomainRule {
    fn name(&self) -> &str {
        "SuspiciousDomain"
    }

    fn priority(&self) -> u8 {
        85
    }

    fn applies_to(&self, nft: &NftInfo) -> bool {
        nft.metadata_uri.is_some() || nft.image_uri.is_some()
    }

    async fn execute(&self, nft: &NftInfo) -> NftResult<ValidationCheckResult> {
        let start_time = Instant::now();
        
        let uris = vec![
            nft.metadata_uri.as_ref(),
            nft.image_uri.as_ref(),
            nft.animation_uri.as_ref(),
            nft.external_url.as_ref(),
        ].into_iter().flatten();

        let suspicious_domains = vec![
            "pastebin.com", "discord.gg", "telegram.org", 
            "bit.ly", "tinyurl.com", "t.co",
        ];

        let mut suspicious_found = false;
        let mut evidence = Vec::new();

        for uri in uris {
            if let Ok(url) = url::Url::parse(uri) {
                if let Some(domain) = url.host_str() {
                    for suspicious_domain in &suspicious_domains {
                        if domain.contains(suspicious_domain) {
                            suspicious_found = true;
                            evidence.push(format!("Suspicious domain in URI: {}", domain));
                        }
                    }
                }
            }
        }

        let status = if suspicious_found {
            CheckStatus::Failed
        } else {
            CheckStatus::Passed
        };

        let risk_level = if suspicious_found {
            RiskLevel::High
        } else {
            RiskLevel::None
        };

        Ok(ValidationCheckResult {
            check_name: self.name().to_string(),
            status,
            risk_level,
            confidence: 0.9,
            details: if suspicious_found {
                "Suspicious domains detected in URIs".to_string()
            } else {
                "All domains appear legitimate".to_string()
            },
            evidence,
            processing_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["metadata_uri", "image_uri", "animation_uri", "external_url"]
    }
}

impl CopymintDetectionRule {
    pub fn new(config: SecurityValidatorConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl SecurityRule for CopymintDetectionRule {
    fn name(&self) -> &str {
        "CopymintDetection"
    }

    fn priority(&self) -> u8 {
        75
    }

    fn applies_to(&self, nft: &NftInfo) -> bool {
        nft.name.is_some() && nft.collection.is_some()
    }

    async fn execute(&self, nft: &NftInfo) -> NftResult<ValidationCheckResult> {
        let start_time = Instant::now();
        
        // This is a simplified copymint detection
        // In a real implementation, this would check against a database of known NFTs
        let mut suspicious_indicators = Vec::new();
        
        // Check for generic names that might indicate copymints
        if let Some(name) = &nft.name {
            let generic_patterns = vec![
                r"#\d+$", // Ends with number (common for copymints)
                r"(?i)copy|fake|unofficial", // Contains copy-related words
            ];
            
            for pattern in &generic_patterns {
                if let Ok(regex) = Regex::new(pattern) {
                    if regex.is_match(name) {
                        suspicious_indicators.push(format!("Name matches pattern: {}", pattern));
                    }
                }
            }
        }

        // Check if collection is verified but NFT isn't
        if let (Some(collection), false) = (&nft.collection, nft.metadata_verified) {
            if collection.verified {
                suspicious_indicators.push("Unverified NFT in verified collection".to_string());
            }
        }

        let status = if suspicious_indicators.is_empty() {
            CheckStatus::Passed
        } else if suspicious_indicators.len() == 1 {
            CheckStatus::Warning
        } else {
            CheckStatus::Failed
        };

        let risk_level = if suspicious_indicators.is_empty() {
            RiskLevel::None
        } else if suspicious_indicators.len() == 1 {
            RiskLevel::Low
        } else {
            RiskLevel::Medium
        };

        Ok(ValidationCheckResult {
            check_name: self.name().to_string(),
            status,
            risk_level,
            confidence: 0.6,
            details: if suspicious_indicators.is_empty() {
                "No copymint indicators detected".to_string()
            } else {
                format!("Copymint indicators: {}", suspicious_indicators.join(", "))
            },
            evidence: suspicious_indicators,
            processing_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    fn required_fields(&self) -> Vec<&'static str> {
        vec!["name", "collection", "metadata_verified"]
    }
}

// Implement rules engine

impl RulesEngine {
    pub fn new(rules: Vec<Arc<dyn SecurityRule>>) -> Self {
        Self {
            rules,
            rule_cache: Arc::new(DashMap::new()),
        }
    }
}

// Implement threat intelligence provider

impl MockThreatIntelligenceProvider {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl ThreatIntelligenceProvider for MockThreatIntelligenceProvider {
    async fn is_address_suspicious(&self, address: &str) -> NftResult<ThreatIntelResult> {
        // Mock implementation - in reality this would query threat intelligence APIs
        let suspicious_addresses = vec![
            "11111111111111111111111111111111", // System program
            "11111111111111111111111111111112", // System program
        ];

        let is_suspicious = suspicious_addresses.contains(&address);

        Ok(ThreatIntelResult {
            is_threat: is_suspicious,
            threat_level: if is_suspicious { RiskLevel::High } else { RiskLevel::None },
            threat_types: if is_suspicious {
                vec![ThreatType::Blacklisted]
            } else {
                vec![]
            },
            confidence: 0.9,
            details: if is_suspicious {
                "Address is in suspicious address list".to_string()
            } else {
                "No threats detected".to_string()
            },
            source: "mock_threat_intel".to_string(),
            first_seen: None,
            last_seen: None,
        })
    }

    async fn is_domain_malicious(&self, domain: &str) -> NftResult<ThreatIntelResult> {
        // Mock implementation
        let malicious_domains = vec![
            "malicious-site.com",
            "phishing-example.org",
        ];

        let is_malicious = malicious_domains.contains(&domain);

        Ok(ThreatIntelResult {
            is_threat: is_malicious,
            threat_level: if is_malicious { RiskLevel::Critical } else { RiskLevel::None },
            threat_types: if is_malicious {
                vec![ThreatType::Phishing, ThreatType::Malware]
            } else {
                vec![]
            },
            confidence: 0.95,
            details: if is_malicious {
                "Domain is known to be malicious".to_string()
            } else {
                "No threats detected".to_string()
            },
            source: "mock_threat_intel".to_string(),
            first_seen: None,
            last_seen: None,
        })
    }

    async fn check_uri_threats(&self, uri: &str) -> NftResult<ThreatIntelResult> {
        // Mock implementation - extract domain and check
        if let Ok(url) = url::Url::parse(uri) {
            if let Some(domain) = url.host_str() {
                return self.is_domain_malicious(domain).await;
            }
        }

        Ok(ThreatIntelResult {
            is_threat: false,
            threat_level: RiskLevel::None,
            threat_types: vec![],
            confidence: 0.5,
            details: "Could not parse URI for threat analysis".to_string(),
            source: "mock_threat_intel".to_string(),
            first_seen: None,
            last_seen: None,
        })
    }

    async fn get_collection_threats(&self, collection_id: &str) -> NftResult<CollectionThreatInfo> {
        // Mock implementation
        Ok(CollectionThreatInfo {
            collection_id: collection_id.to_string(),
            threat_level: RiskLevel::None,
            threats: vec![],
            suspicious_addresses: vec![],
            incidents: vec![],
            community_reports: 0,
            last_updated: chrono::Utc::now(),
        })
    }
}

// Implement blacklist manager

impl BlacklistManager {
    pub fn new() -> Self {
        let manager = Self {
            blacklisted_addresses: Arc::new(DashMap::new()),
            blacklisted_domains: Arc::new(DashMap::new()),
            blacklisted_collections: Arc::new(DashMap::new()),
            auto_update: true,
        };

        // Initialize with some default entries
        manager.initialize_default_entries();
        manager
    }

    fn initialize_default_entries(&self) {
        // Add some known malicious addresses
        let malicious_addresses = vec![
            "11111111111111111111111111111111",
            "11111111111111111111111111111112",
        ];

        for address in malicious_addresses {
            self.blacklisted_addresses.insert(
                address.to_string(),
                BlacklistEntry {
                    value: address.to_string(),
                    entry_type: BlacklistType::Address,
                    reason: "Known malicious address".to_string(),
                    source: "default_blacklist".to_string(),
                    added_at: chrono::Utc::now(),
                    expires_at: None,
                    severity: RiskLevel::Critical,
                    metadata: HashMap::new(),
                },
            );
        }
    }

    pub async fn is_address_blacklisted(&self, address: &str) -> bool {
        self.blacklisted_addresses.contains_key(address)
    }

    pub async fn is_domain_blacklisted(&self, domain: &str) -> bool {
        self.blacklisted_domains.contains_key(domain)
    }

    pub async fn is_collection_blacklisted(&self, collection: &str) -> bool {
        self.blacklisted_collections.contains_key(collection)
    }

    pub fn add_address_blacklist(&self, address: String, entry: BlacklistEntry) {
        self.blacklisted_addresses.insert(address, entry);
    }

    pub fn add_domain_blacklist(&self, domain: String, entry: BlacklistEntry) {
        self.blacklisted_domains.insert(domain, entry);
    }

    pub fn add_collection_blacklist(&self, collection: String, entry: BlacklistEntry) {
        self.blacklisted_collections.insert(collection, entry);
    }
}
