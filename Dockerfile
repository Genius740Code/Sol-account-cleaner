# Multi-stage build for Solana Recover
# Stage 1: Build
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src
COPY config ./config

# Build the application
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false solana-recover

# Create directories
RUN mkdir -p /app/config /app/data /app/logs && \
    chown -R solana-recover:solana-recover /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/solana-recover /app/solana-recover

# Copy configuration files
COPY --from=builder /app/config /app/config

# Set ownership
RUN chown -R solana-recover:solana-recover /app

# Switch to non-root user
USER solana-recover

# Set working directory
WORKDIR /app

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Default command
CMD ["./solana-recover", "server"]
