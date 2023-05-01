FROM rust:1.67 as builder
WORKDIR /usr/src/synapse
COPY . .
RUN cargo install --path .

FROM debian:bullseye-slim
RUN apt-get update && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/synapse /usr/local/bin/synapse
EXPOSE 8080
CMD ["synapse"]
