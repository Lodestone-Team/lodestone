# The binaries are built on ubuntu-24.04 runners (core-build-and-draft.yml), so this
# image must ship the same or a newer glibc.
FROM ubuntu:25.10@sha256:7cc5e35f6567ee8c66d2abb4aab0fd866669e6207c237c3a8f0947a5c7f17092 AS base

FROM base AS production-amd64
ENV binpath="./release/lodestone_core_linux_x86_64_*"

FROM base AS production-arm64
ENV binpath="./release/lodestone_core_linux_aarch64_*"

ARG TARGETARCH
FROM production-$TARGETARCH AS production
ARG lodestone_version

# TLS goes through rustls, so no OpenSSL (or SASL) libraries are needed.
RUN apt-get update \
  && apt-get install -y ca-certificates \
  && update-ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# RUN ldconfig

# RUN echo $LD_LIBRARY_PATH

# create and enter app directory
WORKDIR /app

COPY $binpath ./main

# specify default port
EXPOSE 16662

RUN chmod +x ./main

RUN groupadd user && useradd -g user user

RUN mkdir -p /home/user/.lodestone
RUN chown user /app
RUN chown user /home/user/.lodestone

USER user

# specify persistent volume
VOLUME ["/home/user/.lodestone"]

# start lodestone_core
CMD ["./main"]
