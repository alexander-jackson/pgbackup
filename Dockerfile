FROM lukemathwalker/cargo-chef:0.1.67-rust-1.80.0-alpine3.20 AS chef
WORKDIR /app

FROM chef AS planner
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

# Install anything we need for `musl` builds and set the environment variables
RUN apk add --no-cache musl build-base clang llvm18
RUN rustup target add x86_64-unknown-linux-musl

ENV CC_x86_64_unknown_linux_musl=clang
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-Clink-self-contained=yes -Clinker=rust-lld"

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./src ./src

RUN cargo build --release --target x86_64-unknown-linux-musl --bin pgbackup

FROM alpine:3.20
RUN apk add --no-cache postgresql-client
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/pgbackup .
ENTRYPOINT ["./pgbackup"]
