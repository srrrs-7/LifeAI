# ============================================================
# LifeAI Makefile
# ============================================================
.PHONY: help build build-release test fmt clippy check clean \
        hooks init-firewall \
        build-otel-cc test-otel-cc \
        coverage coverage-html coverage-check \
        metrics open-grafana open-prometheus \
        restart-infra rebuild-otel-cc logs-otel-cc logs-prometheus logs-grafana

# ------------------------------------------------------------
# Default
# ------------------------------------------------------------
help: ## ヘルプ表示
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-24s\033[0m %s\n", $$1, $$2}'

# ------------------------------------------------------------
# Rust — ビルド・テスト・リント
# ------------------------------------------------------------
build: ## cargo build（デバッグ）
	cargo build

build-release: ## cargo build --release
	cargo build --release

test: ## テスト実行
	cargo test

fmt: ## cargo fmt
	cargo fmt

fmt-check: ## フォーマットチェック（CI 用）
	cargo fmt -- --check

clippy: ## リント（warnings = error）
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt clippy test ## fmt + clippy + test を一括実行（高速ループ用）

clean: ## target/ を削除
	cargo clean

# ------------------------------------------------------------
# カバレッジ（cargo-llvm-cov 必須: cargo install cargo-llvm-cov）
# ------------------------------------------------------------
coverage: ## カバレッジ計測（テキストサマリー表示）
	cargo llvm-cov --package otel-cc --summary-only

coverage-html: ## カバレッジ HTML レポート生成
	cargo llvm-cov --package otel-cc --html
	@echo "Report: target/llvm-cov/html/index.html"

coverage-check: ## カバレッジ計測（60% 未満で CI 失敗）
	cargo llvm-cov --package otel-cc --fail-under-lines 60

# ------------------------------------------------------------
# Rust — クレート個別
# ------------------------------------------------------------
build-otel-cc: ## otel-cc のみビルド
	cargo build -p otel-cc

test-otel-cc: ## otel-cc のみテスト
	cargo test -p otel-cc

# ------------------------------------------------------------
# 動作確認・ユーティリティ
# ------------------------------------------------------------
metrics: ## /metrics エンドポイントを確認（otel-cc コンテナ経由）
	@docker compose -f .devcontainer/compose.yaml exec otel-cc curl -sf http://localhost:9091/metrics | head -40 || echo "otel-cc is not running"

open-grafana: ## Grafana をブラウザで開く
	@echo "http://localhost:3000"

open-prometheus: ## Prometheus をブラウザで開く
	@echo "http://localhost:9090"

restart-infra: ## 全コンテナを再起動（otel-cc, prometheus, grafana）
	docker-compose -f .devcontainer/compose.yaml restart otel-cc prometheus grafana

rebuild-otel-cc: ## otel-cc を再ビルドして再起動（コード変更後に実行）
	@OTEL_CONTAINER=$$(docker ps --filter "name=otel-cc" --format "{{.Names}}" | head -1); \
	if [ -z "$$OTEL_CONTAINER" ]; then echo "ERROR: otel-cc container not found"; exit 1; fi; \
	PROJECT=$$(docker inspect $$OTEL_CONTAINER --format '{{index .Config.Labels "com.docker.compose.project"}}'); \
	echo "[rebuild-otel-cc] project=$$PROJECT  container=$$OTEL_CONTAINER"; \
	docker-compose -p $$PROJECT -f .devcontainer/compose.yaml build otel-cc && \
	docker-compose -p $$PROJECT -f .devcontainer/compose.yaml up -d --no-deps otel-cc

logs-otel-cc: ## otel-cc のログを表示
	docker logs lifeai_devcontainer-otel-cc-1

logs-prometheus: ## Prometheus のログを表示
	docker logs lifeai_devcontainer-prometheus-1

logs-grafana: ## Grafana のログを表示
	docker logs lifeai_devcontainer-grafana-1

# ------------------------------------------------------------
# Git Hooks
# ------------------------------------------------------------
hooks: ## Git hooks インストール
	git config core.hooksPath "$$(pwd)/.githooks"
	chmod +x .githooks/pre-commit .githooks/pre-push .githooks/post-commit
	chmod +x .claude/scripts/update-claude-md.sh
	@echo "Git hooks installed to $$(pwd)/.githooks"

# ------------------------------------------------------------
# Dev Container
# ------------------------------------------------------------
init-firewall: ## ファイアウォール初期化（要 root）
	@if [ "$$(id -u)" -ne 0 ]; then \
		if command -v sudo >/dev/null 2>&1; then \
			sudo ./.devcontainer/init-firewall.sh; \
		else \
			echo "ERROR: init-firewall requires root; please run 'sudo make init-firewall'"; \
			exit 1; \
		fi; \
	else \
		./.devcontainer/init-firewall.sh; \
	fi
