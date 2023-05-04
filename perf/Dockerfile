FROM rust:latest

WORKDIR /app

COPY . .

RUN cargo build --release --bin perf_concurrent_connections

CMD ["./target/release/perf_concurrent_connections"]

