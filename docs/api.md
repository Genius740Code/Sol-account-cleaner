# Solana Recover API Documentation

This document provides comprehensive documentation for the Solana Recover REST API, including endpoints, request/response formats, authentication, and usage examples.

## Table of Contents

- [Base URL](#base-url)
- [Authentication](#authentication)
- [API Overview](#api-overview)
- [Endpoints](#endpoints)
- [Error Handling](#error-handling)
- [Rate Limiting](#rate-limiting)
- [SDKs and Libraries](#sdks-and-libraries)
- [Examples](#examples)

## Base URL

```
Production: https://api.solana-recover.com
Development: http://localhost:8080
```

## Authentication

### API Key Authentication

For production use, API keys are required:

```bash
curl -H "X-API-Key: your-api-key" \
     -H "Content-Type: application/json" \
     https://api.solana-recover.com/api/v1/scan
```

### JWT Authentication (Enterprise)

Enterprise users can use JWT tokens:

```bash
curl -H "Authorization: Bearer your-jwt-token" \
     -H "Content-Type: application/json" \
     https://api.solana-recover.com/api/v1/scan
```

### Getting API Keys

1. Sign up at [https://dashboard.solana-recover.com](https://dashboard.solana-recover.com)
2. Create an API key in your dashboard
3. Copy the key and keep it secure
4. Use the key in API requests

## API Overview

The Solana Recover API provides the following main capabilities:

- **Single Wallet Scanning**: Scan individual wallets for recoverable SOL
- **Batch Processing**: Scan multiple wallets efficiently
- **Health Checks**: Monitor API status and performance
- **Metrics**: Retrieve usage and performance metrics
- **Webhooks**: Receive notifications for scan completion

### API Versioning

The API uses URL versioning: `/api/v1/`, `/api/v2/`, etc.

Current version: `v1`

### Content Types

All requests and responses use JSON:

```
Content-Type: application/json
```

### Date Format

All timestamps use ISO 8601 format:

```json
{
  "created_at": "2024-01-15T10:30:00Z",
  "completed_at": "2024-01-15T10:31:25Z"
}
```

## Endpoints

### Health Check

#### GET /health

Check if the API service is healthy and operational.

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "timestamp": "2024-01-15T10:30:00Z",
  "services": {
    "database": "healthy",
    "rpc": "healthy",
    "scanner": "healthy"
  },
  "metrics": {
    "uptime_seconds": 86400,
    "active_scans": 12,
    "total_scans": 150000
  }
}
```

**Status Codes:**
- `200 OK`: Service is healthy
- `503 Service Unavailable`: Service is down

---

### Single Wallet Scan

#### POST /api/v1/scan

Scan a single wallet for recoverable SOL.

**Request Body:**
```json
{
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "fee_percentage": 0.15,
  "include_empty_addresses": true,
  "timeout_seconds": 30
}
```

**Parameters:**
- `wallet_address` (string, required): Solana wallet public key
- `fee_percentage` (float, optional): Fee percentage (0.0-1.0, default: 0.15)
- `include_empty_addresses` (boolean, optional): Include empty account addresses (default: true)
- `timeout_seconds` (integer, optional): Request timeout (default: 30)

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "status": "completed",
  "created_at": "2024-01-15T10:30:00Z",
  "completed_at": "2024-01-15T10:31:25Z",
  "result": {
    "address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "total_accounts": 25,
    "empty_accounts": 8,
    "recoverable_lamports": 2039280,
    "recoverable_sol": 0.00203928,
    "fee_percentage": 0.15,
    "fee_lamports": 305892,
    "net_recovery_lamports": 1733388,
    "net_recovery_sol": 0.00173339,
    "empty_account_addresses": [
      "AbCdEfGhIjKlMnOpQrStUvWxYz1234567890abcdef",
      "BcDeFgHiJkLmNoPqRsTuVwXyZ2345678901bcdef"
    ],
    "scan_time_ms": 1250
  }
}
```

**Status Codes:**
- `200 OK`: Scan completed successfully
- `400 Bad Request`: Invalid wallet address or parameters
- `401 Unauthorized`: Invalid API key
- `429 Too Many Requests`: Rate limit exceeded
- `500 Internal Server Error`: Server error
- `503 Service Unavailable`: Service temporarily unavailable

---

### Batch Wallet Scan

#### POST /api/v1/batch-scan

Scan multiple wallets for recoverable SOL.

**Request Body:**
```json
{
  "wallet_addresses": [
    "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"
  ],
  "fee_percentage": 0.15,
  "include_empty_addresses": true,
  "timeout_seconds": 60,
  "max_concurrent": 10
}
```

**Parameters:**
- `wallet_addresses` (array, required): Array of wallet addresses (max 1000)
- `fee_percentage` (float, optional): Fee percentage (default: 0.15)
- `include_empty_addresses` (boolean, optional): Include empty account addresses
- `timeout_seconds` (integer, optional): Timeout per wallet (default: 60)
- `max_concurrent` (integer, optional): Maximum concurrent scans (default: 10)

**Response:**
```json
{
  "id": "batch-550e8400-e29b-41d4-a716-446655440000",
  "status": "completed",
  "created_at": "2024-01-15T10:30:00Z",
  "completed_at": "2024-01-15T10:32:15Z",
  "total_wallets": 3,
  "successful_scans": 3,
  "failed_scans": 0,
  "total_recoverable_sol": 0.00512345,
  "total_fees_sol": 0.00076852,
  "total_net_recovery_sol": 0.00435493,
  "results": [
    {
      "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
      "status": "completed",
      "result": {
        "address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
        "total_accounts": 25,
        "empty_accounts": 8,
        "recoverable_sol": 0.00203928,
        "fee_sol": 0.00030589,
        "net_recovery_sol": 0.00173339,
        "scan_time_ms": 1250
      }
    },
    {
      "wallet_address": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      "status": "completed",
      "result": {
        "address": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "total_accounts": 12,
        "empty_accounts": 3,
        "recoverable_sol": 0.00152341,
        "fee_sol": 0.00022851,
        "net_recovery_sol": 0.00129490,
        "scan_time_ms": 980
      }
    }
  ],
  "errors": []
}
```

**Status Codes:**
- `200 OK`: Batch scan completed
- `400 Bad Request`: Invalid request parameters
- `401 Unauthorized`: Invalid API key
- `413 Payload Too Large`: Too many wallet addresses
- `429 Too Many Requests`: Rate limit exceeded

---

### Scan Status

#### GET /api/v1/scan/{scan_id}

Get the status of a previous scan.

**Parameters:**
- `scan_id` (string, required): Scan ID returned from scan endpoint

**Response:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "status": "completed",
  "created_at": "2024-01-15T10:30:00Z",
  "completed_at": "2024-01-15T10:31:25Z",
  "progress": {
    "percentage": 100,
    "current_step": "completed",
    "estimated_remaining_seconds": 0
  },
  "result": {
    // Same as single scan result
  }
}
```

**Status Values:**
- `pending`: Scan is queued
- `running`: Scan is in progress
- `completed`: Scan finished successfully
- `failed`: Scan failed
- `cancelled`: Scan was cancelled

---

### Metrics

#### GET /api/v1/metrics

Get API usage and performance metrics.

**Response:**
```json
{
  "period": "24h",
  "scans": {
    "total": 15000,
    "successful": 14850,
    "failed": 150,
    "success_rate": 0.99
  },
  "performance": {
    "avg_scan_time_ms": 1200,
    "p95_scan_time_ms": 2500,
    "p99_scan_time_ms": 5000
  },
  "recovery": {
    "total_recoverable_sol": 125.45,
    "total_fees_sol": 18.82,
    "total_net_recovery_sol": 106.63
  },
  "rate_limits": {
    "requests_per_hour": 800,
    "limit": 1000,
    "reset_time": "2024-01-15T11:00:00Z"
  }
}
```

---

### Webhooks

#### POST /api/v1/webhooks

Register a webhook to receive scan completion notifications.

**Request Body:**
```json
{
  "url": "https://your-app.com/webhook/solana-recover",
  "events": ["scan.completed", "batch.completed"],
  "secret": "your-webhook-secret"
}
```

**Response:**
```json
{
  "id": "webhook-123",
  "url": "https://your-app.com/webhook/solana-recover",
  "events": ["scan.completed", "batch.completed"],
  "active": true,
  "created_at": "2024-01-15T10:30:00Z"
}
```

**Webhook Payload:**
```json
{
  "event": "scan.completed",
  "data": {
    "scan_id": "550e8400-e29b-41d4-a716-446655440000",
    "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "status": "completed",
    "result": {
      // Scan result data
    }
  },
  "timestamp": "2024-01-15T10:31:25Z"
}
```

## Error Handling

### Error Response Format

All errors return a consistent format:

```json
{
  "error": {
    "code": "INVALID_WALLET_ADDRESS",
    "message": "The provided wallet address is invalid",
    "details": {
      "field": "wallet_address",
      "value": "invalid-address"
    },
    "request_id": "req-123456789"
  }
}
```

### Common Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `INVALID_WALLET_ADDRESS` | 400 | Wallet address format is invalid |
| `RATE_LIMIT_EXCEEDED` | 429 | API rate limit exceeded |
| `INVALID_API_KEY` | 401 | API key is invalid or expired |
| `INSUFFICIENT_PERMISSIONS` | 403 | API key lacks required permissions |
| `WALLET_NOT_FOUND` | 404 | Wallet not found on Solana network |
| `RPC_ERROR` | 502 | Solana RPC endpoint error |
| `TIMEOUT` | 504 | Request timeout |
| `INTERNAL_ERROR` | 500 | Internal server error |

### Handling Timeouts

For long-running scans, consider:

1. **Async Pattern**: Submit scan, poll status endpoint
2. **Webhooks**: Register for completion notifications
3. **Timeout Handling**: Implement proper timeout logic

```javascript
// Example: Async scan with polling
async function scanWalletWithPolling(address) {
  // Submit scan
  const response = await fetch('/api/v1/scan', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ wallet_address: address })
  });
  
  const { id } = await response.json();
  
  // Poll for completion
  while (true) {
    const statusResponse = await fetch(`/api/v1/scan/${id}`);
    const status = await statusResponse.json();
    
    if (status.status === 'completed') {
      return status.result;
    } else if (status.status === 'failed') {
      throw new Error('Scan failed');
    }
    
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
}
```

## Rate Limiting

### Rate Limits by Plan

| Plan | Requests/Hour | Concurrent Scans | Batch Size |
|------|---------------|------------------|------------|
| Free | 100 | 5 | 50 |
| Pro | 1,000 | 20 | 500 |
| Enterprise | 10,000 | 100 | 1,000 |

### Rate Limit Headers

All API responses include rate limit headers:

```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 850
X-RateLimit-Reset: 1642248000
```

### Handling Rate Limits

When rate limited, the API returns `429 Too Many Requests`:

```json
{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded",
    "details": {
      "limit": 1000,
      "window": "1h",
      "reset_time": "2024-01-15T11:00:00Z"
    }
  }
}
```

Implement exponential backoff for retries:

```python
import time
import requests
from requests.adapters import HTTPAdapter
from urllib3.util.retry import Retry

# Configure retry strategy
retry_strategy = Retry(
    total=3,
    backoff_factor=1,
    status_forcelist=[429, 500, 502, 503, 504],
)

adapter = HTTPAdapter(max_retries=retry_strategy)
session = requests.Session()
session.mount("https://", adapter)
session.mount("http://", adapter)

# Make request with automatic retries
response = session.post(
    "https://api.solana-recover.com/api/v1/scan",
    json={"wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"},
    headers={"X-API-Key": "your-api-key"}
)
```

## SDKs and Libraries

### Official SDKs

#### JavaScript/TypeScript

```bash
npm install @solana-recover/client
```

```javascript
import { SolanaRecoverClient } from '@solana-recover/client';

const client = new SolanaRecoverClient({
  apiKey: 'your-api-key',
  baseUrl: 'https://api.solana-recover.com'
});

const result = await client.scanWallet('9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM');
console.log(`Recoverable SOL: ${result.recoverable_sol}`);
```

#### Python

```bash
pip install solana-recover
```

```python
from solana_recover import SolanaRecoverClient

client = SolanaRecoverClient(
    api_key='your-api-key',
    base_url='https://api.solana-recover.com'
)

result = client.scan_wallet('9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM')
print(f"Recoverable SOL: {result.recoverable_sol}")
```

#### Rust

```bash
cargo add solana-recover-client
```

```rust
use solana_recover_client::SolanaRecoverClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SolanaRecoverClient::new(
        "your-api-key",
        "https://api.solana-recover.com"
    );
    
    let result = client
        .scan_wallet("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM")
        .await?;
    
    println!("Recoverable SOL: {}", result.recoverable_sol);
    Ok(())
}
```

## Examples

### Basic Usage

#### Scan a Single Wallet

```bash
curl -X POST https://api.solana-recover.com/api/v1/scan \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "fee_percentage": 0.15
  }'
```

#### Batch Scan Multiple Wallets

```bash
curl -X POST https://api.solana-recover.com/api/v1/batch-scan \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "wallet_addresses": [
      "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
      "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    ],
    "fee_percentage": 0.15
  }'
```

### Advanced Examples

#### Async Scan with Webhook

```bash
# Register webhook
curl -X POST https://api.solana-recover.com/api/v1/webhooks \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "url": "https://your-app.com/webhook/solana-recover",
    "events": ["scan.completed"],
    "secret": "your-webhook-secret"
  }'

