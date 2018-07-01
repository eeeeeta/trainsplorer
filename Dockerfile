# ~~~ osm-signal's magic dockerfile ~~~

## BUILDER IMAGE: builds the entire monorepo
FROM base/archlinux:latest AS osms-builder

# update OS
RUN pacman -Syu --noconfirm
RUN pacman -S --needed --noconfirm base-devel

# install Rust: download rustup
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain none -y
# install Rust: download specific nightly toolchain
RUN ~/.cargo/bin/rustup install nightly-2018-06-10
RUN ~/.cargo/bin/rustup default nightly-2018-06-10

# The following bits build just the dependencies of the project, without the source-code itself.
# This is so that we can take advantage of docker's caching and not rebuild everything all the damn time :P

WORKDIR /osm-signal
# add docker_base.tar, which contains Cargo.lock and every Cargo.toml in the repository
ADD ./docker_base.tar /osm-signal/
# make dummy src/lib.rs files, to satisfy cargo
RUN /bin/bash -c 'find /osm-signal/* -type d -prune -exec mkdir {}/src \; -exec touch {}/src/lib.rs \;'
# disable incremental compilation (never going to be used, and bloats binaries)
ENV CARGO_INCREMENTAL=0
# build all the dependencies
RUN ~/.cargo/bin/cargo build --all
# remove the dummy src/ lib.rs files
RUN /bin/bash -c 'rm -rf /osm-signal/*/src'

# The following bits build the actual code.

# Copy in the source.
COPY . /osm-signal
# Delete all the dummy build artefacts that we made earlier when building dependencies.
# NB: this list should be updated as new crates are made.
RUN /bin/bash -c 'pkgs=("osms-db" "national-rail-departures" "osms-nrod" "ntrod-types" "osms-db-setup" "osms-web" "atoc-msn"); for thing in ${pkgs[@]}; do ~/.cargo/bin/cargo clean -p "$thing"; done'
# Actually build the damn code already :P
RUN ~/.cargo/bin/cargo build --all

## INDIVIDUAL IMAGES for each service
# These COPY the build artefacts from the builder container.

FROM debian:stable-slim AS osms-nrod
WORKDIR /osm-signal
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates
COPY --from=osms-builder /osm-signal/target/debug/osms-nrod /osm-signal
ENTRYPOINT ["/osm-signal/osms-nrod"]

FROM debian:stable-slim AS osms-web
WORKDIR /osm-signal
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates
COPY --from=osms-builder /osm-signal/target/debug/osms-web /osm-signal
ENTRYPOINT ["/osm-signal/osms-web"]

FROM debian:stable-slim AS osms-db-setup
WORKDIR /osm-signal
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates
COPY --from=osms-builder /osm-signal/target/debug/osms-db-setup /osm-signal
ENTRYPOINT ["/osm-signal/osms-db-setup"]
