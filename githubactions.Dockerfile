FROM debian:bullseye-slim as base

FROM base AS production-amd64
ENV binpath "./release/lodestone_core_"

FROM base as production-arm64
ENV binpath "./release/lodestone_core_arm_"

ARG TARGETARCH
FROM production-$TARGETARCH AS production
ARG lodestone_version

#
RUN apt-get update \
  && apt-get install -y ca-certificates libssl-dev libsasl2-dev \
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

RUN groupadd -r user && useradd -r -g user user

RUN mkdir -p /home/user/.lodestone
RUN chown user /app
RUN chown user /home/user/.lodestone

USER user

# specify persistent volume
VOLUME ["/home/user/.lodestone"]

# start lodestone_core
CMD ["./main"]