# The binaries are built on ubuntu-24.04 runners (core-build-and-draft.yml), so this
# image must ship the same or a newer glibc: Debian 13 has 2.41, Ubuntu 24.04 2.39.
#
# The runtime base: distroless "cc" has glibc, libgcc_s, zlib, the CA bundle and tzdata,
# which is all the core and the Temurin JREs it downloads need, and no shell or package
# manager. TLS goes through rustls, so no OpenSSL (or SASL) libraries are needed.
FROM gcr.io/distroless/cc-debian13@sha256:809d04afa1be033c441f76bb0051878c342fb76d0ea236bc6dc94b655fa0fbd6 AS base

# The runtime image has no shell to make its user and folders, so they're made here, in
# the same Debian release. uid and gid 1001 are what `useradd` gave `user` in the earlier
# ubuntu:24.04 image (where `ubuntu` has 1000), which existing volumes are owned by. The
# passwd entry gives the core a user name for its default name ("user's Lodestone Core").
# The files are the same on every platform, so this runs on the build machine's.
FROM --platform=$BUILDPLATFORM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2 AS rootfs
COPY --from=base /etc/passwd /etc/group /rootfs/etc/
RUN echo 'user:x:1001:1001::/home/user:/sbin/nologin' >> /rootfs/etc/passwd \
  && echo 'user:x:1001:' >> /rootfs/etc/group \
  && mkdir -p /rootfs/app /rootfs/home/user/.lodestone

FROM base AS production-amd64
ENV binpath="./release/lodestone_core_linux_x86_64_*"

FROM base AS production-arm64
ENV binpath="./release/lodestone_core_linux_aarch64_*"

ARG TARGETARCH
FROM production-$TARGETARCH AS production
ARG lodestone_version

COPY --from=rootfs /rootfs/etc/passwd /rootfs/etc/group /etc/
COPY --from=rootfs --chown=1001:1001 /rootfs/home/user /home/user
COPY --from=rootfs --chown=1001:1001 /rootfs/app /app

# create and enter app directory
WORKDIR /app

# without a shell there is no `chmod`, so the mode is set as the file is copied
COPY --chmod=755 $binpath ./main

# specify default port
EXPOSE 16662

# tells the core it runs in this image, for `launched_by` in /api/v1/info
ENV LODESTONE_DOCKER=1

# where the core keeps its data (~/.lodestone); set here since there is no login to set it
ENV HOME=/home/user

USER 1001:1001

# specify persistent volume
VOLUME ["/home/user/.lodestone"]

# start lodestone_core
CMD ["./main"]
