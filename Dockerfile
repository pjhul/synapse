FROM rust:1.67 as builder
WORKDIR /usr/src/synapse
RUN apt-get update && apt-get install -y clang
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=target \
    cargo install --path . --bin "synapse"

FROM debian:bullseye-slim
RUN apt-get update && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/synapse /usr/local/bin/synapse
EXPOSE 8080
CMD ["synapse"]
