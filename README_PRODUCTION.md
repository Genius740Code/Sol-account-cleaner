# Solana Recover - Production-Ready High-Performance Wallet Scanner

A scalable, secure, and high-performance Solana wallet scanner designed to handle 10,000+ daily users with enterprise-grade features.

## 🚀 Production Features

### Performance & Scalability
- **Optimized RPC Connection Pooling**: Circuit breakers, load balancing, and health checks
- **Redis Caching**: Distributed caching for wallet scan results and API responses
- **Async Batch Processing**: Concurrent wallet scanning with configurable limits
- **Database Connection Pooling**: SQLx-based optimized database connections
- **Memory Optimization**: Object pooling, leak detection, and automatic GC

### Security & Reliability
- **Rate Limiting**: Per-IP and per-API-key rate limiting with circuit breakers
- **Authentication**: JWT-based authentication with API key support
- **Error Handling**: Comprehensive retry mechanisms with exponential backoff
- **Input Validation**: Request size limits and input sanitization
- **Security Headers**: CORS, XSS protection, and secure headers

### Monitoring & Observability
- **Prometheus Metrics**: Comprehensive metrics collection and export
- **System Monitoring**: CPU, memory, disk, and network monitoring
- **Error Tracking**: Detailed error classification and reporting
- **Health Checks**: Multiple health check endpoints
- **Structured Logging**: JSON logging with tracing support

## 📋 System Requirements

### Minimum Requirements
- **CPU**: 4 cores
- **Memory**: 8GB RAM
- **Storage**: 50GB SSD
- **Network**: 100 Mbps

### Recommended Production Requirements
- **CPU**: 8+ cores
- **Memory**: 16GB+ RAM
- **Storage**: 100GB+ SSD
- **Network**: 1 Gbps
- **Redis**: 4GB+ memory
- **Load Balancer**: Nginx/HAProxy

## 🛠️ Installation

### 1. Build from Source

```bash
# Clone the repository
git clone https://github.com/your-org/solana-recover.git
cd solana-recover

# Build optimized binary
cargo build --release --bin main_optimized

# The binary will be at: target/release/main_optimized.exe
```

### 2. Dependencies

#### Redis (Required for production)
```bash
# Ubuntu/Debian
sudo apt-get install redis-server

# CentOS/RHEL
sudo yum install redis

# macOS
brew install redis

# Start Redis
sudo systemctl start redis
```

#### Environment Variables
```bash
# Required for production
export REDIS_URL="redis://localhost:6379"
export JWT_SECRET="your-super-secret-jwt-key-change-this"
export ENABLE_AUTH="true"
export ENABLE_REDIS="true"

# Optional
export DATABASE_URL="sqlite:./solana_recover.db"
export LOG_LEVEL="info"
export RUST_LOG="info"
```

## ⚙️ Configuration

### Production Configuration File

Create `config/production.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080
workers = 8

[rpc]
endpoints = [
    "https://api.mainnet-beta.solana.com",
    "https://solana-api.projectserum.com",
    "https://rpc.ankr.com/solana"
]
pool_size = 20
timeout_ms = 5000
rate_limit_rps = 100

[scanner]
batch_size = 500
max_concurrent_wallets = 1000
retry_attempts = 5
retry_delay_ms = 1000

[cache]
ttl_seconds = 300
max_size = 50000
cleanup_interval_seconds = 60

[database]
database_url = "sqlite:./solana_recover.db"
max_connections = 20

[logging]
level = "info"
format = "json"
file_path = "/var/log/solana-recover/app.log"
```

### Environment-Specific Configuration

```bash
# Development
./target/release/main_optimized --config config/development.toml

# Production
./target/release/main_optimized --production --config config/production.toml

# Custom configuration
./target/release/main_optimized --config config/custom.toml
```

## 🚀 Deployment

### 1. Systemd Service (Linux)

Create `/etc/systemd/system/solana-recover.service`:

