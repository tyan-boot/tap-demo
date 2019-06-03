FROM rust:latest as build

RUN cargo install cargo-build-deps

WORKDIR /build

RUN USER=root cargo new --bin tap-demo

WORKDIR /build/tap-demo

COPY Cargo.toml Cargo.lock ./

RUN cargo build-deps --release

COPY src /build/tap-demo/src

RUN cargo build --release

FROM ubuntu:latest

WORKDIR /code

COPY --from=build /build/tap-demo/target/release/tap-demo /code/tap-demo

RUN apt update && apt install -y iproute2 iputils-ping

ENTRYPOINT ["./tap-demo"]