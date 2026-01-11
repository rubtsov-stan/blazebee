BINARY_NAME = blazebee
VERSION     ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
CARGO_FLAGS ?= --release

TYPE        ?= standard
CONFIG      ?= config.example.toml
CONFIG_PATH = $(abspath ${CONFIG})

ifeq ($(TYPE),minimal)
  FEATURES = minimal
else ifeq ($(TYPE),large)
  FEATURES = large
else ifeq ($(TYPE),no-mqtt)
  FEATURES = no-mqtt
else
  FEATURES = standard
endif

UNAME_M := $(shell uname -m)
ifeq ($(UNAME_M),x86_64)
  DETECTED_ARCH = amd64
  DETECTED_PLATFORM = linux/amd64
  DOCKER_TARGETARCH = amd64
else ifeq ($(UNAME_M),aarch64)
  DETECTED_ARCH = arm64
  DETECTED_PLATFORM = linux/arm64
  DOCKER_TARGETARCH = arm64
else ifeq ($(UNAME_M),arm64)
  DETECTED_ARCH = arm64
  DETECTED_PLATFORM = linux/arm64
  DOCKER_TARGETARCH = arm64
else
  DETECTED_ARCH = $(UNAME_M)
  DETECTED_PLATFORM = linux/$(UNAME_M)
  DOCKER_TARGETARCH = $(UNAME_M)
endif

ARCH ?= $(DETECTED_ARCH)
PLATFORM ?= $(DETECTED_PLATFORM)
TARGETARCH ?= $(DOCKER_TARGETARCH)