```ini
[Unit]
Description=Solana Recover API
After=network.target redis.service

[Service]
Type=simple
User=solana-recover
Group=solana-recover
WorkingDirectory=/opt/solana-recover
ExecStart=/opt/solana-recover/target/release/main_optimized --production --server
Restart=always
RestartSec=5
Environment=REDIS_URL=redis://localhost:6379
Environment=JWT_SECRET=your-super-secret-jwt-key
Environment=ENABLE_AUTH=true
Environment=ENABLE_REDIS=true
Environment=LOG_LEVEL=info

[Install]
WantedBy=multi-user.target
```

```bash
# Enable and start service
sudo systemctl enable solana-recover
sudo systemctl start solana-recover

# Check status
sudo systemctl status solana-recover
```

### 2. Docker Deployment

Create `Dockerfile`:

```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin main_optimized

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false solana-recover

WORKDIR /app
COPY --from=builder /app/target/release/main_optimized .
COPY --from=builder /app/config ./config

# Set permissions
RUN chown -R solana-recover:solana-recover /app

USER solana-recover

EXPOSE 8080 9091

CMD ["./main_optimized", "--production", "--server"]
```

### 3. Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  solana-recover:
    build: .
    ports:
      - "8080:8080"
      - "9091:9091"
    environment:
      - REDIS_URL=redis://redis:6379
      - JWT_SECRET=your-super-secret-jwt-key
      - ENABLE_AUTH=true
      - ENABLE_REDIS=true
      - LOG_LEVEL=info
    depends_on:
      - redis
    restart: unless-stopped
    volumes:
      - ./data:/app/data
      - ./logs:/app/logs

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    restart: unless-stopped
    command: redis-server --appendonly yes --maxmemory 2gb --maxmemory-policy allkeys-lru

  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
      - ./ssl:/etc/nginx/ssl
    depends_on:
      - solana-recover
    restart: unless-stopped

volumes:
  redis_data:
```

## 📊 API Endpoints

### Core API
- `GET /health` - Basic health check
- `GET /health/ping` - Ping endpoint
- `GET /` - API information and endpoints
- `POST /api/v1/scan` - Scan single wallet
- `POST /api/v1/batch-scan` - Batch scan wallets
- `POST /api/v1/recover` - Recover SOL
- `POST /api/v1/estimate-fees` - Estimate recovery fees
- `GET /api/v1/recovery/{id}` - Get recovery status

### Monitoring
- `GET /metrics` - Application metrics (JSON)
- `GET /metrics/prometheus` - Prometheus metrics
- `GET /status` - System status and health

### Example API Usage

```bash
# Health check
curl http://localhost:8080/health

# Scan wallet
curl -X POST http://localhost:8080/api/v1/scan \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{"wallet_address": "11111111111111111111111111111111"}'

# Batch scan
curl -X POST http://localhost:8080/api/v1/batch-scan \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "wallet_addresses": [
      "11111111111111111111111111111111",
      "22222222222222222222222222222222"
    ]
  }'

# Get metrics
curl http://localhost:8080/metrics
```

## 🔧 Performance Tuning

### RPC Connection Pool
```toml
[rpc]
pool_size = 20                    # Increase for high load
timeout_ms = 5000                # Adjust based on network
rate_limit_rps = 100             # Per endpoint rate limit
```

### Batch Processing
```toml
[scanner]
max_concurrent_wallets = 1000    # Concurrent wallet scans
batch_size = 500                 # Batch processing size
retry_attempts = 5               # Retry attempts
```

### Memory Optimization
```bash
# Set memory limits
export RUST_BACKTRACE=1
export RUST_LOG=info
```

### Database Optimization
```sql
-- SQLite optimizations
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = 10000;
PRAGMA temp_store = MEMORY;
```

## 📈 Monitoring & Alerting

### Prometheus Metrics

Key metrics to monitor:
- `http_requests_total` - Total HTTP requests
- `http_request_duration_ms` - Request duration
- `active_users_total` - Active users
- `wallets_scanned_total` - Wallets scanned
- `sol_recovered_total` - SOL recovered
- `cache_hit_rate` - Cache hit rate
- `database_connections_active` - DB connections
- `system_cpu_usage_percent` - CPU usage
- `system_memory_usage_mb` - Memory usage

### Grafana Dashboard

Create a Grafana dashboard with panels for:
- Request rate and response time
- Error rate and status codes
- Active users and throughput
- Resource utilization (CPU, memory, disk)
- Cache performance
- Database performance

### Alerting Rules

Example Prometheus alerting rules:

```yaml
groups:
- name: solana-recover
  rules:
  - alert: HighErrorRate
    expr: rate(http_requests_error_total[5m]) / rate(http_requests_total[5m]) > 0.05
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "High error rate detected"
      
  - alert: HighMemoryUsage
    expr: system_memory_usage_mb / system_memory_total_mb > 0.85
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "High memory usage detected"
      
  - alert: DatabaseConnectionPoolExhausted
    expr: database_connections_active / database_connections_max > 0.9
    for: 1m
    labels:
      severity: warning
    annotations:
      summary: "Database connection pool nearly exhausted"
