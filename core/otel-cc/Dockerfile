FROM rust:1.78-slim AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY core/Cargo.toml core/src/ core/
COPY core/otel-cc/ core/otel-cc/

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo build --release -p otel-cc

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/otel-cc /usr/local/bin/otel-cc

RUN useradd -m vscode
USER vscode

EXPOSE 4318 9091

CMD ["otel-cc"]
