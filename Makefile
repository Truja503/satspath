# ─────────────────────────────────────────────────────────────────────────────
# SatsPath — Makefile
# Convenience targets for Docker build, run, and development workflows.
# ─────────────────────────────────────────────────────────────────────────────

.PHONY: help build build-cli build-bridge run shell up down logs clean scan

# ── Default ──────────────────────────────────────────────────────────────────
help: ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
	  awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ── Build ─────────────────────────────────────────────────────────────────────
build: build-cli build-bridge ## Build all Docker images

build-cli: ## Build the SatsPath CLI image
	docker build \
	  --target runtime \
	  --tag satspath-cli:latest \
	  --tag satspath-cli:$(shell git rev-parse --short HEAD 2>/dev/null || echo dev) \
	  --build-arg BUILDKIT_INLINE_CACHE=1 \
	  .

build-bridge: ## Build the ARK bridge image
	docker build \
	  --target runtime \
	  --tag satspath-ark-bridge:latest \
	  --tag satspath-ark-bridge:$(shell git rev-parse --short HEAD 2>/dev/null || echo dev) \
	  ark-bridge/

# ── Run ───────────────────────────────────────────────────────────────────────
run: ## Run a satspath CLI command (pass CMD=<args>, e.g. make run CMD="--help")
	docker compose run --rm satspath-cli $(CMD)

shell: ## Open a shell in the CLI container for debugging (overrides ENTRYPOINT)
	docker compose run --rm --entrypoint /bin/bash satspath-cli

# ── Compose ──────────────────────────────────────────────────────────────────
up: ## Start the ARK bridge service (background)
	docker compose --profile bridge up -d ark-bridge

down: ## Stop all services
	docker compose --profile bridge down

logs: ## Tail logs from all running services
	docker compose --profile bridge logs -f

# ── Security scan ────────────────────────────────────────────────────────────
scan: ## Run Trivy vulnerability scan on both images (requires trivy installed)
	@echo "==> Scanning satspath-cli:latest"
	trivy image --severity HIGH,CRITICAL satspath-cli:latest
	@echo ""
	@echo "==> Scanning satspath-ark-bridge:latest"
	trivy image --severity HIGH,CRITICAL satspath-ark-bridge:latest

# ── Dev helpers ───────────────────────────────────────────────────────────────
clean: ## Remove built images and dangling layers
	docker compose --profile bridge down --volumes --remove-orphans || true
	docker rmi satspath-cli:latest satspath-ark-bridge:latest 2>/dev/null || true
	docker image prune -f

init: ## Run satspath init inside the container (creates /data/.satspath)
	docker compose run --rm satspath-cli init

# ── Quick smoke-test ──────────────────────────────────────────────────────────
smoke: build ## Build then verify both images produce --help output
	@echo "==> CLI smoke test"
	docker run --rm satspath-cli:latest --help
	@echo ""
	@echo "==> Bridge smoke test (node version)"
	docker run --rm --entrypoint node satspath-ark-bridge:latest --version
