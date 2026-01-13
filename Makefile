DOCKER_USERNAME ?= rubtsov-stan
IMAGE_NAME ?= blazebee
REGISTRY ?= docker.io
GHCR_REGISTRY ?= ghcr.io/$(DOCKER_USERNAME)
CONFIG_PATH ?= config.example.toml

TYPES := minimal standard large
PLATFORMS := linux/amd64,linux/arm64

GREEN := \033[0;32m
YELLOW := \033[1;33m
NC := \033[0m

.PHONY: all docker docker-% docker-latest docker-local-minimal docker-local-standard docker-local-large run-local-% clean help

all: docker

docker: $(addprefix docker-,$(TYPES))

docker-%:
	@echo "$(GREEN)Building and pushing $(IMAGE_NAME):$* (multi-arch)$(NC)"
	docker buildx build \
		--platform $(PLATFORMS) \
		--build-arg CARGO_FEATURES=$* \
		--tag $(REGISTRY)/$(DOCKER_USERNAME)/$(IMAGE_NAME):$* \
		--tag $(REGISTRY)/$(DOCKER_USERNAME)/$(IMAGE_NAME):$*-$(shell git rev-parse --short HEAD) \
		--tag $(GHCR_REGISTRY)/$(IMAGE_NAME):$* \
		--tag $(GHCR_REGISTRY)/$(IMAGE_NAME):$*-$(shell git rev-parse --short HEAD) \
		--push .

docker-latest:
	@echo "$(GREEN)Building and pushing :latest (standard variant)$(NC)"
	docker buildx build \
		--platform $(PLATFORMS) \
		--build-arg CARGO_FEATURES=standard \
		--tag $(REGISTRY)/$(DOCKER_USERNAME)/$(IMAGE_NAME):latest \
		--tag $(REGISTRY)/$(DOCKER_USERNAME)/$(IMAGE_NAME):latest-$(shell git rev-parse --short HEAD) \
		--tag $(GHCR_REGISTRY)/$(IMAGE_NAME):latest \
		--tag $(GHCR_REGISTRY)/$(IMAGE_NAME):latest-$(shell git rev-parse --short HEAD) \
		--push .

docker-local-minimal:
	@echo "$(YELLOW)Building local image $(IMAGE_NAME):minimal (amd64 only)$(NC)"
	docker buildx build \
		--platform linux/amd64 \
		--build-arg CARGO_FEATURES=minimal \
		--tag $(IMAGE_NAME):minimal \
		--load .

docker-local-standard:
	@echo "$(YELLOW)Building local image $(IMAGE_NAME):standard (amd64 only)$(NC)"
	docker buildx build \
		--platform linux/amd64 \
		--build-arg CARGO_FEATURES=standard \
		--tag $(IMAGE_NAME):standard \
		--load .

docker-local-large:
	@echo "$(YELLOW)Building local image $(IMAGE_NAME):large (amd64 only)$(NC)"
	docker buildx build \
		--platform linux/amd64 \
		--build-arg CARGO_FEATURES=large \
		--tag $(IMAGE_NAME):large \
		--load .

run-local-%:
	@echo "$(GREEN)Running local $(IMAGE_NAME):$* with config: $(CONFIG_PATH)$(NC)"
	@if [ -f "$(CONFIG_PATH)" ]; then \
		echo "Using local config.toml"; \
		docker run --rm -it \
			--network host \
			-v $(abspath $(CONFIG_PATH)):/etc/blazebee/config.toml:ro \
			$(IMAGE_NAME):$*; \
	else \
		echo "Config file $(CONFIG_PATH) not found → using default in image"; \
		docker run --rm -it \
			--network host \
			$(IMAGE_NAME):$*; \
	fi

clean:
	@echo "$(YELLOW)Cleaning buildx cache...$(NC)"
	docker builder prune -f --all

help:
	@echo "Makefile for BlazeBee (musl + scratch, multi-arch)"
	@echo ""
	@echo " make docker                  -> Build and push all types (minimal/standard/large)"
	@echo " make docker-standard         -> Build and push 'standard'"
	@echo " make docker-latest           -> Build and push :latest (standard)"
	@echo ""
	@echo " make docker-local-minimal    -> Local build minimal (amd64, no push)"
	@echo " make docker-local-standard   -> Local build standard (amd64, no push)"
	@echo " make docker-local-large      -> Local build large (amd64, no push)"
	@echo " make run-local-standard      -> Run local standard image"
	@echo " make clean                   -> Clean buildx cache"
	@echo ""
	@echo "Variables:"
	@echo "  DOCKER_USERNAME=$(DOCKER_USERNAME)"
	@echo "  PLATFORMS=$(PLATFORMS)"
	@echo "  CONFIG_PATH=$(CONFIG_PATH) (default: config.example.toml)"