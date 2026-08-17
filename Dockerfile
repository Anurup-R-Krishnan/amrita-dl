# Multi-stage Dockerfile for Amrita Exam Papers Search Engine
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Create dummy source to cache Rust dependency builds
RUN mkdir -p src/bin && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/bin/server.rs && \
    cargo build --release && \
    rm -rf src

# Copy real source code
COPY src ./src
COPY web ./web

# Touch main files to invalidate cargo build cache for application code
RUN touch src/main.rs src/bin/server.rs && cargo build --release --bin server

# Runtime image
FROM debian:bookworm-slim

# Install runtime SSL certs and SQLite libraries
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy built release binary and web static assets
COPY --from=builder /app/target/release/server /app/server
COPY web /app/web

# Default environment configuration
ENV PORT=8080 \
    INDEX_DB=/app/data/index.db \
    INDEXED_ROOT=/app/data/amrita-exam-papers-indexed \
    RAW_ROOT=/app/data/amrita-exam-papers

EXPOSE 8080

CMD ["/app/server"]