# Submit scan (async)
curl -X POST https://api.solana-recover.com/api/v1/scan \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "async": true
  }'
```

#### Custom Fee Structure

```bash
curl -X POST https://api.solana-recover.com/api/v1/scan \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "fee_structure": {
      "percentage": 0.10,
      "minimum_lamports": 500000,
      "waive_below_lamports": 10000000
    }
  }'
```

### Integration Examples

#### Express.js Integration

```javascript
const express = require('express');
const { SolanaRecoverClient } = require('@solana-recover/client');

const app = express();
const client = new SolanaRecoverClient({
  apiKey: process.env.SOLANA_RECOVER_API_KEY
});

app.post('/api/wallets/:address/scan', async (req, res) => {
  try {
    const { address } = req.params;
    const result = await client.scanWallet(address);
    res.json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

app.listen(3000, () => {
  console.log('Server running on port 3000');
});
```

#### Django Integration

```python
from django.http import JsonResponse
from django.views.decorators.http import require_http_methods
from solana_recover import SolanaRecoverClient

client = SolanaRecoverClient(api_key=settings.SOLANA_RECOVER_API_KEY)

@require_http_methods(["POST"])
def scan_wallet(request, address):
    try:
        result = client.scan_wallet(address)
        return JsonResponse(result)
    except Exception as error:
        return JsonResponse({'error': str(error)}, status=500)
```

## Support

- 📧 **API Support**: api-support@solana-recover.com
- 📖 **Documentation**: [docs.solana-recover.com](https://docs.solana-recover.com)
- 🐛 **Bug Reports**: [GitHub Issues](https://github.com/Genius740Code/Sol-account-cleaner/issues)
- 💬 **Community**: [Discord](https://discord.gg/solana-recover)

---

For more information about using the Solana Recover API, check out our [Getting Started Guide](getting-started.md) and [Examples](../examples/).
