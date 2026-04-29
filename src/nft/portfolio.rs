//! # NFT Portfolio Analyzer
//!
//! Ultra-fast, comprehensive portfolio analysis with advanced metrics,
//! risk assessment, and actionable insights.

use crate::nft::errors::{NftError, NftResult};
use crate::nft::types::*;
use crate::nft::valuation::ValuationEngine;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Comprehensive NFT portfolio analyzer
#[derive(Clone)]
pub struct PortfolioAnalyzer {
    /// Valuation engine
    valuation_engine: Arc<ValuationEngine>,
    
    /// Configuration
    config: PortfolioAnalyzerConfig,
    
    /// Performance metrics
    metrics: Arc<PortfolioMetrics>,
}

/// Portfolio analyzer configuration
#[derive(Debug, Clone)]
pub struct PortfolioAnalyzerConfig {
    /// Enable deep analysis
    pub enable_deep_analysis: bool,
    
    /// Enable risk assessment
    pub enable_risk_assessment: bool,
    
    /// Enable trend analysis
    pub enable_trend_analysis: bool,
    
    /// Enable performance benchmarking
    pub enable_performance_benchmarking: bool,
    
    /// Maximum NFTs to analyze per portfolio
    pub max_nfts_per_portfolio: Option<u32>,
    
    /// Analysis timeout in seconds
    pub analysis_timeout_seconds: u64,
    
    /// Confidence threshold for insights
    pub confidence_threshold: f64,
    
    /// Risk tolerance level
    pub risk_tolerance: RiskTolerance,
}

/// Risk tolerance levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskTolerance {
    Conservative,
    Moderate,
    Aggressive,
}

impl Default for PortfolioAnalyzerConfig {
    fn default() -> Self {
        Self {
            enable_deep_analysis: true,
            enable_risk_assessment: true,
            enable_trend_analysis: true,
            enable_performance_benchmarking: false,
            max_nfts_per_portfolio: None,
            analysis_timeout_seconds: 300, // 5 minutes
            confidence_threshold: 0.7,
            risk_tolerance: RiskTolerance::Moderate,
        }
    }
}

/// Portfolio performance metrics
#[derive(Debug, Default)]
pub struct PortfolioMetrics {
    /// Total portfolios analyzed
    pub total_portfolios: Arc<std::sync::atomic::AtomicU64>,
    
    /// Successful analyses
    pub successful_analyses: Arc<std::sync::atomic::AtomicU64>,
    
    /// Failed analyses
    pub failed_analyses: Arc<std::sync::atomic::AtomicU64>,
    
    /// Average analysis time in milliseconds
    pub avg_analysis_time_ms: Arc<std::sync::atomic::AtomicU64>,
    
    /// Total NFTs processed
    pub total_nfts_processed: Arc<std::sync::atomic::AtomicU64>,
    
    /// Average portfolio size
    pub avg_portfolio_size: Arc<std::sync::atomic::AtomicF64>,
    
    /// High-risk portfolios identified
    pub high_risk_portfolios: Arc<std::sync::atomic::AtomicU64>,
    
    /// Insights generated
    pub insights_generated: Arc<std::sync::atomic::AtomicU64>,
}

/// Portfolio analysis insights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioInsight {
    /// Insight type
    pub insight_type: InsightType,
    
    /// Insight title
    pub title: String,
    
    /// Insight description
    pub description: String,
    
    /// Confidence score (0-1)
    pub confidence: f64,
    
    /// Priority level
    pub priority: Priority,
    
    /// Actionable recommendation
    pub recommendation: Option<String>,
    
    /// Related NFT addresses
    pub related_nfts: Vec<String>,
    
    /// Data supporting the insight
    pub supporting_data: HashMap<String, serde_json::Value>,
    
    /// When insight was generated
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Types of portfolio insights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InsightType {
    /// Risk-related insight
    Risk,
    /// Value-related insight
    Value,
    /// Diversification insight
    Diversification,
    /// Performance insight
    Performance,
    /// Trend insight
    Trend,
    /// Security insight
    Security,
    /// Opportunity insight
    Opportunity,
    /// Warning insight
    Warning,
}

