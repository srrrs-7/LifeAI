FROM rust:1.78-slim AS builder

WORKDIR /build

# 依存解決に必要なパッケージを先にインストール
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# ── ① ワークスペースのマニフェストだけを先にコピー ──────────────────────
COPY Cargo.toml Cargo.lock ./
COPY core/Cargo.toml core/Cargo.toml
COPY core/otel-cc/Cargo.toml core/otel-cc/Cargo.toml

# ── ② ダミーソースで依存クレートだけを先にビルド（キャッシュ層として機能）──
RUN mkdir -p core/src core/otel-cc/src \
    && echo "pub fn dummy(){}" > core/src/lib.rs \
    && echo "fn main(){}" > core/otel-cc/src/main.rs \
    && cargo build --release -p otel-cc \
    && rm -rf core/src core/otel-cc/src

# ── ③ 本物のソースをコピーしてリビルド ──────────────────────────────────
COPY core/src/      core/src/
COPY core/otel-cc/  core/otel-cc/

# ダミービルド成果物のタイムスタンプをリセットして確実に再コンパイルさせる
RUN find core -name "*.rs" | xargs touch \
    && cargo build --release -p otel-cc

# ── ランタイムイメージ ───────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/otel-cc /usr/local/bin/otel-cc

RUN useradd -m vscode && mkdir -p /data && chown vscode:vscode /data
USER vscode

EXPOSE 4318 9091

CMD ["otel-cc"]