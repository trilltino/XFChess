# syntax=docker/dockerfile:1.7

FROM rust:1.96-slim-bookworm AS builder

WORKDIR /workspace

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    build-essential \
    ca-certificates \
    clang \
    curl \
    git \
    libclang-dev \
    libssl-dev \
    libudev-dev \
    make \
    perl \
    pkg-config \
    zlib1g-dev \
  && rm -rf /var/lib/apt/lists/*

RUN curl -sSfL https://release.solana.com/v3.1.12/install | sh -s -- --bin-dir /usr/local/bin

ENV PATH="/root/.cargo/bin:/usr/local/bin:${PATH}"

RUN cargo install anchor-cli --version 1.1.2 --locked

RUN anchor --version && solana --version

WORKDIR /workspace
CMD ["/bin/bash"]
