use axum::{
    extract::{Request, State},
    http::{StatusCode, HeaderMap},
    middleware::Next,
    response::Response,
};
use governor::{
    clock::{QuantaClock, QuantaInstant},
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use std::sync::Arc;
use tracing::{debug, warn, info};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use chrono::Utc;
use metrics::{counter, histogram, gauge};
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String, // User ID
    pub email: String,
    pub api_key: String,
    pub rate_limit_rps: u32,
    pub permissions: Vec<String>,
    pub iat: i64, // Issued at
    pub exp: i64, // Expiration
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub token_expiry_hours: i64,
    pub api_key_header: String,
    pub enable_auth: bool,
    pub default_rate_limit_rps: u32,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| "default-secret-change-in-production".to_string()),
            token_expiry_hours: 24,
            api_key_header: "X-API-Key".to_string(),
            enable_auth: std::env::var("ENABLE_AUTH").unwrap_or_else(|_| "true".to_string()) == "true",
            default_rate_limit_rps: 100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub enable_jitter: bool,
    pub enable_ip_whitelist: bool,
    pub whitelisted_ips: Vec<String>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 100,
            burst_size: 10,
            enable_jitter: true,
            enable_ip_whitelist: false,
            whitelisted_ips: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub auth: AuthConfig,
    pub rate_limit: RateLimitConfig,
    pub enable_cors: bool,
    pub enable_compression: bool,
    pub max_request_size_mb: usize,
    pub enable_request_id: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            auth: AuthConfig::default(),
            rate_limit: RateLimitConfig::default(),
            enable_cors: true,
            enable_compression: true,
            max_request_size_mb: 10,
            enable_request_id: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecurityMiddleware {
    auth_config: AuthConfig,
    rate_limit_config: RateLimitConfig,
    // Per-IP rate limiters
    ip_limiters: Arc<
        tokio::sync::RwLock<
            std::collections::HashMap<
                String,
                Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>>
            >
        >
    >,
    // Per-API key rate limiters
    api_key_limiters: Arc<
        tokio::sync::RwLock<
            std::collections::HashMap<
                String,
                Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>>
            >
        >
    >,
}

