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

.PHONY: all docker docker-% docker-latest docker-local-% run-local-% clean help

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

run-local-%:
	@echo "$(GREEN)Running local $(IMAGE_NAME):$* with config: $(CONFIG_PATH)$(NC)"
	@if [ -f "$(CONFIG_PATH)" ]; then \
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

run-local-%:
	@echo "$(GREEN)Running local $(IMAGE_NAME):$*$(NC)"
	docker run --rm -it \
		--network host \
		-v $(CURDIR)/config.example.toml:/etc/blazebee/config.toml:ro \
		$(IMAGE_NAME):$*

clean:
	@echo "$(YELLOW)Cleaning buildx cache...$(NC)"
	docker builder prune -f --all

help:
	@echo "Makefile for BlazeBee (musl + scratch, multi-arch)"
	@echo ""
	@echo "  make docker                  -> Build and push all types"
	@echo "  make docker-standard         -> Build and push 'standard' type"
	@echo "  make docker-latest           -> Build and push:latest (standard)"
	@echo "  make docker-local-standard   -> Local build (amd64)"
	@echo "  make run-local-standard      -> Run local image (amd64)"
	@echo "  make clean                   -> Clean buildx cache"
	@echo ""
	@echo "Variables:"
	@echo "  DOCKER_USERNAME=$(DOCKER_USERNAME)"
	@echo "  PLATFORMS=$(PLATFORMS)"