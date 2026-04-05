# Architecture Documentation

This document provides a comprehensive overview of Solana Recover's architecture, including system design, component interactions, data flow, and technical implementation details.

## Table of Contents

- [System Overview](#system-overview)
- [Core Components](#core-components)
- [Data Flow](#data-flow)
- [Security Architecture](#security-architecture)
- [Performance Architecture](#performance-architecture)
- [Scalability Design](#scalability-design)
- [Technology Stack](#technology-stack)
- [Deployment Architecture](#deployment-architecture)

## System Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Client Layer                              │
├─────────────────┬─────────────────┬─────────────────┬─────────────────┤
│   Web Client   │   CLI Client   │  Mobile Apps   │  API Clients   │
└─────────┬───────┴─────────┬───────┴─────────┬───────┴─────────┬───────┘
          │                      │                      │                      │
          └──────────────────────┼──────────────────────┼──────────────────────┘
                                 │                      │
                    ┌─────────────┴─────────────┐    ┌─────────────┴─────────────┐
                    │      API Gateway        │    │   Load Balancer          │
                    │  (Authentication,       │    │  (nginx/HAProxy)        │
                    │   Rate Limiting,       │    └─────────────┬─────────────┘
                    │    TLS Termination)    │                  │
                    └─────────────┬─────────────┘                  │
                                 │                                   │
                    ┌─────────────┴─────────────┐                  │
                    │   Solana Recover API    │◄─────────────────┘
                    │  (Multiple Instances)    │
                    └─────────────┬─────────────┘
                                 │
          ┌────────────────────────┼────────────────────────┐
          │                      │                      │
┌─────────┴─────────┐  ┌─────────┴─────────┐  ┌─────────┴─────────┐
│   Database Layer   │  │    Cache Layer      │  │  External Services  │
│  (PostgreSQL)      │  │    (Redis)         │  │  (Solana RPC)     │
│                   │  │                   │  │  (Turnkey API)     │
│ - Scan Results    │  │ - Wallet Sessions   │  │                   │
│ - User Data      │  │ - Scan Cache       │  │ - Wallet Auth      │
│ - Audit Logs     │  │ - API Responses    │  │ - Transaction Sig  │
└───────────────────┘  └───────────────────┘  └───────────────────┘
```

### Key Design Principles

1. **Microservices Architecture**: Loosely coupled, independently deployable services
2. **Event-Driven**: Asynchronous communication with event sourcing
3. **Security First**: Zero-trust architecture with defense in depth
4. **Performance Optimized**: Horizontal scaling with intelligent caching
5. **Observability**: Comprehensive monitoring and tracing

## Core Components

### 1. API Server

#### Responsibilities
- HTTP/HTTPS request handling
- Authentication and authorization
- Request validation and rate limiting
- Response formatting and error handling

#### Implementation
```rust
// src/api/server.rs
pub struct ApiServer {
    scanner: Arc<WalletScanner>,
    batch_processor: Arc<BatchProcessor>,
    recovery_manager: Arc<RecoveryManager>,
    wallet_manager: Arc<WalletManager>,
    cache_manager: Arc<CacheManager>,
}

impl ApiServer {
    pub async fn start(self, config: &ServerConfig) -> Result<()> {
        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/api/v1/scan", post(scan_handler))
            .route("/api/v1/batch-scan", post(batch_scan_handler))
            .route("/api/v1/recover", post(recover_handler))
            .route("/api/v1/wallets/connect", post(connect_wallet_handler))
            .layer(AuthLayer::new())
            .layer(RateLimitLayer::new())
            .layer(TracingLayer::new());
            
        let listener = TcpListener::bind((config.host, config.port)).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}
```

#### Features
- **Async Request Handling**: Tokio-based async runtime
- **Middleware Pipeline**: Authentication, rate limiting, CORS, logging
- **Graceful Shutdown**: Clean connection termination
- **Health Checks**: Comprehensive health monitoring

### 2. Wallet Scanner

#### Responsibilities
- Single wallet scanning
- Token account enumeration
- Empty account detection
- Balance calculation

#### Implementation
```rust
// src/core/scanner.rs
pub struct WalletScanner {
    connection_pool: Arc<dyn ConnectionPoolTrait>,
    cache_manager: Option<Arc<CacheManager>>,
}

impl WalletScanner {
    pub async fn scan_wallet(&self, address: &str) -> Result<ScanResult> {
        // Check cache first
        if let Some(cache) = &self.cache_manager {
            if let Some(cached) = cache.get(address).await? {
                return Ok(cached);
            }
        }
        
        // Perform scan
        let result = self.scan_wallet_internal(address).await?;
        
        // Cache result
        if let Some(cache) = &self.cache_manager {
            cache.set(address, &result, CACHE_TTL).await?;
        }
        
        Ok(result)
    }
    
    async fn scan_wallet_internal(&self, address: &str) -> Result<WalletInfo> {
        let pubkey = Pubkey::from_str(address)?;
        let client = self.connection_pool.get_client().await?;
        
        // Get all token accounts
        let token_accounts = client.get_token_accounts(&pubkey).await?;
        
        // Process accounts for empty ones
        let mut empty_accounts = Vec::new();
        let mut total_recoverable = 0u64;
        
        for account in token_accounts {
            if let Some(empty) = self.check_empty_account(&account).await? {
                total_recoverable += empty.lamports;
                empty_accounts.push(empty);
            }
        }
        
        Ok(WalletInfo {
            address: address.to_string(),
            pubkey,
            total_accounts: token_accounts.len() as u64,
            empty_accounts: empty_accounts.len() as u64,
            recoverable_lamports: total_recoverable,
            recoverable_sol: total_recoverable as f64 / LAMPORTS_PER_SOL,
            empty_account_addresses: empty_accounts.iter().map(|a| a.address.clone()).collect(),
            scan_time_ms: 0, // Set by caller
        })
    }
}
```

### 3. Batch Processor

#### Responsibilities
- Concurrent wallet scanning
- Queue management
- Progress tracking
- Result aggregation

#### Implementation
```rust
// src/core/processor.rs
pub struct BatchProcessor {
    scanner: Arc<WalletScanner>,
    cache_manager: Option<Arc<CacheManager>>,
    persistence_manager: Option<Arc<PersistenceManager>>,
    config: BatchProcessorConfig,
}

impl BatchProcessor {
    pub async fn process_batch(&self, request: &BatchScanRequest) -> Result<BatchScanResult> {
        let start_time = Instant::now();
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_wallets));
        
        // Process wallets concurrently
        let futures: Vec<_> = request.wallet_addresses
            .iter()
            .map(|address| {
                let scanner = self.scanner.clone();
                let semaphore = semaphore.clone();
                
                async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    scanner.scan_wallet(address).await
                }
            })
            .collect();
        
        let results = futures::future::join_all(futures).await;
        
        // Aggregate results
        let (successful, failed): (Vec<_>, Vec<_>) = results
            .into_iter()
            .partition(Result::is_ok);
            
        let successful_results: Vec<_> = successful
            .into_iter()
            .map(Result::unwrap)
            .collect();
            
        let total_recoverable: f64 = successful_results
            .iter()
            .filter_map(|r| r.result.as_ref())
            .map(|w| w.recoverable_sol)
            .sum();
        
        Ok(BatchScanResult {
            batch_id: request.id,
            total_wallets: request.wallet_addresses.len() as u64,
            successful_scans: successful_results.len() as u64,
            failed_scans: failed.len() as u64,
            total_recoverable_sol,
            results: successful_results,
            errors: failed.into_iter().map(Result::unwrap_err).collect(),
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }
}
```

### 4. Recovery Manager

#### Responsibilities
- Transaction building
- Wallet signing
- Transaction submission
- Confirmation tracking

#### Implementation
```rust
// src/core/recovery.rs
pub struct RecoveryManager {
    connection_pool: Arc<dyn ConnectionPoolTrait>,
    wallet_manager: Arc<WalletManager>,
    config: RecoveryConfig,
}

impl RecoveryManager {
    pub async fn recover_sol(&self, request: &RecoveryRequest) -> Result<RecoveryResult> {
        // Validate request
        self.validate_recovery_request(request).await?;
        
        // Get wallet connection
        let wallet_connection = self.wallet_manager.get_connection(&request.wallet_connection_id).await?;
        
        // Build recovery transactions
        let transactions = self.build_recovery_transactions(request).await?;
        
        // Sign and submit transactions
        let mut submitted_transactions = Vec::new();
        let mut total_recovered = 0u64;
        let mut total_fees = 0u64;
        
        for transaction in transactions {
            let signed_tx = self.wallet_manager.sign_transaction(
                &wallet_connection, 
                &transaction
            ).await?;
            
            let signature = self.connection_pool.get_client().await?
                .send_transaction(&signed_tx).await?;
                
            // Wait for confirmation
            let confirmed_tx = self.wait_for_confirmation(&signature).await?;
            
            total_recovered += confirmed_tx.lamports_recovered;
            total_fees += confirmed_tx.fee_paid;
            submitted_transactions.push(confirmed_tx);
        }
        
        Ok(RecoveryResult {
            id: request.id,
            wallet_address: request.wallet_address.clone(),
            status: RecoveryStatus::Completed,
            total_accounts_recovered: request.empty_accounts.len() as u64,
            total_lamports_recovered: total_recovered,
            total_fees_paid: total_fees,
            net_lamports: total_recovered - total_fees,
            net_sol: (total_recovered - total_fees) as f64 / LAMPORTS_PER_SOL,
            transactions: submitted_transactions,
            created_at: request.created_at,
            completed_at: Some(Utc::now()),
            duration_ms: 0, // Set by caller
        })
    }
}
```

### 5. Wallet Manager

#### Responsibilities
- Wallet provider abstraction
- Connection management
- Transaction signing
- Provider-specific logic

#### Implementation
```rust
// src/wallet/manager.rs
pub struct WalletManager {
    providers: HashMap<WalletType, Box<dyn WalletProvider>>,
    connections: Arc<DashMap<String, WalletConnection>>,
}

impl WalletManager {
    pub fn new() -> Self {
        let mut providers: HashMap<WalletType, Box<dyn WalletProvider>> = HashMap::new();
        
        providers.insert(WalletType::Turnkey, Box::new(TurnkeyProvider::new()));
        providers.insert(WalletType::Phantom, Box::new(PhantomProvider::new()));
        providers.insert(WalletType::Solflare, Box::new(SolflareProvider::new()));
        
        Self {
            providers,
            connections: Arc::new(DashMap::new()),
        }
    }
    
    pub async fn connect_wallet(&self, credentials: &WalletCredentials) -> Result<WalletConnection> {
        let provider = self.providers.get(&credentials.wallet_type)
            .ok_or_else(|| SolanaRecoverError::UnsupportedWalletType(credentials.wallet_type.clone()))?;
            
        let connection = provider.connect(credentials).await?;
        
        // Store connection
        self.connections.insert(connection.id.clone(), connection.clone());
        
        Ok(connection)
    }
    
    pub async fn sign_transaction(
        &self, 
        connection_id: &str, 
        transaction: &[u8]
    ) -> Result<Vec<u8>> {
        let connection = self.connections.get(connection_id)
            .ok_or_else(|| SolanaRecoverError::WalletConnectionNotFound(connection_id.to_string()))?;
            
        let provider = self.providers.get(&connection.wallet_type)
            .ok_or_else(|| SolanaRecoverError::UnsupportedWalletType(connection.wallet_type.clone()))?;
            
        provider.sign_transaction(connection, transaction).await
    }
}
```

### 6. Connection Pool

#### Responsibilities
- RPC endpoint management
- Connection lifecycle
- Load balancing
- Health checking

#### Implementation
```rust
// src/rpc/pool.rs
pub struct ConnectionPool {
    endpoints: Vec<RpcEndpoint>,
    connections: Arc<Mutex<Vec<RpcClient>>>,
    semaphore: Arc<Semaphore>,
    health_checker: Arc<HealthChecker>,
}

impl ConnectionPool {
    pub async fn get_client(&self) -> Result<Arc<RpcClient>> {
        let _permit = self.semaphore.acquire().await?;
        
        // Get healthy endpoint
        let endpoint = self.health_checker.get_healthy_endpoint().await?;
        
        // Get or create connection
        let mut connections = self.connections.lock().await;
        let client = if let Some(existing) = connections.iter().find(|c| c.endpoint_id == endpoint.id) {
            Arc::clone(existing)
        } else {
            let new_client = Arc::new(RpcClient::new(&endpoint.url).await?);
            connections.push(new_client.clone());
            new_client
        };
        
        Ok(client)
    }
}
```

## Data Flow

### Wallet Scan Flow

```
Client Request → API Gateway → Authentication → Rate Limiting → Scanner
     ↓
Cache Check → RPC Call → Token Account Fetch → Empty Account Detection
     ↓
Result Processing → Cache Storage → Response Formatting → Client Response
```

### SOL Recovery Flow

```
Client Request → Validation → Wallet Connection → Transaction Building
     ↓
Transaction Signing → RPC Submission → Confirmation Waiting
     ↓
Result Aggregation → Audit Logging → Response Formatting → Client Response
```

## Security Architecture

### Defense in Depth

1. **Network Security**
   - TLS 1.3 encryption
   - Certificate pinning
   - DDoS protection

2. **Application Security**
   - JWT authentication
   - API key management
   - Input validation
   - Rate limiting

3. **Data Security**
   - Encrypted credentials
   - Audit logging
   - Data minimization
   - Secure key storage

### Authentication Flow

```
Client Request → Extract Credentials → Validate JWT/API Key → Check Permissions
     ↓
Rate Limit Check → Request Processing → Audit Logging → Response
```

### Wallet Security

```
Wallet Connection → Provider Authentication → Secure Session → Transaction Signing
     ↓
Signature Verification → Audit Trail → Secure Storage → Response
```

## Performance Architecture

### Caching Strategy

1. **Multi-Level Cache**
   - L1: In-memory cache (Moka)
   - L2: Distributed cache (Redis)
   - L3: Database persistence

2. **Cache Policies**
   - TTL-based expiration
   - LRU eviction
   - Write-through updates
   - Cache warming

### Connection Management

1. **Connection Pooling**
   - Pre-warmed connections
   - Health monitoring
   - Automatic failover
   - Load balancing

2. **Concurrency Control**
   - Semaphore-based limits
   - Work-stealing queues
   - Backpressure handling

### Performance Optimizations

1. **Database Optimizations**
   - Connection pooling
   - Query optimization
   - Indexing strategy
   - Partitioning

2. **Network Optimizations**
   - HTTP/2 support
   - Connection reuse
   - Request batching
   - Compression

## Scalability Design

### Horizontal Scaling

1. **Stateless Services**
   - Session externalization
   - Shared cache
   - Load balancing
   - Auto-scaling

2. **Database Scaling**
   - Read replicas
   - Connection pooling
   - Sharding strategy
   - Caching layers

### Resource Management

1. **Memory Management**
   - Object pooling
   - Garbage collection
   - Memory monitoring
   - Leak detection

2. **CPU Management**
   - Async processing
   - Thread pool tuning
   - Work distribution
   - CPU affinity

## Technology Stack

### Core Technologies

- **Language**: Rust (1.70+)
- **Runtime**: Tokio async runtime
- **Web Framework**: Axum
- **Database**: PostgreSQL with SQLx
- **Cache**: Redis
- **Message Queue**: In-memory with flume

### Dependencies

```toml
# Core
tokio = { version = "1.0", features = ["full"] }
axum = "0.7"
serde = { version = "1.0", features = ["derive"] }

# Database
sqlx = { version = "0.7", features = ["postgres", "chrono", "uuid"] }
redis = { version = "0.26", features = ["tokio-comp"] }

# Solana
solana-sdk = "1.18"
solana-client = "1.18"
spl-token = "4.0"

# Performance
rayon = "1.8"
dashmap = "5.5"
moka = { version = "0.12", features = ["future"] }

# Security
jsonwebtoken = "9.0"
sha2 = "0.10"
hmac = "0.12"

# Monitoring
tracing = "0.1"
metrics = "0.21"
prometheus = "0.13"
```

### Infrastructure

- **Containerization**: Docker
- **Orchestration**: Kubernetes
- **Load Balancer**: nginx/HAProxy
- **Monitoring**: Prometheus + Grafana
- **Logging**: ELK Stack or Loki

## Deployment Architecture

### Production Deployment

```
Internet → CDN → Load Balancer → API Gateway → API Servers
                                    ↓
                               Cache Layer (Redis)
                                    ↓
                            Database Cluster (PostgreSQL)
                                    ↓
                    External Services (Solana RPC, Turnkey)
```

### High Availability

1. **Redundancy**
   - Multi-AZ deployment
   - Database replication
   - Cache clustering
   - Health monitoring

2. **Failover**
   - Automatic failover
   - Graceful degradation
   - Circuit breakers
   - Recovery procedures

### Disaster Recovery

1. **Backup Strategy**
   - Database backups
   - Configuration backups
   - Off-site storage
   - Recovery testing

2. **Business Continuity**
   - Multi-region deployment
   - Traffic routing
   - Data synchronization
   - Emergency procedures

---

This architecture documentation provides a comprehensive overview of Solana Recover's technical design. For implementation details, refer to the specific component documentation and source code.
