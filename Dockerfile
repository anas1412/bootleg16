# The game server and the lobby, no game files inside (mount your own; see README "Hosting").
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -j 2 -p server && cargo build --release -j 2 -p lobby --features steam \
 && cp target/release/libsteam_api.so /src/

FROM debian:bookworm-slim
# Steam's client library (steamclient.so), which the lobby needs to check Steam logins:
# installed with Valve's SteamCMD, run once to fetch it.
RUN dpkg --add-architecture i386 && apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl lib32gcc-s1 \
 && mkdir -p /opt/steamcmd && curl -fsSL https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz | tar xz -C /opt/steamcmd \
 && /opt/steamcmd/steamcmd.sh +quit \
 && mkdir -p /opt/steamhome/.steam/sdk64 && ln -s /opt/steamcmd/linux64/steamclient.so /opt/steamhome/.steam/sdk64/steamclient.so \
 && chmod -R a+rX /opt/steamcmd && chmod 1777 /opt/steamhome \
 && apt-get purge -y curl && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/libsteam_api.so /usr/local/lib/
RUN ldconfig
COPY --from=build /src/target/release/server /src/target/release/lobby /usr/local/bin/
# The servers run as the host user (see host.sh), not root; Steam's library looks in $HOME.
ENV BOOTLEG16_DIR=/game HOME=/opt/steamhome
WORKDIR /bootleg16
