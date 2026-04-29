# NFT Support Implementation Plan

## Overview

This plan outlines the implementation of NFT support to enable users and developers to close empty NFT accounts, similar to the existing SOL recovery functionality for empty token accounts. The implementation will leverage the existing ultra-fast scanning architecture and add NFT-specific detection and recovery capabilities.

## Current State Analysis

The existing system provides:
- Ultra-fast wallet scanning with sub-second performance
- Empty token account detection and SOL recovery
- Batch processing capabilities
- CLI and API interfaces
- Connection pooling and caching
- Comprehensive error handling

## NFT Support Requirements

### 1. NFT Account Detection
- Identify NFT accounts (Mint Accounts, Token Accounts, Metadata Accounts)
- Detect empty/closed NFT accounts eligible for rent recovery
- Support for different NFT standards (Metaplex, Token Metadata)
- Distinguish between valuable NFTs and empty accounts

### 2. User Experience
- **Regular Users**: Simple commands to scan and recover from empty NFT accounts
- **Developers**: Advanced options for customizable NFT search and recovery
- **CLI Integration**: New commands and enhanced existing commands
- **API Support**: Public API for NFT operations

### 3. Performance Requirements
- Maintain ultra-fast scanning performance
- Efficient NFT metadata handling
- Optimized batch processing for NFT collections
- Smart caching for frequently accessed NFT data

## Technical Architecture

### 1. Core Components

#### A. NFT Scanner Module (`src/core/nft_scanner.rs`)
```rust
pub struct NftScanner {
    connection_pool: Arc<dyn ConnectionPoolTrait>,
    metadata_cache: Arc<NftMetadataCache>,
    config: NftScannerConfig,
}

pub struct NftAccount {
    pub address: String,
    pub mint: String,
    pub owner: String,
    pub account_type: NftAccountType,
    pub lamports: u64,
    pub metadata: Option<NftMetadata>,
    pub is_empty: bool,
}

pub enum NftAccountType {
    MintAccount,
    TokenAccount,
    MetadataAccount,
    MasterEditionAccount,
    CollectionAccount,
}
```

#### B. NFT Metadata Handler (`src/core/nft_metadata.rs`)
```rust
pub struct NftMetadataHandler {
    metadata_client: Arc<MetadataClient>,
    cache: Arc<NftMetadataCache>,
}

pub struct NftMetadata {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub collection: Option<CollectionInfo>,
    pub attributes: Vec<Attribute>,
    pub is_verified: bool,
}
```