```

## 🔒 Security Best Practices

### Authentication
- Use strong JWT secrets (256-bit minimum)
- Rotate API keys regularly
- Implement IP whitelisting where appropriate
- Use HTTPS in production

### Rate Limiting
- Configure per-IP rate limits
- Implement per-API-key limits
- Use circuit breakers for external services
- Monitor for abuse patterns

### Input Validation
- Validate all input parameters
- Sanitize user inputs
- Implement request size limits
- Use parameterized queries for database

### Infrastructure Security
- Run behind reverse proxy (nginx/HAProxy)
- Implement firewall rules
- Use TLS 1.3 for encryption
- Regular security updates

## 🚨 Troubleshooting

### Common Issues

#### High Memory Usage
```bash
# Check memory usage
curl http://localhost:8080/metrics | grep memory

# Force garbage collection
curl -X POST http://localhost:8080/admin/optimize-memory

# Check for memory leaks
curl http://localhost:8080/admin/memory-leaks
```

#### Database Performance
```bash
# Check database metrics
curl http://localhost:8080/metrics | grep database

# Run database vacuum
curl -X POST http://localhost:8080/admin/vacuum-database
```

#### RPC Issues
```bash
# Check RPC endpoint health
curl http://localhost:8080/metrics | grep rpc

# Check circuit breaker status
curl http://localhost:8080/admin/circuit-breakers
```

### Log Analysis
```bash
# View application logs
tail -f /var/log/solana-recover/app.log

# Filter error logs
grep "ERROR" /var/log/solana-recover/app.log

# Monitor specific endpoints
grep "POST /api/v1/scan" /var/log/solana-recover/app.log
```

## 📚 API Documentation

### Authentication
All API endpoints require authentication unless explicitly disabled:

```bash
# Using API Key
curl -H "X-API-Key: your-api-key" ...

# Using JWT Token
curl -H "Authorization: Bearer your-jwt-token" ...
```

### Rate Limits
- Default: 100 requests per minute per IP
- API keys: Custom limits based on subscription
- Burst: Up to 10 requests per second

### Error Responses
```json
{
  "success": false,
  "error": "Rate limit exceeded",
  "timestamp": "2024-01-01T12:00:00Z"
}
```

## 🔄 Scaling

### Horizontal Scaling
- Deploy multiple instances behind load balancer
- Use Redis for shared caching
- Use external database (PostgreSQL) for persistence
- Implement session affinity if needed

### Vertical Scaling
- Increase CPU cores for better concurrency
- Add memory for larger cache sizes
- Use faster storage (NVMe SSDs)
- Optimize database configuration

### Database Scaling
- Read replicas for read-heavy workloads
- Connection pooling optimization
- Query optimization and indexing
- Consider PostgreSQL for higher write loads

## 📞 Support

For production support:
- Monitor system health metrics
- Set up alerting for critical issues
- Keep regular backups of database
- Document custom configurations
- Test disaster recovery procedures

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 🤝 Contributing

Please read CONTRIBUTING.md for details on our code of conduct and the process for submitting pull requests.

---

**Note**: This is a production-ready system designed for high-load environments. Ensure proper testing, monitoring, and backup strategies before deploying to production.