/// Priority levels for insights
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// Portfolio comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioComparison {
    /// Portfolio being compared
    pub portfolio_address: String,
    
    /// Comparison metrics
    pub metrics: ComparisonMetrics,
    
    /// Relative performance (0-100, higher is better)
    pub relative_performance: f64,
    
    /// Strengths compared to benchmark
    pub strengths: Vec<String>,
    
    /// Weaknesses compared to benchmark
    pub weaknesses: Vec<String>,
    
    /// Recommendations
    pub recommendations: Vec<String>,
    
    /// Comparison timestamp
    pub compared_at: chrono::DateTime<chrono::Utc>,
}

/// Comparison metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonMetrics {
    /// Value percentile (0-100)
    pub value_percentile: f64,
    
    /// Diversity percentile (0-100)
    pub diversity_percentile: f64,
    
    /// Risk percentile (0-100, lower is better)
    pub risk_percentile: f64,
    
    /// Quality percentile (0-100)
    pub quality_percentile: f64,
    
    /// Growth percentile (0-100)
    pub growth_percentile: f64,
}

impl PortfolioAnalyzer {
    /// Create new portfolio analyzer
    pub fn new(valuation_engine: Arc<ValuationEngine>, config: PortfolioAnalyzerConfig) -> Self {
        let metrics = Arc::new(PortfolioMetrics::default());
        
        Self {
            valuation_engine,
            config,
            metrics,
        }
    }

