FROM rust:bullseye AS builder

RUN mkdir /build
WORKDIR /build

# only needed to not crash workspace
COPY ./svalin_iced ./svalin_iced

COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./svalin_macros ./svalin_macros
COPY ./marmelade ./marmelade
COPY ./svalin_pki ./svalin_pki
COPY ./svalin_rpc ./svalin_rpc
COPY ./svalin_sysctl ./svalin_sysctl
COPY ./svalin ./svalin
WORKDIR /build

RUN cargo build --release

FROM debian:bullseye-slim

RUN mkdir -p /var/lib/svalin/server
WORKDIR /var/lib/svalin/server

COPY --from=builder /build/target/release/svalin /usr/local/bin/svalin

EXPOSE 1234

ENTRYPOINT ["/usr/local/bin/svalin"]
CMD ["server", "0.0.0.0:1234"]