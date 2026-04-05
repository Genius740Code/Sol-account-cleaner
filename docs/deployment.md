# Deployment Guide

This guide covers deploying Solana Recover to production environments, including Docker, Kubernetes, cloud platforms, and on-premises setups.

## Table of Contents

- [Deployment Options](#deployment-options)
- [System Requirements](#system-requirements)
- [Environment Configuration](#environment-configuration)
- [Docker Deployment](#docker-deployment)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Cloud Platform Deployment](#cloud-platform-deployment)
- [Monitoring and Logging](#monitoring-and-logging)
- [Security Considerations](#security-considerations)
- [Performance Tuning](#performance-tuning)
- [Backup and Recovery](#backup-and-recovery)
- [Maintenance](#maintenance)

## Deployment Options

### Recommended Deployments

1. **Docker Compose** - Small to medium deployments
2. **Kubernetes** - Large-scale, containerized deployments
3. **Cloud Services** - Managed solutions
4. **Bare Metal** - On-premises deployments

### Deployment Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Load Balancer │    │   Web Frontend  │    │   Mobile Apps   │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────┴─────────────┐
                    │   API Gateway/Proxy       │
                    │  (nginx, traefik, etc.)   │
                    └─────────────┬─────────────┘
                                 │
                    ┌─────────────┴─────────────┐
                    │   Solana Recover API      │
                    │  (Multiple Instances)     │
                    └─────────────┬─────────────┘
                                 │
                    ┌─────────────┴─────────────┐
                    │   Database & Cache        │
                    │  (PostgreSQL, Redis)      │
                    └─────────────┬─────────────┘
                                 │
                    ┌─────────────┴─────────────┐
                    │   Monitoring & Logging    │
                    │ (Prometheus, Grafana)     │
                    └───────────────────────────┘
```

## System Requirements

### Minimum Requirements

- **CPU**: 2 cores
- **Memory**: 4GB RAM
- **Storage**: 50GB SSD
- **Network**: 100 Mbps

### Recommended Requirements

- **CPU**: 4+ cores
- **Memory**: 8GB+ RAM
- **Storage**: 100GB+ SSD
- **Network**: 1 Gbps

### High-Performance Requirements

- **CPU**: 8+ cores
- **Memory**: 16GB+ RAM
- **Storage**: 500GB+ NVMe SSD
- **Network**: 10 Gbps

### Software Dependencies

- **Docker**: 20.10+
- **Docker Compose**: 2.0+
- **Kubernetes**: 1.24+ (if using K8s)
- **PostgreSQL**: 13+ (for production database)
- **Redis**: 6+ (for caching)

## Environment Configuration

### Environment Variables

Create a `.env` file for your deployment:

```bash
# Application Configuration
SOLANA_RECOVER_ENV=production
SOLANA_RECOVER_LOG_LEVEL=info
SOLANA_RECOVER_PORT=8080
SOLANA_RECOVER_HOST=0.0.0.0

# Database Configuration
DATABASE_URL=postgresql://user:password@postgres:5432/solana_recover
DATABASE_POOL_SIZE=20
DATABASE_TIMEOUT_SECONDS=30

# Redis Configuration
REDIS_URL=redis://redis:6379
REDIS_POOL_SIZE=10

# Solana RPC Configuration
SOLANA_RPC_ENDPOINTS=https://api.mainnet-beta.solana.com,https://solana-api.projectserum.com
SOLANA_RPC_POOL_SIZE=50
SOLANA_RPC_TIMEOUT_MS=5000
SOLANA_RPC_RATE_LIMIT_RPS=100

# Security Configuration
JWT_SECRET=your-super-secret-jwt-key
API_KEY_ENCRYPTION_KEY=your-32-character-encryption-key
CORS_ORIGINS=https://yourdomain.com,https://app.yourdomain.com

# Monitoring Configuration
METRICS_ENABLED=true
METRICS_PORT=9090
HEALTH_CHECK_INTERVAL=30

# Performance Configuration
MAX_CONCURRENT_WALLETS=1000
BATCH_SIZE=100
CACHE_TTL_SECONDS=300
```

### Configuration Files

#### Production Config (`config/production.toml`)

```toml
[server]
host = "0.0.0.0"
port = 8080
workers = 8
timeout_seconds = 60

[database]
url = "${DATABASE_URL}"
pool_size = 20
timeout_seconds = 30
migration_auto = true

[redis]
url = "${REDIS_URL}"
pool_size = 10
timeout_seconds = 5

[rpc]
endpoints = ["https://api.mainnet-beta.solana.com", "https://solana-api.projectserum.com"]
pool_size = 50
timeout_ms = 5000
rate_limit_rps = 100
health_check_interval_seconds = 30

[scanner]
batch_size = 100
max_concurrent_wallets = 1000
retry_attempts = 3
retry_delay_ms = 1000

[fees]
default_percentage = 0.15
minimum_lamports = 1000000
waive_below_lamports = 10000000

[security]
jwt_secret = "${JWT_SECRET}"
api_key_encryption_key = "${API_KEY_ENCRYPTION_KEY}"
cors_origins = ["https://yourdomain.com"]

[monitoring]
metrics_enabled = true
metrics_port = 9090
health_check_interval = 30
log_level = "info"

[cache]
ttl_seconds = 300
max_size = 10000
```

## Docker Deployment

### Dockerfile

```dockerfile
# Multi-stage build for production
FROM rust:1.75-slim as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY config ./config

# Build the application
RUN cargo build --release

# Production image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 solana

# Copy application
COPY --from=builder /app/target/release/solana-recover /usr/local/bin/
COPY --from=builder /app/config ./config

# Set permissions
RUN chown -R solana:solana /app
USER solana

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Start the application
CMD ["solana-recover", "server", "--config", "config/production.toml"]
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  solana-recover:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - DATABASE_URL=postgresql://postgres:${POSTGRES_PASSWORD}@postgres:5432/solana_recover
      - REDIS_URL=redis://redis:6379
      - JWT_SECRET=${JWT_SECRET}
      - API_KEY_ENCRYPTION_KEY=${API_KEY_ENCRYPTION_KEY}
    env_file:
      - .env
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    restart: unless-stopped
    networks:
      - solana-network
    volumes:
      - ./config:/app/config:ro
      - ./logs:/app/logs

  postgres:
    image: postgres:15-alpine
    environment:
      - POSTGRES_DB=solana_recover
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./scripts/init.sql:/docker-entrypoint-initdb.d/init.sql
    networks:
      - solana-network
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    command: redis-server --appendonly yes
    volumes:
      - redis_data:/data
    networks:
      - solana-network
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx/nginx.conf:/etc/nginx/nginx.conf:ro
      - ./nginx/ssl:/etc/nginx/ssl:ro
    depends_on:
      - solana-recover
    networks:
      - solana-network
    restart: unless-stopped

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9091:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
    networks:
      - solana-network
    restart: unless-stopped

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_PASSWORD}
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana/dashboards:/etc/grafana/provisioning/dashboards:ro
      - ./monitoring/grafana/datasources:/etc/grafana/provisioning/datasources:ro
    networks:
      - solana-network
    restart: unless-stopped

volumes:
  postgres_data:
  redis_data:
  prometheus_data:
  grafana_data:

networks:
  solana-network:
    driver: bridge
```

### Deployment Commands

```bash
# Build and start services
docker-compose up -d --build

# View logs
docker-compose logs -f solana-recover

# Scale the application
docker-compose up -d --scale solana-recover=3

# Update the application
docker-compose pull
docker-compose up -d

# Backup database
docker-compose exec postgres pg_dump -U postgres solana_recover > backup.sql

# Restore database
docker-compose exec -T postgres psql -U postgres solana_recover < backup.sql
```

## Kubernetes Deployment

### Namespace

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: solana-recover
  labels:
    name: solana-recover
```

### ConfigMap

```yaml
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: solana-recover-config
  namespace: solana-recover
data:
  production.toml: |
    [server]
    host = "0.0.0.0"
    port = 8080
    workers = 8

    [database]
    url = "${DATABASE_URL}"
    pool_size = 20

    [redis]
    url = "${REDIS_URL}"
    pool_size = 10

    [rpc]
    endpoints = ["https://api.mainnet-beta.solana.com"]
    pool_size = 50
    timeout_ms = 5000
    rate_limit_rps = 100

    [scanner]
    batch_size = 100
    max_concurrent_wallets = 1000
    retry_attempts = 3
    retry_delay_ms = 1000

    [fees]
    default_percentage = 0.15
    minimum_lamports = 1000000
    waive_below_lamports = 10000000
```

### Secret

```yaml
# secret.yaml
apiVersion: v1
kind: Secret
metadata:
  name: solana-recover-secrets
  namespace: solana-recover
type: Opaque
data:
  database-url: <base64-encoded-database-url>
  jwt-secret: <base64-encoded-jwt-secret>
  api-key-encryption-key: <base64-encoded-encryption-key>
```

### Deployment

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: solana-recover
  namespace: solana-recover
  labels:
    app: solana-recover
spec:
  replicas: 3
  selector:
    matchLabels:
      app: solana-recover
  template:
    metadata:
      labels:
        app: solana-recover
    spec:
      containers:
      - name: solana-recover
        image: solana-recover:latest
        ports:
        - containerPort: 8080
        - containerPort: 9090
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: solana-recover-secrets
              key: database-url
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: solana-recover-secrets
              key: jwt-secret
        - name: API_KEY_ENCRYPTION_KEY
          valueFrom:
            secretKeyRef:
              name: solana-recover-secrets
              key: api-key-encryption-key
        volumeMounts:
        - name: config
          mountPath: /app/config
        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: solana-recover-config
```

### Service

```yaml
# service.yaml
apiVersion: v1
kind: Service
metadata:
  name: solana-recover-service
  namespace: solana-recover
spec:
  selector:
    app: solana-recover
  ports:
  - name: http
    port: 80
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: 9090
  type: ClusterIP
```

### Ingress

```yaml
# ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: solana-recover-ingress
  namespace: solana-recover
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/rate-limit: "100"
spec:
  tls:
  - hosts:
    - api.solana-recover.com
    secretName: solana-recover-tls
  rules:
  - host: api.solana-recover.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: solana-recover-service
            port:
              number: 80
```

### Horizontal Pod Autoscaler

```yaml
# hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: solana-recover-hpa
  namespace: solana-recover
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: solana-recover
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

### Deployment Commands

```bash
# Apply all configurations
kubectl apply -f namespace.yaml
kubectl apply -f configmap.yaml
kubectl apply -f secret.yaml
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
kubectl apply -f ingress.yaml
kubectl apply -f hpa.yaml

# Check deployment status
kubectl get pods -n solana-recover
kubectl get services -n solana-recover
kubectl get ingress -n solana-recover

# View logs
kubectl logs -f deployment/solana-recover -n solana-recover

# Scale deployment
kubectl scale deployment solana-recover --replicas=5 -n solana-recover

# Update deployment
kubectl set image deployment/solana-recover solana-recover=solana-recover:v1.2.0 -n solana-recover
```

## Cloud Platform Deployment

### AWS Deployment

#### Using ECS (Elastic Container Service)

```json
{
  "family": "solana-recover",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "512",
  "memory": "1024",
  "executionRoleArn": "arn:aws:iam::account:role/ecsTaskExecutionRole",
  "taskRoleArn": "arn:aws:iam::account:role/ecsTaskRole",
  "containerDefinitions": [
    {
      "name": "solana-recover",
      "image": "your-account.dkr.ecr.region.amazonaws.com/solana-recover:latest",
      "portMappings": [
        {
          "containerPort": 8080,
          "protocol": "tcp"
        }
      ],
      "environment": [
        {
          "name": "DATABASE_URL",
          "value": "postgresql://user:pass@rds-endpoint:5432/db"
        }
      ],
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/solana-recover",
          "awslogs-region": "us-west-2",
          "awslogs-stream-prefix": "ecs"
        }
      },
      "healthCheck": {
        "command": ["CMD-SHELL", "curl -f http://localhost:8080/health || exit 1"],
        "interval": 30,
        "timeout": 5,
        "retries": 3
      }
    }
  ]
}
```

#### Infrastructure as Code (Terraform)

```hcl
# main.tf
provider "aws" {
  region = var.aws_region
}

# VPC
resource "aws_vpc" "main" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = {
    Name = "solana-recover-vpc"
  }
}

# ECS Cluster
resource "aws_ecs_cluster" "main" {
  name = "solana-recover"

  setting {
    name  = "containerInsights"
    value = "enabled"
  }
}

# RDS Database
resource "aws_db_instance" "postgres" {
  identifier     = "solana-recover-db"
  engine         = "postgres"
  engine_version = "15.3"
  instance_class = "db.t3.micro"
  
  allocated_storage     = 20
  max_allocated_storage  = 100
  storage_encrypted      = true
  storage_type          = "gp2"
  
  db_name  = "solana_recover"
  username = var.db_username
  password = var.db_password
  
  vpc_security_group_ids = [aws_security_group.rds.id]
  db_subnet_group_name   = aws_db_subnet_group.main.name
  
  backup_retention_period = 7
  backup_window          = "03:00-04:00"
  maintenance_window     = "sun:04:00-sun:05:00"
  
  skip_final_snapshot = true
  
  tags = {
    Name = "solana-recover-db"
  }
}

# ElastiCache Redis
resource "aws_elasticache_subnet_group" "main" {
  name       = "solana-recover-cache-subnet"
  subnet_ids = aws_subnet.private[*].id
}

resource "aws_elasticache_cluster" "redis" {
  cluster_id           = "solana-recover-redis"
  engine               = "redis"
  node_type            = "cache.t3.micro"
  num_cache_nodes      = 1
  parameter_group_name = "default.redis7"
  port                 = 6379
  subnet_group_name    = aws_elasticache_subnet_group.main.name
  security_group_ids   = [aws_security_group.redis.id]
  
  tags = {
    Name = "solana-recover-redis"
  }
}

# Application Load Balancer
resource "aws_lb" "main" {
  name               = "solana-recover-alb"
  internal           = false
  load_balancer_type = "application"
  security_groups    = [aws_security_group.alb.id]
  subnets            = aws_subnet.public[*].id

  enable_deletion_protection = false

  tags = {
    Name = "solana-recover-alb"
  }
}

# ECS Service
resource "aws_ecs_service" "main" {
  name            = "solana-recover"
  cluster         = aws_ecs_cluster.main.id
  task_definition = aws_ecs_task_definition.main.arn
  desired_count   = 2
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = aws_subnet.private[*].id
    security_groups  = [aws_security_group.ecs.id]
    assign_public_ip = false
  }

  load_balancer {
    target_group_arn = aws_lb_target_group.main.arn
    container_name   = "solana-recover"
    container_port   = 8080
  }

  depends_on = [aws_lb_listener.main]
}
```

### Google Cloud Platform

#### Cloud Run Deployment

```bash
# Build and push image
gcloud builds submit --tag gcr.io/PROJECT-ID/solana-recover

# Deploy to Cloud Run
gcloud run deploy solana-recover \
  --image gcr.io/PROJECT-ID/solana-recover \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --memory 1Gi \
  --cpu 1 \
  --max-instances 100 \
  --min-instances 0 \
  --set-env-vars DATABASE_URL=postgresql://...,REDIS_URL=redis://...
```

### Azure Container Instances

```yaml
# azure-deployment.yaml
apiVersion: 2019-12-01
location: eastus
name: solana-recover-group
properties:
  containers:
  - name: solana-recover
    properties:
      image: solana-recover:latest
      ports:
      - port: 8080
      resources:
        requests:
          cpu: 1.0
          memoryInGb: 2.0
      environmentVariables:
      - name: DATABASE_URL
        secureValue: your-connection-string
  osType: Linux
  restartPolicy: Always
  ipAddress:
    type: Public
    ports:
    - port: 8080
      protocol: TCP
tags: {}
type: Microsoft.ContainerInstance/containerGroups
```

## Monitoring and Logging

### Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "solana_recover_rules.yml"

scrape_configs:
  - job_name: 'solana-recover'
    static_configs:
      - targets: ['solana-recover:9090']
    metrics_path: /metrics
    scrape_interval: 10s

  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']

  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
```

### Grafana Dashboard

```json
{
  "dashboard": {
    "title": "Solana Recover Dashboard",
    "panels": [
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(solana_recover_requests_total[5m])",
            "legendFormat": "{{method}} {{status}}"
          }
        ]
      },
      {
        "title": "Response Time",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(solana_recover_request_duration_seconds_bucket[5m]))",
            "legendFormat": "95th percentile"
          }
        ]
      },
      {
        "title": "Active Scans",
        "type": "singlestat",
        "targets": [
          {
            "expr": "solana_recover_active_scans",
            "legendFormat": "Active Scans"
          }
        ]
      }
    ]
  }
}
```

### Logging Configuration

```yaml
# docker-compose.logging.yml
version: '3.8'

services:
  elasticsearch:
    image: docker.elastic.co/elasticsearch/elasticsearch:8.5.0
    environment:
      - discovery.type=single-node
      - "ES_JAVA_OPTS=-Xms512m -Xmx512m"
    volumes:
      - elasticsearch_data:/usr/share/elasticsearch/data
    networks:
      - logging

  logstash:
    image: docker.elastic.co/logstash/logstash:8.5.0
    volumes:
      - ./logstash/pipeline:/usr/share/logstash/pipeline:ro
    networks:
      - logging
    depends_on:
      - elasticsearch

  kibana:
    image: docker.elastic.co/kibana/kibana:8.5.0
    ports:
      - "5601:5601"
    environment:
      - ELASTICSEARCH_HOSTS=http://elasticsearch:9200
    networks:
      - logging
    depends_on:
      - elasticsearch

volumes:
  elasticsearch_data:

networks:
  logging:
    driver: bridge
```

## Security Considerations

### Network Security

```yaml
# nginx.conf
server {
    listen 443 ssl http2;
    server_name api.solana-recover.com;

    ssl_certificate /etc/nginx/ssl/cert.pem;
    ssl_certificate_key /etc/nginx/ssl/key.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512;

    # Security headers
    add_header X-Frame-Options DENY;
    add_header X-Content-Type-Options nosniff;
    add_header X-XSS-Protection "1; mode=block";
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains";

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;
    limit_req zone=api burst=20 nodelay;

    location / {
        proxy_pass http://solana-recover:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Secrets Management

```bash
# Using Kubernetes Secrets
kubectl create secret generic solana-recover-secrets \
  --from-literal=database-url="postgresql://..." \
  --from-literal=jwt-secret="your-jwt-secret" \
  --from-literal=api-key-encryption-key="your-encryption-key"

# Using AWS Secrets Manager
aws secretsmanager create-secret \
  --name solana-recover/production \
  --secret-string '{"database_url":"...","jwt_secret":"..."}'
```

## Performance Tuning

### Database Optimization

```sql
-- PostgreSQL optimization
-- Create indexes for common queries
CREATE INDEX CONCURRENTLY idx_wallet_scans_address ON wallet_scans(wallet_address);
CREATE INDEX CONCURRENTLY idx_wallet_scans_created_at ON wallet_scans(created_at);

-- Partition large tables
CREATE TABLE wallet_scans_partitioned (
    LIKE wallet_scans INCLUDING ALL
) PARTITION BY RANGE (created_at);

-- Connection pooling configuration
ALTER SYSTEM SET max_connections = 200;
ALTER SYSTEM SET shared_buffers = '256MB';
ALTER SYSTEM SET effective_cache_size = '1GB';
```

### Application Tuning

```toml
# Performance configuration
[server]
workers = 8
timeout_seconds = 60

[database]
pool_size = 20
timeout_seconds = 30
statement_timeout_seconds = 30

[scanner]
batch_size = 200
max_concurrent_wallets = 2000
queue_size = 10000

[cache]
ttl_seconds = 600
max_size = 50000
```

## Backup and Recovery

### Database Backups

```bash
# Automated backup script
#!/bin/bash
BACKUP_DIR="/backups/solana-recover"
DATE=$(date +%Y%m%d_%H%M%S)

# Create backup
docker-compose exec -T postgres pg_dump -U postgres solana_recover | gzip > "$BACKUP_DIR/backup_$DATE.sql.gz"

# Retention policy (keep 30 days)
find "$BACKUP_DIR" -name "backup_*.sql.gz" -mtime +30 -delete

# Upload to cloud storage (optional)
aws s3 cp "$BACKUP_DIR/backup_$DATE.sql.gz" s3://your-backup-bucket/
```

### Disaster Recovery

```yaml
# disaster-recovery.yaml
apiVersion: v1
kind: Pod
metadata:
  name: disaster-recovery
spec:
  containers:
  - name: recovery
    image: postgres:15-alpine
    command: ["/bin/bash"]
    args: ["-c", "while true; do sleep 30; done"]
    volumeMounts:
    - name: backup-storage
      mountPath: /backups
  volumes:
  - name: backup-storage
    persistentVolumeClaim:
      claimName: backup-pvc
```

## Maintenance

### Health Checks

```bash
# Health check script
#!/bin/bash

# Check API health
curl -f http://localhost:8080/health || exit 1

# Check database connectivity
docker-compose exec postgres pg_isready -U postgres || exit 1

# Check Redis connectivity
docker-compose exec redis redis-cli ping || exit 1

# Check metrics endpoint
curl -f http://localhost:9090/metrics || exit 1

echo "All health checks passed"
```

### Rolling Updates

```bash
# Zero-downtime deployment
#!/bin/bash

# Pull new image
docker-compose pull solana-recover

# Update service one container at a time
docker-compose up -d --no-deps solana-recover

# Wait for health check
sleep 30

# Verify deployment
curl -f http://localhost:8080/health

echo "Deployment completed successfully"
```

---

This deployment guide provides comprehensive instructions for deploying Solana Recover in various environments. For additional support, contact our team at deployment@solana-recover.com.