impl SecurityMiddleware {
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            auth_config: config.auth.clone(),
            rate_limit_config: config.rate_limit,
            ip_limiters: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            api_key_limiters: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    pub async fn authenticate_request(&self, headers: &HeaderMap) -> Result<AuthClaims, StatusCode> {
        if !self.auth_config.enable_auth {
            // Return default claims for unauthenticated requests
            return Ok(AuthClaims {
                sub: "anonymous".to_string(),
                email: "anonymous@example.com".to_string(),
                api_key: "anonymous".to_string(),
                rate_limit_rps: self.auth_config.default_rate_limit_rps,
                permissions: vec!["read".to_string()],
                iat: Utc::now().timestamp(),
                exp: Utc::now().timestamp() + 3600,
            });
        }
        
        // Try API key authentication first
        if let Some(api_key) = headers.get(&self.auth_config.api_key_header) {
            if let Ok(api_key_str) = api_key.to_str() {
                return self.authenticate_api_key(api_key_str).await;
            }
        }
        
        // Try JWT authentication
        if let Some(auth_header) = headers.get("authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token = &auth_str[7..];
                    return self.authenticate_jwt(token).await;
                }
            }
        }
        
        warn!("Authentication failed: no valid credentials found");
        Err(StatusCode::UNAUTHORIZED)
    }
    
    async fn authenticate_api_key(&self, api_key: &str) -> Result<AuthClaims, StatusCode> {
        // In a real implementation, you'd validate the API key against a database
        // For now, we'll create a simple hash-based validation
        
        let api_key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));
        
        // Mock validation - in production, check against database
        if api_key.len() < 16 {
            warn!("Invalid API key format");
            return Err(StatusCode::UNAUTHORIZED);
        }
        
        // Create claims based on API key
        let claims = AuthClaims {
            sub: format!("user_{}", &api_key_hash[..8]),
            email: format!("user-{}@example.com", &api_key_hash[..8]),
            api_key: api_key.to_string(),
            rate_limit_rps: self.auth_config.default_rate_limit_rps,
            permissions: vec!["read".to_string(), "write".to_string()],
            iat: Utc::now().timestamp(),
            exp: Utc::now().timestamp() + (self.auth_config.token_expiry_hours * 3600),
        };
        
        debug!("API key authentication successful for user: {}", claims.sub);
        Ok(claims)
    }
    
    async fn authenticate_jwt(&self, token: &str) -> Result<AuthClaims, StatusCode> {
        let validation = Validation::default();
        let decoding_key = DecodingKey::from_secret(self.auth_config.jwt_secret.as_ref());
        
        match decode::<AuthClaims>(token, &decoding_key, &validation) {
            Ok(token_data) => {
                let claims = token_data.claims;
                
                // Check if token is expired
                if Utc::now().timestamp() > claims.exp {
                    warn!("JWT token expired for user: {}", claims.sub);
                    return Err(StatusCode::UNAUTHORIZED);
                }
                
                debug!("JWT authentication successful for user: {}", claims.sub);
                Ok(claims)
            }
            Err(e) => {
                warn!("JWT authentication failed: {}", e);
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    }
    
    pub async fn check_rate_limit(&self, ip: &str, claims: &AuthClaims) -> Result<(), StatusCode> {
        // Check IP whitelist
        if self.rate_limit_config.enable_ip_whitelist && !self.rate_limit_config.whitelisted_ips.contains(&ip.to_string()) {
            warn!("IP {} not in whitelist", ip);
            return Err(StatusCode::FORBIDDEN);
        }
        
        // Get or create rate limiter for this API key
        let limiter = {
            let mut limiters = self.api_key_limiters.write().await;
            limiters.entry(claims.api_key.clone()).or_insert_with(|| {
                let quota = Quota::per_minute(NonZeroU32::new(claims.rate_limit_rps).unwrap());
                Arc::new(RateLimiter::direct(quota))
            }).clone()
        };
        
        // Check rate limit
        match limiter.check() {
            Ok(_) => {
                debug!("Rate limit check passed for API key: {}", claims.api_key);
                Ok(())
            }
            Err(_) => {
                warn!("Rate limit exceeded for API key: {}", claims.api_key);
                counter!("security.rate_limit_exceeded", 1);
                Err(StatusCode::TOO_MANY_REQUESTS)
            }
        }
    }
    
    pub async fn generate_jwt_token(&self, user_id: &str, email: &str, api_key: &str, permissions: Vec<String>) -> Result<String, crate::SolanaRecoverError> {
        let now = Utc::now();
        let claims = AuthClaims {
            sub: user_id.to_string(),
            email: email.to_string(),
            api_key: api_key.to_string(),
            rate_limit_rps: self.auth_config.default_rate_limit_rps,
            permissions,
            iat: now.timestamp(),
            exp: now.timestamp() + (self.auth_config.token_expiry_hours * 3600),
        };
        
        let encoding_key = EncodingKey::from_secret(self.auth_config.jwt_secret.as_ref());
        let token = encode(&Header::default(), &claims, &encoding_key)
            .map_err(|e| crate::SolanaRecoverError::AuthenticationError(format!("Failed to generate token: {}", e)))?;
        
        info!("Generated JWT token for user: {}", user_id);
        Ok(token)
    }
    
    pub async fn cleanup_expired_limiters(&self) {
        let mut ip_limiters = self.ip_limiters.write().await;
        let mut api_key_limiters = self.api_key_limiters.write().await;
        
        // Simple cleanup strategy: remove limiters that haven't been used recently
        // In a more sophisticated implementation, you'd track last usage time
        
        if ip_limiters.len() > 10000 {
            // Keep only the most recent 5000 IP limiters
            let keys_to_remove: Vec<String> = ip_limiters.keys().take(5000).cloned().collect();
            for key in keys_to_remove {
                ip_limiters.remove(&key);
            }
            info!("Cleaned up IP rate limiters, remaining: {}", ip_limiters.len());
        }
        
        if api_key_limiters.len() > 10000 {
            // Keep only the most recent 5000 API key limiters
            let keys_to_remove: Vec<String> = api_key_limiters.keys().take(5000).cloned().collect();
            for key in keys_to_remove {
                api_key_limiters.remove(&key);
            }
            info!("Cleaned up API key rate limiters, remaining: {}", api_key_limiters.len());
        }
    }
}

// Middleware function for Axum
pub async fn security_middleware(
    State(security): State<Arc<SecurityMiddleware>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let start_time = std::time::Instant::now();
    
    // Extract client IP
    let ip = extract_client_ip(&request);
    
    // Extract headers
    let headers = request.headers().clone();
    
    // Authenticate request
    let claims = security.authenticate_request(&headers).await?;
    
    // Check rate limits
    security.check_rate_limit(&ip, &claims).await?;
    
    // Add security headers to response
    let mut response = next.run(request).await;
    
    // Add security headers
    let response_headers = response.headers_mut();
    response_headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    response_headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    response_headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    response_headers.insert("Strict-Transport-Security", "max-age=31536000; includeSubDomains".parse().unwrap());
    response_headers.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
    
    // Add user context to headers for downstream handlers
    response_headers.insert("X-User-ID", claims.sub.parse().unwrap());
    response_headers.insert("X-User-Email", claims.email.parse().unwrap());
    
    // Record metrics
    let duration = start_time.elapsed();
    histogram!("security.middleware.duration_ms", duration.as_millis() as f64);
    counter!("security.requests.total", 1);
    gauge!("security.active_users", 1.0);
    
    debug!("Security middleware processed request in {:?} for user: {}", duration, claims.sub);
    
    Ok(response)
}

fn extract_client_ip(request: &Request) -> String {
    // Try to get IP from various headers
    let headers = request.headers();
    
    // Check for forwarded headers in order of preference
    let forwarded_headers = [
        "x-forwarded-for",
        "x-real-ip", 
        "cf-connecting-ip",
        "x-client-ip",
    ];
    
    for header_name in forwarded_headers {
        if let Some(header_value) = headers.get(header_name) {
            if let Ok(header_str) = header_value.to_str() {
                // X-Forwarded-For can contain multiple IPs, take the first one
                let ip = header_str.split(',').next().unwrap_or("").trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }
    
    // Fallback to connection info
    "unknown".to_string()
}

// Request ID middleware
pub async fn request_id_middleware(
    request: Request,
    next: Next,
) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Add request ID to response headers
    let mut response = next.run(request).await;
    response.headers_mut().insert("X-Request-ID", request_id.parse().unwrap());
    
    response
}

// Logging middleware
pub async fn logging_middleware(
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let user_agent = request.headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown").to_string();
    
    let start_time = std::time::Instant::now();
    let response = next.run(request).await;
    let duration = start_time.elapsed();
    
    let status = response.status();
    let request_id = response.headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");
    
    // Log request details
    info!(
        method = %method,
        uri = %uri,
        status = %status,
        duration_ms = duration.as_millis(),
        user_agent = %user_agent,
        request_id = %request_id,
        "HTTP request completed"
    );
    
    // Record metrics
    counter!("http_requests_total", 1);
    histogram!("http_request_duration_ms", duration.as_millis() as f64);
    
    response
}

// CORS middleware configuration
pub fn cors_layer() -> tower_http::cors::CorsLayer {
    use tower_http::cors::{Any, CorsLayer};
    
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::PUT, axum::http::Method::DELETE])
        .allow_headers(Any)
        .allow_credentials(true)
}

