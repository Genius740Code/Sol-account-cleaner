# Solana Account Cleaner - API Documentation

## Overview

The Solana Account Cleaner provides a comprehensive REST API for wallet scanning, SOL recovery, and system monitoring. This documentation covers all available endpoints, request/response formats, and usage examples.

## Base URL

```
Production: https://api.solana-recover.com
Development: http://localhost:8080
```

## Authentication

All API requests require authentication using Bearer tokens:

```http
Authorization: Bearer <your-api-token>
```

### Getting API Token

```bash
curl -X POST https://api.solana-recover.com/auth/token \
  -H "Content-Type: application/json" \
  -d '{
    "api_key": "your-api-key",
    "organization_id": "your-org-id"
  }'
```

## Core Endpoints

### Wallet Scanning

#### Scan Single Wallet

```http
POST /api/scan/wallet
Content-Type: application/json

{
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "rpc_endpoint": "https://api.mainnet-beta.solana.com",
  "include_details": true
}
```

**Response:**
```json
{
  "id": "scan-123",
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "status": "completed",
  "result": {
    "address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "total_accounts": 25,
    "empty_accounts": 8,
    "recoverable_sol": 0.045678901,
    "recoverable_lamports": 45678901,
    "empty_account_addresses": [
      "ABC123...",
      "DEF456..."
    ],
    "scan_time_ms": 1250
  },
  "created_at": "2026-04-11T20:00:00Z",
  "completed_at": "2026-04-11T20:00:01.250Z"
}
```

#### Batch Scan Multiple Wallets

```http
POST /api/scan/batch
Content-Type: application/json

{
  "wallet_addresses": [
    "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
  ],
  "rpc_endpoint": "https://api.mainnet-beta.solana.com",
  "parallel_processing": true,
  "max_concurrent": 50
}
```

**Response:**
```json
{
  "id": "batch-456",
  "status": "processing",
  "total_wallets": 2,
  "completed_wallets": 0,
  "failed_wallets": 0,
  "total_recoverable_sol": 0.0,
  "estimated_completion_time": "2026-04-11T20:01:00Z",
  "results": [],
  "created_at": "2026-04-11T20:00:00Z"
}
```

### SOL Recovery

#### Recover SOL from Empty Accounts

```http
POST /api/recover/sol
Content-Type: application/json

{
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "destination_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "empty_accounts": [
    "ABC123...",
    "DEF456..."
  ],
  "max_fee_lamports": 10000000,
  "priority_fee_lamports": 1000000
}
```

**Response:**
```json
{
  "id": "recovery-789",
  "status": "completed",
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "destination_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "net_sol": 0.044567890,
  "total_fees_paid": 1000000,
  "transaction_signatures": [
    "5j7s8..."
  ],
  "created_at": "2026-04-11T20:00:00Z",
  "completed_at": "2026-04-11T20:00:02.500Z"
}
```

### System Monitoring

#### Get System Health

```http
GET /api/health
```

**Response:**
```json
{
  "status": "healthy",
  "version": "1.0.2",
  "uptime_seconds": 86400,
  "components": {
    "database": "healthy",
    "rpc_connections": "healthy",
    "cache": "healthy",
    "encryption": "healthy"
  },
  "timestamp": "2026-04-11T20:00:00Z"
}
```

#### Get Metrics

```http
GET /api/metrics
```

**Response:**
```json
{
  "timestamp": "2026-04-11T20:00:00Z",
  "performance": {
    "wallet_scans_per_second": 125.5,
    "average_scan_time_ms": 850.0,
    "success_rate": 0.98,
    "error_rate": 0.02
  },
  "resources": {
    "cpu_usage_percent": 45.2,
    "memory_usage_mb": 512.3,
    "active_connections": 25,
    "cache_hit_rate": 0.85
  },
  "security": {
    "authentication_failures": 3,
    "security_score": 0.95,
    "blocked_ips": {},
    "last_security_event": "2026-04-11T19:45:00Z"
  }
}
```

#### Get Detailed Metrics

```http
GET /api/metrics/detailed
```

