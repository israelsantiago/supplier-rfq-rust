FROM rust:1.100.0-bookworm AS builder
WORKDIR /workspace
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY tests ./tests
RUN cargo build --release --locked -p amri-stage5b2-wiremock-rs

FROM debian:bookworm-slim
COPY --from=builder /workspace/target/release/amri-stage5b2-wiremock-rs /usr/local/bin/amri-stage5b2-wiremock-rs
USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/amri-stage5b2-wiremock-rs"]
