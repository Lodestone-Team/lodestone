# Build from the repository root, since core is a member of the Cargo workspace there:
#   docker build -f core/Dockerfile .
# The toolchain must match rust-toolchain.toml.

# The runtime base: distroless "cc" has glibc, libgcc_s, zlib, the CA bundle and tzdata,
# which is all the core and the Temurin JREs it downloads need, and no shell or package
# manager, so the image is about half the size of debian:trixie-slim plus apt. It is
# Debian 13, like the builder, so the binary's glibc requirement is met.
FROM gcr.io/distroless/cc-debian13@sha256:809d04afa1be033c441f76bb0051878c342fb76d0ea236bc6dc94b655fa0fbd6 AS runtime

FROM rust:1.99.1-trixie@sha256:8a4e90c7946b74626488b1446f712c0fe0ed1a596cbd3d871684bc9c5433ae07 AS build

# create and enter app directory
WORKDIR /app

# copy over the workspace (see Dockerfile.dockerignore)
COPY . ./

# make sure the toolchain from rust-toolchain.toml (and its components) is installed
RUN rustup toolchain install --profile minimal --no-self-update

# build app using 'release' profile, with packed relative relocations (DT_RELR): about
# 1.5 MB (7%) off the binary, and less for the loader to do at start. The binary then
# needs glibc 2.36, which the Debian 13 runtime has (2.41). Set here rather than in
# .cargo/config.toml, which every build reads: a developer's machine may have an older
# glibc. Debian 13's linkers (GNU ld 2.44, and rust-lld on x86_64) support it.
RUN RUSTFLAGS="-C link-arg=-Wl,-z,pack-relative-relocs" cargo build --locked --release -p lodestone_core

# The runtime image has no shell to make its user and folders, so they're made here.
# uid and gid 1000 are what `useradd` gave `user` in the earlier debian:trixie-slim image,
# which existing volumes are owned by. The passwd entry gives the core a user name for its
# default name ("user's Lodestone Core"), as before.
COPY --from=runtime /etc/passwd /etc/group /rootfs/etc/
RUN echo 'user:x:1000:1000::/home/user:/sbin/nologin' >> /rootfs/etc/passwd \
  && echo 'user:x:1000:' >> /rootfs/etc/group \
  && mkdir -p /rootfs/app /rootfs/home/user/.lodestone

FROM runtime AS production

COPY --from=build /rootfs/etc/passwd /rootfs/etc/group /etc/
COPY --from=build --chown=1000:1000 /rootfs/home/user /home/user
COPY --from=build --chown=1000:1000 /rootfs/app /app
COPY --from=build /app/target/release/lodestone_core /app/main

# create and enter app directory
WORKDIR /app

# specify default port
EXPOSE 16662

# tells the core it runs in this image, for `launched_by` in /api/v1/info
ENV LODESTONE_DOCKER=1

# where the core keeps its data (~/.lodestone); set here since there is no login to set it
ENV HOME=/home/user

USER 1000:1000

# specify persistent volume
VOLUME ["/home/user/.lodestone"]

# start lodestone_core
CMD ["./main"]