    /// Analyze NFT portfolio
    pub async fn analyze_portfolio(&self, wallet_address: &str, nfts: &[NftInfo]) -> NftResult<NftPortfolio> {
        let start_time = Instant::now();
        self.metrics.total_portfolios.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Apply portfolio size limit
        let nfts_to_analyze = if let Some(limit) = self.config.max_nfts_per_portfolio {
            if nfts.len() > limit as usize {
                warn!("Portfolio size {} exceeds limit {}, analyzing first {} NFTs", 
                    nfts.len(), limit, limit);
                &nfts[..limit as usize]
            } else {
                nfts
            }
        } else {
            nfts
        };

        self.metrics.total_nfts_processed.fetch_add(
            nfts_to_analyze.len() as u64, 
            std::sync::atomic::Ordering::Relaxed
        );

        // Value all NFTs in parallel
        let valuation_results = self.valuation_engine.value_nfts(nfts_to_analyze).await?;
        
        // Merge valuations with NFT info
        let mut valued_nfts = Vec::new();
        for (nft, valuation) in nfts_to_analyze.iter().zip(valuation_results) {
            let mut valued_nft = nft.clone();
            valued_nft.estimated_value_lamports = Some(valuation.estimated_value_lamports);
            valued_nft.last_valuation = Some(valuation.last_updated);
            valued_nfts.push(valued_nft);
        }

        // Calculate portfolio metrics
        let total_value = valued_nfts.iter()
            .filter_map(|nft| nft.estimated_value_lamports)
            .sum();

        let verified_count = valued_nfts.iter()
            .filter(|nft| nft.metadata_verified)
            .count() as u32;

        let high_risk_count = valued_nfts.iter()
            .filter(|nft| nft.security_assessment.risk_level >= RiskLevel::High)
            .count() as u32;

        // Collection breakdown
        let collection_breakdown = self.calculate_collection_breakdown(&valued_nfts);

        // Value distribution
        let value_distribution = self.calculate_value_distribution(&valued_nfts);

        // Risk distribution
        let risk_distribution = self.calculate_risk_distribution(&valued_nfts);

        // Quality metrics
        let quality_metrics = self.calculate_quality_metrics(&valued_nfts);

        let portfolio = NftPortfolio {
            id: uuid::Uuid::new_v4(),
            wallet_address: wallet_address.to_string(),
            nfts: valued_nfts,
            total_value_lamports: total_value,
            total_count: nfts_to_analyze.len() as u32,
            verified_count,
            high_risk_count,
            collection_breakdown,
            value_distribution,
            risk_distribution,
            quality_metrics,
            analyzed_at: chrono::Utc::now(),
            analysis_duration_ms: start_time.elapsed().as_millis() as u64,
            analysis_config: format!("{:?}", self.config),
        };

        // Update metrics
        self.metrics.successful_analyses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let analysis_time_ms = start_time.elapsed().as_millis() as u64;
        self.metrics.avg_analysis_time_ms.fetch_add(analysis_time_ms, std::sync::atomic::Ordering::Relaxed);
        self.metrics.avg_portfolio_size.fetch_add(
            nfts_to_analyze.len() as f64, 
            std::sync::atomic::Ordering::Relaxed
        );

        if portfolio.high_risk_count > 0 {
            self.metrics.high_risk_portfolios.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        info!("Analyzed portfolio for {}: {} NFTs, {} SOL value in {}ms", 
            wallet_address, portfolio.total_count, portfolio.total_value_lamports as f64 / 1_000_000_000.0, analysis_time_ms);

        Ok(portfolio)
    }

    /// Generate portfolio insights
    pub async fn generate_insights(&self, portfolio: &NftPortfolio) -> NftResult<Vec<PortfolioInsight>> {
        let mut insights = Vec::new();

        // Risk insights
        if self.config.enable_risk_assessment {
            insights.extend(self.generate_risk_insights(portfolio)?);
        }

        // Value insights
        insights.extend(self.generate_value_insights(portfolio)?);

        // Diversification insights
        insights.extend(self.generate_diversification_insights(portfolio)?);

        // Performance insights
        if self.config.enable_performance_benchmarking {
            insights.extend(self.generate_performance_insights(portfolio)?);
        }

        // Trend insights
        if self.config.enable_trend_analysis {
            insights.extend(self.generate_trend_insights(portfolio)?);
        }

        // Security insights
        insights.extend(self.generate_security_insights(portfolio)?);

        // Sort by priority and confidence
        insights.sort_by(|a, b| {
            b.priority.cmp(&a.priority)
                .then_with(|| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
        });

        self.metrics.insights_generated.fetch_add(
            insights.len() as u64, 
            std::sync::atomic::Ordering::Relaxed
        );

        Ok(insights)
    }

    /// Compare portfolio to benchmark
    pub async fn compare_to_benchmark(&self, portfolio: &NftPortfolio) -> NftResult<PortfolioComparison> {
        // In a real implementation, this would compare against a database of portfolios
        // For now, we'll generate mock comparison data
        
        let metrics = ComparisonMetrics {
            value_percentile: 65.0,
            diversity_percentile: 45.0,
            risk_percentile: 30.0, // Lower is better for risk
            quality_percentile: 70.0,
            growth_percentile: 55.0,
        };

        let relative_performance = (metrics.value_percentile + metrics.diversity_percentile + 
            (100.0 - metrics.risk_percentile) + metrics.quality_percentile + metrics.growth_percentile) / 5.0;

        let mut strengths = Vec::new();
        let mut weaknesses = Vec::new();
        let mut recommendations = Vec::new();

        if metrics.value_percentile > 70.0 {
            strengths.push("Strong portfolio value".to_string());
        } else if metrics.value_percentile < 30.0 {
            weaknesses.push("Below average portfolio value".to_string());
            recommendations.push("Consider acquiring higher-value NFTs".to_string());
        }

        if metrics.diversity_percentile < 30.0 {
            weaknesses.push("Low collection diversity".to_string());
            recommendations.push("Diversify across different collections".to_string());
        }

        if metrics.risk_percentile > 70.0 {
            weaknesses.push("High risk exposure".to_string());
            recommendations.push("Review and reduce high-risk holdings".to_string());
        }

        Ok(PortfolioComparison {
            portfolio_address: portfolio.wallet_address.clone(),
            metrics,
            relative_performance,
            strengths,
            weaknesses,
            recommendations,
            compared_at: chrono::Utc::now(),
        })
    }

    /// Calculate collection breakdown
    fn calculate_collection_breakdown(&self, nfts: &[NftInfo]) -> HashMap<String, CollectionBreakdown> {
        let mut collection_stats: HashMap<String, (u32, u64)> = HashMap::new();

        // Count NFTs and sum values per collection
        for nft in nfts {
            let collection_name = nft.collection.as_ref()
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            let (count, total_value) = collection_stats.entry(collection_name).or_insert((0, 0));
            *count += 1;
            *total_value += nft.estimated_value_lamports.unwrap_or(0);
        }

        let total_portfolio_value = collection_stats.values()
            .map(|(_, value)| *value)
            .sum::<u64>();

        let mut breakdown = HashMap::new();
        for (collection_name, (count, total_value)) in collection_stats {
            let average_value = if count > 0 { total_value / count as u64 } else { 0 };
            let portfolio_percentage = if total_portfolio_value > 0 {
                total_value as f64 / total_portfolio_value as f64 * 100.0
            } else {
                0.0
            };

            // Find if collection is verified
            let verified = nfts.iter()
                .filter(|nft| nft.collection.as_ref().map(|c| &c.name) == Some(&collection_name))
                .any(|nft| nft.collection.as_ref().map(|c| c.verified).unwrap_or(false));

            breakdown.insert(collection_name.clone(), CollectionBreakdown {
                collection_name,
                count,
                total_value_lamports: total_value,
                average_value_lamports: average_value,
                portfolio_percentage,
                verified,
            });
        }

        breakdown
    }

    /// Calculate value distribution
    fn calculate_value_distribution(&self, nfts: &[NftInfo]) -> ValueDistribution {
        let mut values: Vec<u64> = nfts.iter()
            .filter_map(|nft| nft.estimated_value_lamports)
            .collect();

        if values.is_empty() {
            return ValueDistribution {
                highest_value: None,
                lowest_value: None,
                median_value: None,
                average_value: 0.0,
                percentiles: HashMap::new(),
                concentration: 0.0,
            };
        }

        values.sort_unstable();

        let highest_value = *values.last().unwrap();
        let lowest_value = *values.first().unwrap();
        let median_value = if values.len() % 2 == 0 {
            (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2
        } else {
            values[values.len() / 2]
        };

        let average_value = values.iter().sum::<u64>() as f64 / values.len() as f64;

        // Calculate percentiles
        let mut percentiles = HashMap::new();
        let percentile_keys = [25, 50, 75, 90, 95, 99];
        for &p in &percentile_keys {
            let index = ((p as f64 / 100.0) * (values.len() - 1) as f64) as usize;
            percentiles.insert(p, values[index]);
        }

        // Calculate Gini coefficient for concentration
        let concentration = self.calculate_gini_coefficient(&values);

        ValueDistribution {
            highest_value: Some(highest_value),
            lowest_value: Some(lowest_value),
            median_value: Some(median_value),
            average_value,
            percentiles,
            concentration,
        }
    }

    /// Calculate risk distribution
    fn calculate_risk_distribution(&self, nfts: &[NftInfo]) -> RiskDistribution {
        let mut counts = HashMap::new();
        let mut value_by_risk = HashMap::new();

        for nft in nfts {
            let risk_level = nft.security_assessment.risk_level;
            let value = nft.estimated_value_lamports.unwrap_or(0);

            *counts.entry(risk_level).or_insert(0) += 1;
            *value_by_risk.entry(risk_level).or_insert(0) += value;
        }

        let total_count = nfts.len() as u32;
        let total_value = value_by_risk.values().sum::<u64>();

        let mut percentages = HashMap::new();
        for (risk_level, count) in &counts {
            let percentage = *count as f64 / total_count as f64 * 100.0;
            percentages.insert(*risk_level, percentage);
        }

        // Calculate overall risk score
        let overall_risk_score = self.calculate_overall_risk_score(&counts, &value_by_risk);

        RiskDistribution {
            counts,
            value_by_risk,
            percentages,
            overall_risk_score,
        }
    }

    /// Calculate quality metrics
    fn calculate_quality_metrics(&self, nfts: &[NftInfo]) -> PortfolioQualityMetrics {
        let rarity_scores: Vec<f64> = nfts.iter()
            .filter_map(|nft| nft.rarity_score)
            .collect();

        let quality_scores: Vec<f64> = nfts.iter()
            .filter_map(|nft| nft.quality_score)
            .collect();

        let average_rarity_score = if !rarity_scores.is_empty() {
            Some(rarity_scores.iter().sum::<f64>() / rarity_scores.len() as f64)
        } else {
            None
        };

        let average_quality_score = if !quality_scores.is_empty() {
            Some(quality_scores.iter().sum::<f64>() / quality_scores.len() as f64)
        } else {
            None
        };

        let verified_count = nfts.iter().filter(|nft| nft.metadata_verified).count();
        let verification_rate = if !nfts.is_empty() {
            verified_count as f64 / nfts.len() as f64 * 100.0
        } else {
            0.0
        };

        // Calculate metadata completeness
        let mut completeness_scores = Vec::new();
        for nft in nfts {
            let mut fields_present = 0;
            let mut total_fields = 0;

            if nft.name.is_some() { fields_present += 1; }
            total_fields += 1;

            if nft.description.is_some() { fields_present += 1; }
            total_fields += 1;

            if nft.image_uri.is_some() { fields_present += 1; }
            total_fields += 1;

            if nft.external_url.is_some() { fields_present += 1; }
            total_fields += 1;

            if !nft.attributes.is_empty() { fields_present += 1; }
            total_fields += 1;

            let completeness = if total_fields > 0 {
                fields_present as f64 / total_fields as f64 * 100.0
            } else {
                0.0
            };
            completeness_scores.push(completeness);
        }

        let metadata_completeness = if !completeness_scores.is_empty() {
            completeness_scores.iter().sum::<f64>() / completeness_scores.len() as f64
        } else {
            0.0
        };

        // Calculate image availability
        let image_count = nfts.iter().filter(|nft| nft.image_uri.is_some()).count();
        let image_availability = if !nfts.is_empty() {
            image_count as f64 / nfts.len() as f64 * 100.0
        } else {
            0.0
        };

        let unique_collections = nfts.iter()
            .filter_map(|nft| nft.collection.as_ref().map(|c| c.name.clone()))
            .collect::<std::collections::HashSet<_>>()
            .len() as u32;

        // Calculate diversity score (0-100)
        let diversity_score = if nfts.len() > 0 {
            (unique_collections as f64 / nfts.len() as f64 * 100.0).min(100.0)
        } else {
            0.0
        };

        PortfolioQualityMetrics {
            average_rarity_score,
            average_quality_score,
            verification_rate,
            metadata_completeness,
            image_availability,
            unique_collections,
            diversity_score,
        }
    }

    /// Calculate Gini coefficient for value concentration
    fn calculate_gini_coefficient(&self, values: &[u64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        let n = values.len() as f64;
        let sum_values = values.iter().sum::<u64>() as f64;

        if sum_values == 0.0 {
            return 0.0;
        }

        let mut sorted_values = values.to_vec();
        sorted_values.sort_unstable();

        let mut cumulative_sum = 0.0;
        let mut sum_of_differences = 0.0;

        for (i, &value) in sorted_values.iter().enumerate() {
            cumulative_sum += value as f64;
            sum_of_differences += (2.0 * (i as f64 + 1.0) - n as f64 - 1.0) * value as f64;
        }

        let gini = sum_of_differences / (n * sum_values);
        gini.max(0.0).min(1.0)
    }

    /// Calculate overall risk score
    fn calculate_overall_risk_score(
        &self,
        counts: &HashMap<RiskLevel, u32>,
        value_by_risk: &HashMap<RiskLevel, u64>,
    ) -> f64 {
        let total_value = value_by_risk.values().sum::<u64>() as f64;
        if total_value == 0.0 {
            return 0.0;
        }

        let mut weighted_risk = 0.0;
        for (risk_level, value) in value_by_risk {
            let risk_multiplier = match risk_level {
                RiskLevel::None => 0.0,
                RiskLevel::Low => 0.25,
                RiskLevel::Medium => 0.5,
                RiskLevel::High => 0.75,
                RiskLevel::Critical => 1.0,
            };
            weighted_risk += (*value as f64 / total_value) * risk_multiplier;
        }

        weighted_risk * 100.0
    }

    /// Generate risk insights
    fn generate_risk_insights(&self, portfolio: &NftPortfolio) -> NftResult<Vec<PortfolioInsight>> {
        let mut insights = Vec::new();

        // High concentration risk
        if portfolio.value_distribution.concentration > 0.7 {
            insights.push(PortfolioInsight {
                insight_type: InsightType::Risk,
                title: "High Value Concentration".to_string(),
                description: "Your portfolio value is highly concentrated in few NFTs".to_string(),
                confidence: 0.8,
                priority: Priority::High,
                recommendation: Some("Consider diversifying your holdings to reduce concentration risk".to_string()),
                related_nfts: vec![],
                supporting_data: {
                    let mut data = HashMap::new();
                    data.insert("concentration".to_string(), serde_json::json!(portfolio.value_distribution.concentration));
                    data
                },
                generated_at: chrono::Utc::now(),
            });
        }

        // High risk NFTs
        if portfolio.high_risk_count > portfolio.total_count / 4 {
            insights.push(PortfolioInsight {
                insight_type: InsightType::Security,
                title: "High Risk Exposure".to_string(),
                description: format!("{}% of your NFTs have high or critical risk levels", 
                    (portfolio.high_risk_count as f64 / portfolio.total_count as f64 * 100.0) as u32),
                confidence: 0.9,
                priority: Priority::Critical,
                recommendation: Some("Review and consider reducing high-risk holdings".to_string()),
                related_nfts: portfolio.nfts.iter()
                    .filter(|nft| nft.security_assessment.risk_level >= RiskLevel::High)
                    .map(|nft| nft.mint_address.clone())
                    .collect(),
                supporting_data: {
                    let mut data = HashMap::new();
                    data.insert("high_risk_count".to_string(), serde_json::json!(portfolio.high_risk_count));
                    data.insert("total_count".to_string(), serde_json::json!(portfolio.total_count));
                    data
                },
                generated_at: chrono::Utc::now(),
            });
        }

        Ok(insights)
    }

    /// Generate value insights
    fn generate_value_insights(&self, portfolio: &NftPortfolio) -> NftResult<Vec<PortfolioInsight>> {
        let mut insights = Vec::new();

        // Low verification rate
        if portfolio.quality_metrics.verification_rate < 50.0 {
            insights.push(PortfolioInsight {
                insight_type: InsightType::Value,
                title: "Low Verification Rate".to_string(),
                description: format!("Only {:.1}% of your NFTs are verified", portfolio.quality_metrics.verification_rate),
                confidence: 0.8,
                priority: Priority::Medium,
                recommendation: Some("Verified NFTs generally have better liquidity and value retention".to_string()),
                related_nfts: portfolio.nfts.iter()
                    .filter(|nft| !nft.metadata_verified)
                    .map(|nft| nft.mint_address.clone())
                    .collect(),
                supporting_data: {
                    let mut data = HashMap::new();
                    data.insert("verification_rate".to_string(), serde_json::json!(portfolio.quality_metrics.verification_rate));
                    data
                },
                generated_at: chrono::Utc::now(),
            });
        }

        Ok(insights)
    }

    /// Generate diversification insights
    fn generate_diversification_insights(&self, portfolio: &NftPortfolio) -> NftResult<Vec<PortfolioInsight>> {
        let mut insights = Vec::new();

        // Low diversity
        if portfolio.quality_metrics.diversity_score < 30.0 {
            insights.push(PortfolioInsight {
                insight_type: InsightType::Diversification,
                title: "Low Collection Diversity".to_string(),
                description: format!("Your portfolio spans only {} different collections", portfolio.quality_metrics.unique_collections),
                confidence: 0.9,
                priority: Priority::Medium,
                recommendation: Some("Consider diversifying across different collections to reduce collection-specific risk".to_string()),
                related_nfts: vec![],
                supporting_data: {
                    let mut data = HashMap::new();
                    data.insert("diversity_score".to_string(), serde_json::json!(portfolio.quality_metrics.diversity_score));
                    data.insert("unique_collections".to_string(), serde_json::json!(portfolio.quality_metrics.unique_collections));
                    data
                },
                generated_at: chrono::Utc::now(),
            });
        }

        Ok(insights)
    }

    /// Generate performance insights
    fn generate_performance_insights(&self, _portfolio: &NftPortfolio) -> NftResult<Vec<PortfolioInsight>> {
        // Placeholder for performance insights
        Ok(vec![])
    }

    /// Generate trend insights
    fn generate_trend_insights(&self, _portfolio: &NftPortfolio) -> NftResult<Vec<PortfolioInsight>> {
        // Placeholder for trend insights
        Ok(vec![])
    }

    /// Generate security insights
    fn generate_security_insights(&self, portfolio: &NftPortfolio) -> NftResult<Vec<PortfolioInsight>> {
        let mut insights = Vec::new();

        // Unverified collections
        let unverified_collections: Vec<String> = portfolio.collection_breakdown.values()
            .filter(|cb| !cb.verified && cb.count > 0)
            .map(|cb| cb.collection_name.clone())
            .collect();

        if !unverified_collections.is_empty() {
            insights.push(PortfolioInsight {
                insight_type: InsightType::Security,
                title: "Unverified Collections Detected".to_string(),
                description: format!("You hold NFTs from {} unverified collections", unverified_collections.len()),
                confidence: 0.8,
                priority: Priority::Medium,
                recommendation: Some("Exercise caution with unverified collections as they may have higher risk".to_string()),
                related_nfts: portfolio.nfts.iter()
                    .filter(|nft| nft.collection.as_ref().map(|c| !c.verified).unwrap_or(false))
                    .map(|nft| nft.mint_address.clone())
                    .collect(),
                supporting_data: {
                    let mut data = HashMap::new();
                    data.insert("unverified_collections".to_string(), serde_json::json!(unverified_collections));
                    data
                },
                generated_at: chrono::Utc::now(),
            });
        }

        Ok(insights)
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> &PortfolioMetrics {
        &self.metrics
    }
}