#### C. Enhanced Empty Account Detection
Extend existing `EmptyAccount` struct:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmptyAccount {
    pub address: String,
    pub lamports: u64,
    pub owner: String,
    pub mint: Option<String>,
    // New NFT fields
    pub account_type: Option<AccountType>,
    pub nft_metadata: Option<NftMetadata>,
    pub is_nft_account: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountType {
    Token,
    NftMint,
    NftToken,
    NftMetadata,
    System,
    Other,
}
```

### 2. Unified Scanner Integration

#### A. Enhanced Unified Scanner
Update `src/core/unified_scanner.rs`:
```rust
impl UnifiedWalletScanner {
    pub async fn scan_wallet_with_nfts(&self, wallet_address: &str) -> Result<WalletInfo> {
        // Scan regular accounts + NFT accounts
        let token_accounts = self.scan_token_accounts(wallet_address).await?;
        let nft_accounts = self.nft_scanner.scan_nft_accounts(wallet_address).await?;
        
        // Merge and process results
        self.merge_scan_results(token_accounts, nft_accounts).await
    }
}
```

#### B. Performance Modes for NFTs
```rust
pub enum NftScanMode {
    Fast,      // Basic NFT account detection
    Thorough,  // Full metadata analysis
    Custom,    // User-defined criteria
}
```

### 3. CLI Interface Enhancements

#### A. New NFT Commands
```bash
# Scan for empty NFT accounts
solana-recover nft scan <wallet_address>

# Show NFT accounts only
solana-recover nft show <wallet_address> --type empty

# Recover from empty NFT accounts
solana-recover nft reclaim <wallet_address> --destination <address>

# Batch NFT operations
solana-recover nft batch <file_path> --mode fast
```

#### B. Enhanced Existing Commands
```bash
# Include NFTs in regular scan
solana-recover scan <wallet_address> --include-nfts

# Show NFT-specific info in dev mode
solana-recover show <targets> --dev --nft-only
```

### 4. API Interface

#### A. New NFT Endpoints
```rust
// Public API functions
pub async fn scan_wallet_nfts(wallet_address: &str, rpc_endpoint: Option<&str>) -> Result<NftScanResult>;
pub async fn get_empty_nft_accounts(wallet_address: &str, config: NftScanConfig) -> Result<Vec<EmptyNftAccount>>;
pub async fn reclaim_sol_from_nfts(request: &NftRecoveryRequest) -> Result<NftRecoveryResult>;
```

#### B. REST API Endpoints
```
GET /api/v1/wallets/{address}/nfts
GET /api/v1/wallets/{address}/nfts/empty
POST /api/v1/nfts/reclaim
GET /api/v1/nfts/collections/{id}/accounts
```

## Implementation Phases

### Phase 1: Core NFT Detection (Week 1-2)
1. **NFT Account Types Identification**
   - Implement NFT account type detection
   - Add Metaplex program ID recognition
   - Create NFT account structures

2. **Basic NFT Scanner**
   - Implement `NftScanner` with basic detection
   - Add NFT account filtering logic
   - Integrate with existing connection pool

3. **Empty NFT Account Detection**
   - Extend empty account logic for NFTs
   - Add NFT-specific rent exemption checks
   - Implement NFT account validation

### Phase 2: Metadata Integration (Week 2-3)
1. **NFT Metadata Handler**
   - Implement metadata fetching from Metaplex
   - Add metadata caching layer
   - Handle metadata parsing and validation

2. **Enhanced Detection Logic**
   - Use metadata to distinguish valuable NFTs
   - Implement collection-based filtering
   - Add attribute-based search capabilities

3. **Performance Optimization**
   - Optimize metadata fetching with batching
   - Add intelligent caching strategies
   - Implement fast-path for common NFT patterns

### Phase 3: CLI Integration (Week 3-4)
1. **New CLI Commands**
   - Add `nft` subcommand structure
   - Implement NFT-specific commands
   - Add NFT options to existing commands

2. **User Experience**
   - Add NFT-specific output formatting
   - Implement NFT recovery confirmations
   - Add progress indicators for NFT operations

3. **Developer Features**
   - Add advanced NFT search options
   - Implement collection-based operations
   - Add NFT metadata export capabilities

### Phase 4: API and Advanced Features (Week 4-5)
1. **Public API**
   - Implement NFT scanning API
   - Add NFT recovery endpoints
   - Create NFT batch processing API

2. **Advanced Filtering**
   - Implement custom NFT filters
   - Add collection-based filtering
   - Add attribute-based search

3. **Configuration and Customization**
   - Add NFT-specific configuration options
   - Implement customizable recovery strategies
   - Add NFT performance tuning options

### Phase 5: Testing and Optimization (Week 5-6)
1. **Comprehensive Testing**
   - Unit tests for NFT components
   - Integration tests for NFT workflows
   - Performance benchmarks

2. **Documentation**
   - Update API documentation
   - Add NFT usage examples
   - Create developer guides

3. **Production Readiness**
   - Error handling refinement
   - Security audit for NFT operations
   - Performance optimization

## Configuration Options

### NFT Scanner Configuration
```rust
#[derive(Debug, Clone)]
pub struct NftScannerConfig {
    pub scan_mode: NftScanMode,
    pub include_metadata: bool,
    pub max_metadata_requests: usize,
    pub cache_ttl: Duration,
    pub filter_collections: Vec<String>,
    pub min_rent_threshold: u64,
}
```

### CLI Configuration
```toml
[nft]
default_scan_mode = "fast"
include_metadata = true
max_metadata_requests = 100
cache_ttl_minutes = 30
```

## Security Considerations

### 1. NFT Validation
- Verify NFT ownership before recovery
- Validate metadata authenticity
- Check for suspicious NFT patterns

### 2. Transaction Security
- Add NFT-specific transaction validation
- Implement NFT recovery rate limiting
- Add audit logging for NFT operations

### 3. Metadata Security
- Validate metadata sources
- Implement metadata caching security
- Add metadata integrity checks

## Performance Optimizations

### 1. Caching Strategy
- L1 Cache: Hot NFT metadata (Moka)
- L2 Cache: Warm NFT data (DashMap)
- Intelligent eviction based on collection popularity

### 2. Batch Processing
- Batch NFT metadata requests
- Parallel NFT account scanning
- Optimized NFT collection processing

### 3. Fast Path Optimization
- Common NFT pattern recognition
- Preloaded popular collection metadata
- Optimized for empty NFT detection

## Success Metrics

### Performance Targets
- **NFT Scan Speed**: < 2 seconds for typical wallets
- **Metadata Fetch**: < 500ms per NFT
- **Batch Processing**: 50+ NFTs/second
- **Memory Usage**: < 100MB for 1000 NFTs

### User Experience Goals
- **CLI Simplicity**: One-command NFT recovery
- **Developer Power**: Advanced customization options
- **API Performance**: Sub-second NFT scan responses
- **Error Handling**: Clear NFT-specific error messages

## Backward Compatibility

### 1. Existing Functionality
- All current SOL recovery features preserved
- Existing CLI commands unchanged
- API backward compatibility maintained

### 2. Gradual Rollout
- Feature flags for NFT functionality
- Optional NFT inclusion in scans
- Configurable NFT detection levels

## Future Enhancements

### 1. Advanced NFT Features
- NFT portfolio analysis
- NFT valuation integration
- NFT marketplace integration

### 2. Multi-Chain Support
- Ethereum NFT support
- Cross-chain NFT recovery
- Multi-chain portfolio management

### 3. AI-Powered Features
- NFT value prediction
- Automated NFT categorization
- Smart NFT recovery recommendations

## Conclusion

This implementation plan provides a comprehensive approach to adding NFT support while maintaining the existing ultra-fast performance and user-friendly interface. The phased approach ensures gradual rollout with proper testing and optimization at each stage.

The key success factors are:
1. **Performance**: Maintaining sub-second scan speeds
2. **Simplicity**: Easy-to-use CLI commands for regular users
3. **Power**: Advanced options for developers
4. **Security**: Robust validation and error handling
5. **Scalability**: Efficient batch processing and caching

By leveraging the existing architecture and adding NFT-specific components, we can deliver a seamless NFT recovery experience that integrates perfectly with the current SOL recovery functionality.
