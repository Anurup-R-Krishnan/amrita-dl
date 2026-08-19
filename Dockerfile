# Multi-stage Dockerfile using cargo-chef for optimal layer caching
FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef
WORKDIR /app

# Stage 1: Compute dependency recipe
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Caching dependencies
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build actual application binary
COPY . .
RUN cargo build --release --bin server

# Stage 3: Minimal, secure production runtime
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create unprivileged user for security
RUN useradd -m -u 1000 -U appuser

WORKDIR /app

# Copy built release binary and web static assets
# Note: index.html is embedded into binary at compile time via rust-embed; /app/web is served at runtime via ServeDir
COPY --from=builder --chown=appuser:appuser /app/target/release/server /app/server
COPY --chown=appuser:appuser web /app/web

# Create data directory with appropriate appuser ownership
RUN mkdir -p /app/data && chown -R appuser:appuser /app/data

# Default environment configuration
ENV PORT=8080 \
    INDEX_DB=/app/data/index.db \
    INDEXED_ROOT=/app/data/amrita-exam-papers-indexed \
    RAW_ROOT=/app/data/amrita-exam-papers

USER appuser

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/health || exit 1

CMD ["/app/server"]