**Response:**
```json
{
  "timestamp": "2026-04-11T20:00:00Z",
  "cache_metrics": {
    "l1_cache_hit_rate": 0.90,
    "l2_cache_hit_rate": 0.75,
    "l3_cache_hit_rate": 0.60,
    "overall_hit_rate": 0.82,
    "cache_memory_usage_mb": 150.0,
    "cache_evictions_total": 1250,
    "cache_operations_per_second": 1500.0,
    "average_cache_lookup_time_us": 25.5
  },
  "connection_pool_metrics": {
    "active_connections": 25,
    "idle_connections": 15,
    "connection_reuse_rate": 0.95,
    "average_connection_lifetime_ms": 30000.0,
    "connection_creation_rate": 2.5,
    "connection_errors": 2,
    "connection_utilization": 0.65,
    "endpoint_health_scores": {
      "https://api.mainnet-beta.solana.com": 0.98
    }
  },
  "memory_pool_metrics": {
    "pool_efficiency": 0.88,
    "memory_saved_bytes": 104857600,
    "allocation_rate": 500.0,
    "deallocation_rate": 480.0,
    "pool_hit_rate": 0.92,
    "average_allocation_time_ns": 150.0,
    "fragmentation_ratio": 0.05,
    "gc_pressure": 0.15
  }
}
```

## WebSocket API

### Real-time Updates

Connect to WebSocket for real-time scan updates:

```javascript
const ws = new WebSocket('wss://api.solana-recover.com/ws');

ws.onopen = function() {
  // Authenticate
  ws.send(JSON.stringify({
    type: 'auth',
    token: 'your-api-token'
  }));
};

ws.onmessage = function(event) {
  const data = JSON.parse(event.data);
  
  switch(data.type) {
    case 'scan_update':
      console.log('Scan progress:', data.progress);
      break;
    case 'scan_completed':
      console.log('Scan completed:', data.result);
      break;
    case 'error':
      console.error('Error:', data.error);
      break;
  }
};
```

#### WebSocket Message Types

**Authentication:**
```json
{
  "type": "auth",
  "token": "your-api-token"
}
```

**Scan Update:**
```json
{
  "type": "scan_update",
  "scan_id": "scan-123",
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "progress": 0.75,
  "current_account": 18,
  "total_accounts": 25,
  "estimated_remaining_time": 300
}
```

**Scan Completed:**
```json
{
  "type": "scan_completed",
  "scan_id": "scan-123",
  "result": {
    "total_accounts": 25,
    "empty_accounts": 8,
    "recoverable_sol": 0.045678901
  }
}
```

## Error Handling

### Error Response Format

All errors follow this format:

```json
{
  "error": {
    "code": "INVALID_WALLET_ADDRESS",
    "message": "The provided wallet address is invalid",
    "details": {
      "field": "wallet_address",
      "value": "invalid-address"
    },
    "timestamp": "2026-04-11T20:00:00Z",
    "request_id": "req-123"
  }
}
```

### Common Error Codes

| Code | Description | HTTP Status |
|------|-------------|-------------|
| `INVALID_WALLET_ADDRESS` | Invalid Solana wallet address format | 400 |
| `INSUFFICIENT_BALANCE` | Insufficient SOL for transaction fees | 400 |
| `RATE_LIMIT_EXCEEDED` | API rate limit exceeded | 429 |
| `UNAUTHORIZED` | Invalid or missing authentication token | 401 |
| `FORBIDDEN` | Access denied to requested resource | 403 |
| `WALLET_NOT_FOUND` | Wallet not found or has no accounts | 404 |
| `TRANSACTION_FAILED` | SOL recovery transaction failed | 500 |
| `RPC_ERROR` | RPC endpoint error | 502 |
| `INTERNAL_ERROR` | Internal server error | 500 |

## Rate Limiting

### Rate Limits

- **Standard Tier**: 100 requests/minute
- **Premium Tier**: 1000 requests/minute
- **Enterprise Tier**: 10000 requests/minute

