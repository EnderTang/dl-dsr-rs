FROM rust:1.89-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p dldsr-node -p dldsr-bench

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/dldsr-node /usr/local/bin/dldsr-node
COPY --from=builder /app/target/release/dldsr-bench /usr/local/bin/dldsr-bench
COPY docker/entrypoint.sh /usr/local/bin/dldsr-entrypoint
COPY docker/topology /app/docker/topology
RUN chmod +x /usr/local/bin/dldsr-entrypoint
ENTRYPOINT ["dldsr-entrypoint"]

