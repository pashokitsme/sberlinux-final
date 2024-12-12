FROM rust:slim-bookworm AS builder

WORKDIR /src/vpn
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
  libssl3 \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/vpn/target/release/vpn-server /bin/vpn-server

ENTRYPOINT ["vpn-server", "--config", "/etc/vpn/config.yml"]