### Rate Limit Headers

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1649728800
```

## Pagination

### Paginated Responses

Large responses are paginated:

```json
{
  "data": [...],
  "pagination": {
    "page": 1,
    "per_page": 50,
    "total_pages": 10,
    "total_items": 500,
    "has_next": true,
    "has_prev": false
  }
}
```

### Query Parameters

- `page`: Page number (default: 1)
- `per_page`: Items per page (default: 50, max: 1000)
- `sort`: Sort field
- `order`: Sort order (asc/desc)

## SDK Examples

### Rust SDK

```rust
use solana_recover::{SolanaRecoverClient, ScanRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SolanaRecoverClient::new("your-api-token")?;
    
    let scan_request = ScanRequest {
        wallet_address: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
        rpc_endpoint: Some("https://api.mainnet-beta.solana.com".to_string()),
        include_details: true,
    };
    
    let result = client.scan_wallet(scan_request).await?;
    println!("Recoverable SOL: {}", result.recoverable_sol);
    
    Ok(())
}
```

### Python SDK

```python
from solana_recover import SolanaRecoverClient

client = SolanaRecoverClient(api_token="your-api-token")

scan_result = client.scan_wallet(
    wallet_address="9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    rpc_endpoint="https://api.mainnet-beta.solana.com",
    include_details=True
)

print(f"Recoverable SOL: {scan_result.recoverable_sol}")
```

### JavaScript SDK

```javascript
import { SolanaRecoverClient } from 'solana-recover';

const client = new SolanaRecoverClient('your-api-token');

const scanResult = await client.scanWallet({
  walletAddress: '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM',
  rpcEndpoint: 'https://api.mainnet-beta.solana.com',
  includeDetails: true
});

console.log(`Recoverable SOL: ${scanResult.recoverableSol}`);
```

## Configuration

### Environment Variables

```bash
# API Configuration
SOLANA_RECOVER_API_URL=https://api.solana-recover.com
SOLANA_RECOVER_API_TOKEN=your-api-token

# RPC Configuration
SOLANA_RECOVER_DEFAULT_RPC=https://api.mainnet-beta.solana.com
SOLANA_RECOVER_RPC_TIMEOUT=30000

# Security Configuration
SOLANA_RECOVER_CERTIFICATE_PINNING=true
SOLANA_RECOVER_ALLOWED_ORIGINS=https://api.solana-recover.com

# Performance Configuration
SOLANA_RECOVER_MAX_CONCURRENT_SCANS=100
SOLANA_RECOVER_CACHE_SIZE=1000
SOLANA_RECOVER_METRICS_ENABLED=true
```

## Testing

### Test Environment

```bash
# Use test endpoint
curl -X POST https://test-api.solana-recover.com/scan/wallet \
  -H "Authorization: Bearer test-token" \
  -H "Content-Type: application/json" \
  -d '{
    "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
  }'
```

### Mock Data

For testing, use these known wallet addresses:

```json
{
  "test_wallets": [
    "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    "11111111111111111111111111111111112"
  ]
}
```

## Best Practices

### Performance Optimization

1. **Batch Operations**: Use batch endpoints for multiple wallets
2. **Caching**: Enable client-side caching for repeated requests
3. **Compression**: Request gzip compression for large responses
4. **WebSocket**: Use WebSocket for real-time updates

### Security Best Practices

1. **Token Security**: Store API tokens securely, rotate regularly
2. **HTTPS**: Always use HTTPS endpoints
3. **Input Validation**: Validate all inputs before sending
4. **Rate Limits**: Monitor rate limits and implement backoff

### Error Handling

1. **Retry Logic**: Implement exponential backoff for retries
2. **Error Classification**: Handle different error types appropriately
3. **Logging**: Log errors for debugging
4. **User Feedback**: Provide clear error messages to users

## Changelog

### Version 1.0.2 (Current)
- Enhanced security with comprehensive input validation
- Added detailed metrics collection
- Improved error handling and reporting
- Added WebSocket real-time updates

### Version 1.0.1
- Fixed connection pool memory leaks
- Improved batch processing performance
- Added rate limiting headers

### Version 1.0.0
- Initial release with core wallet scanning and SOL recovery
- Basic REST API endpoints
- Authentication and authorization

## Support

### Documentation
- [API Reference](https://docs.solana-recover.com/api)
- [SDK Documentation](https://docs.solana-recover.com/sdk)
- [Examples](https://github.com/solana-recover/examples)

### Support Channels
- Email: support@solana-recover.com
- Discord: https://discord.gg/solana-recover
- Status Page: https://status.solana-recover.com

### Bug Reports
- GitHub Issues: https://github.com/solana-recover/issues
- Security: security@solana-recover.com

---

**Last Updated**: April 11, 2026
**API Version**: v1.0.2
**Documentation Version**: 1.0
