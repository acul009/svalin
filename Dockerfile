FROM debian:bullseye-slim

RUN mkdir -p /var/lib/svalin/server
WORKDIR /var/lib/svalin/server

COPY ./target/release/svalin /usr/local/bin/svalin

EXPOSE 1234

ENTRYPOINT ["/usr/local/bin/svalin"]
CMD ["server", "0.0.0.0:1234"]