GREEN  := \033[0;32m
YELLOW := \033[0;33m
RED    := \033[0;31m
BLUE   := \033[0;34m
NC     := \033[0m

DOCKER_TAG = ${BINARY_NAME}:${TYPE}
DOCKER_TAG_ARCH = ${BINARY_NAME}:${TYPE}-${ARCH}
DOCKER_TAG_VERSION = ${BINARY_NAME}:${VERSION}-${TYPE}

.PHONY: all build run clean help

all: build

build:
	@echo "${GREEN}Building binary (${TYPE} / features: ${FEATURES})...${NC}"
	cargo build ${CARGO_FLAGS} --features "${FEATURES}"

run: build
	@echo "${GREEN}Running binary (${TYPE}) with config: ${CONFIG_PATH}...${NC}"
	@test -f "${CONFIG_PATH}" || (echo "${RED}Config file ${CONFIG_PATH} not found!${NC}" && exit 1)
	BLAZEBEE_CONFIG=${CONFIG_PATH} ./target/release/${BINARY_NAME}

clean:
	@echo "${YELLOW}Cleaning up...${NC}"
	cargo clean
	rm -f target/release/${BINARY_NAME}

.PHONY: docker docker-minimal docker-large docker-test

docker:
	@echo "${GREEN}Building Docker image (${TYPE}) for ${ARCH}...${NC}"
	@echo "${BLUE}Using config file: ${CONFIG_PATH}${NC}"
	@test -f "${CONFIG_PATH}" || (echo "${RED}Config file ${CONFIG_PATH} not found!${NC}" && exit 1)
	docker build \
		--platform ${PLATFORM} \
		--build-arg CARGO_FEATURES="${FEATURES}" \
		--build-arg TARGETARCH="${TARGETARCH}" \
		--build-arg CONFIG_FILE="${CONFIG}" \
		-t ${DOCKER_TAG} \
		-t ${DOCKER_TAG_VERSION} \
		.

docker-minimal:
	@echo "${GREEN}Building minimal Docker image for ${ARCH}...${NC}"
	@$(MAKE) docker TYPE=minimal

docker-large:
	@echo "${GREEN}Building large Docker image for ${ARCH}...${NC}"
	@$(MAKE) docker TYPE=large

docker-test:
	@echo "${GREEN}Testing Docker container (${TYPE})...${NC}"
	docker run --rm -it ${BINARY_NAME}:${TYPE} --version || \
	docker run --rm -it ${BINARY_NAME}:${TYPE} --help || \
	docker run --rm -it ${BINARY_NAME}:${TYPE}

.PHONY: docker-run docker-shell docker-inspect show-arch docker-check

docker-run:
	@echo "${GREEN}Running Docker container with config from host: ${CONFIG_PATH}...${NC}"
	@test -f "${CONFIG_PATH}" || (echo "${RED}Config file ${CONFIG_PATH} not found!${NC}" && exit 1)
	docker run --rm -it \
		--network host \
		-v "${CONFIG_PATH}":/etc/blazebee/config.toml:ro \
		${BINARY_NAME}:${TYPE}

docker-shell:
	docker run --rm -it --entrypoint /bin/sh ${BINARY_NAME}:${TYPE}

docker-inspect:
	@echo "${GREEN}Inspecting Docker image...${NC}"
	docker image inspect ${BINARY_NAME}:${TYPE} --format '{{.Architecture}}'
	docker image inspect ${BINARY_NAME}:${TYPE} --format '{{.Os}}'
	docker image inspect ${BINARY_NAME}:${TYPE} --format '{{.Config.Entrypoint}}'

docker-check:
	@echo "${GREEN}Checking binary in image...${NC}"
	docker run --rm --entrypoint /bin/sh ${BINARY_NAME}:${TYPE} -c "file /usr/local/bin/blazebee || file /bin/blazebee"
	docker run --rm --entrypoint /bin/sh ${BINARY_NAME}:${TYPE} -c "ldd /usr/local/bin/blazebee 2>/dev/null || echo 'Static binary'"
	@echo "${GREEN}Checking config file...${NC}"
	docker run --rm --entrypoint /bin/sh ${BINARY_NAME}:${TYPE} -c "ls -la /etc/blazebee/"

show-arch:
	@echo "${GREEN}Architecture information:${NC}"
	@echo "  System: ${UNAME_M}"
	@echo "  Detected: ${ARCH} (${PLATFORM})"
	@echo "  Docker target: ${TARGETARCH}"
	@echo "  Build type: ${TYPE}"
	@echo "  Features: ${FEATURES}"
	@echo "  Config file: ${CONFIG_PATH}"

help:
	@echo ""
	@echo "${GREEN}Makefile for blazebee${NC}"
	@echo ""
	@echo "Detected architecture: ${UNAME_M} -> ${ARCH}"
	@echo ""
	@echo "${YELLOW}Main commands:${NC}"
	@echo "  make build          -> build binary"
	@echo "  make run            -> build and run with ${CONFIG}"
	@echo "  make docker         -> build Docker image for ${ARCH}"
	@echo "  make docker-test    -> test running container"
	@echo "  make docker-run     -> run container with host config"
	@echo ""
	@echo "${YELLOW}Build types:${NC}"
	@echo "  make docker-minimal -> minimal image"
	@echo "  make docker-large   -> large image"
	@echo "  make TYPE=no-mqtt docker -> no-mqtt image"
	@echo ""
	@echo "${YELLOW}Debug commands:${NC}"
	@echo "  make docker-shell   -> open shell in container"
	@echo "  make docker-inspect -> inspect image details"
	@echo "  make docker-check   -> check binary compatibility"
	@echo "  make show-arch      -> show architecture info"
	@echo ""
	@echo "${YELLOW}Examples:${NC}"
	@echo "  # Build with custom config"
	@echo "  make docker CONFIG=myconfig.toml"
	@echo "  make run CONFIG=myconfig.toml"
	@echo ""
	@echo "  # Build and test"
	@echo "  make docker && make docker-test"
	@echo ""
	@echo "  # Run with host config file"
	@echo "  make docker-run CONFIG=custom.toml"
	@echo ""
	@echo "  # Specific architecture"
	@echo "  make docker ARCH=arm64 PLATFORM=linux/arm64 TARGETARCH=arm64"
	@echo ""
	@echo "${YELLOW}Variables:${NC}"
	@echo "  TYPE=${TYPE}          -> build type"
	@echo "  CONFIG=${CONFIG}      -> config file (default: config.example.toml)"
	@echo "  ARCH=${ARCH}          -> architecture"
	@echo "  PLATFORM=${PLATFORM}  -> docker platform"
	@echo "  TARGETARCH=${TARGETARCH} -> docker build arg"
	@echo ""

.DEFAULT_GOAL := help