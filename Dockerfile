# trainsplorer dockerfile (oh dear)

## BUILDER IMAGE: builds the entire monorepo
FROM archlinux/base:latest AS tspl-builder

# update OS
RUN pacman -Syu --noconfirm
RUN pacman -S --needed --noconfirm base-devel

# install Rust: download rustup
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain stable -y

# The following bits build just the dependencies of the project, without the source-code itself.
# This is so that we can take advantage of docker's caching and not rebuild everything all the damn time :P

WORKDIR /tspl
# add all the cargo files, to get deps

RUN /bin/bash -c 'pkgs=("osms-db" "osms-darwin" "darwin-types" "osms-nrod" "ntrod-types" "osms-db-setup" "osms-web" "atoc-msn"); for thing in ${pkgs[@]}; do mkdir "/tspl/$thing"; done'
ADD ./osms-db/Cargo.toml /tspl/osms-db/
ADD ./osms-db-setup/Cargo.toml /tspl/osms-db-setup/
ADD ./osms-darwin/Cargo.toml /tspl/osms-darwin/
ADD ./osms-nrod/Cargo.toml /tspl/osms-nrod/
ADD ./osms-web/Cargo.toml /tspl/osms-web/
ADD ./atoc-msn/Cargo.toml /tspl/atoc-msn/
ADD ./ntrod-types/Cargo.toml /tspl/ntrod-types/
ADD ./darwin-types/Cargo.toml /tspl/darwin-types/
ADD ./Cargo.lock /tspl/
ADD ./Cargo.toml /tspl/
# make dummy src/lib.rs files, to satisfy cargo
RUN /bin/bash -c 'find /tspl/* -type d -prune -exec mkdir {}/src \; -exec touch {}/src/lib.rs \;'
# disable incremental compilation (never going to be used, and bloats binaries)
ENV CARGO_INCREMENTAL=0
# build all the dependencies
RUN ~/.cargo/bin/cargo build --all-targets --release
# remove the dummy src/ lib.rs files
RUN /bin/bash -c 'rm -rf /tspl/*/src'

# The following bits build the actual code.

# Copy in the source.
COPY . /tspl
# Delete all the dummy build artefacts that we made earlier when building dependencies.
# NB: this list should be updated as new crates are made.
RUN ~/.cargo/bin/cargo clean --release -p osms-db -p osms-darwin -p darwin-types -p osms-nrod -p ntrod-types -p osms-db-setup -p osms-web -p atoc-msn
# Actually build the damn code already :P
RUN ~/.cargo/bin/cargo build --all-targets --release

## INDIVIDUAL IMAGES for each service
# These COPY the build artefacts from the builder container.

FROM debian:stable-slim AS osms-nrod
WORKDIR /tspl
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates
COPY --from=tspl-builder /tspl/target/release/osms-nrod /tspl
ENTRYPOINT ["/tspl/osms-nrod"]

FROM debian:stable-slim AS osms-web
WORKDIR /tspl
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates
COPY --from=tspl-builder /tspl/target/release/osms-web /tspl
ENTRYPOINT ["/tspl/osms-web"]

FROM debian:stable-slim AS osms-db-setup
WORKDIR /tspl
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates
COPY --from=tspl-builder /tspl/target/release/osms-db-setup /tspl
ENTRYPOINT ["/tspl/osms-db-setup"]