// Compression middleware configuration
pub fn compression_layer() -> tower_http::compression::CompressionLayer {
    tower_http::compression::CompressionLayer::new()
}

// Request size limit middleware (placeholder - would need tower-http limit feature)
pub fn request_size_limit_layer(_max_size_mb: usize) -> String {
    "Request size limiting not available without tower-http limit feature".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    
    #[tokio::test]
    async fn test_security_middleware_creation() {
        let config = SecurityConfig::default();
        let middleware = SecurityMiddleware::new(config);
        
        assert_eq!(middleware.auth_config.enable_auth, true);
        assert_eq!(middleware.rate_limit_config.requests_per_minute, 100);
    }
    
    #[tokio::test]
    async fn test_jwt_token_generation() {
        let config = SecurityConfig::default();
        let middleware = SecurityMiddleware::new(config);
        
        let token = middleware.generate_jwt_token(
            "test_user",
            "test@example.com", 
            "test_api_key",
            vec!["read".to_string()]
        ).await;
        
        assert!(token.is_ok());
    }
    
    #[tokio::test]
    async fn test_jwt_token_validation() {
        let config = SecurityConfig::default();
        let middleware = SecurityMiddleware::new(config);
        
        let token = middleware.generate_jwt_token(
            "test_user",
            "test@example.com",
            "test_api_key", 
            vec!["read".to_string()]
        ).await.unwrap();
        
        let claims = middleware.authenticate_jwt(&token).await;
        assert!(claims.is_ok());
        
        let claims_data = claims.unwrap();
        assert_eq!(claims_data.sub, "test_user");
        assert_eq!(claims_data.email, "test@example.com");
    }
}
