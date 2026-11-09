# Build from the repository root, since core is a member of the Cargo workspace there:
#   docker build -f core/Dockerfile .
# The toolchain must match rust-toolchain.toml.
FROM rust:1.99.1-trixie@sha256:8a4e90c7946b74626488b1446f712c0fe0ed1a596cbd3d871684bc9c5433ae07 AS build

# create and enter app directory
WORKDIR /app

# libclang is needed by bindgen (libsqlite3-sys, libnghttp2) in the deno dependency tree
RUN apt-get update \
  && apt-get install -y clang libclang-dev \
  && rm -rf /var/lib/apt/lists/*

# copy over the workspace (see Dockerfile.dockerignore)
COPY . ./

# make sure the toolchain from rust-toolchain.toml (and its components) is installed
RUN rustup toolchain install --profile minimal --no-self-update

# build app using 'release' profile
RUN cargo build --locked --release -p lodestone_core

# Same Debian release as the builder, so the binary's glibc requirement is met.
FROM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2 AS production

#
RUN apt-get update \
  && apt-get install -y ca-certificates \
  && update-ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# create and enter app directory
WORKDIR /app

# copy over built app
COPY --from=build /app/target/release/lodestone_core ./main

# specify default port
EXPOSE 16662

# tells the core it runs in this image, for `launched_by` in /api/v1/info
ENV LODESTONE_DOCKER=1

RUN groupadd user && useradd -g user user

RUN mkdir -p /home/user/.lodestone
RUN chown user /app
RUN chown user /home/user/.lodestone

USER user

# specify persistent volume
VOLUME ["/home/user/.lodestone"]

# start lodestone_core
CMD ["./main"]
