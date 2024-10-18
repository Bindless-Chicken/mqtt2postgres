FROM rust:1.81-bullseye AS builder
WORKDIR /m2r
COPY . .
RUN cargo build --release

FROM rust:1.81-slim-bullseye
COPY --from=builder /m2r/target/release/mqtt2postgres /usr/local/bin/mqtt2postgres
CMD ["mqtt2postgres"]
