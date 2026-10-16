# The game server and the lobby, no game files inside (mount your own; see README "Hosting").
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -j 2 -p server -p master

FROM debian:bookworm-slim
COPY --from=build /src/target/release/server /src/target/release/master /usr/local/bin/
ENV BOOTLEG16_DIR=/game
WORKDIR /bootleg16
