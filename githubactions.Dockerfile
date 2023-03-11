# syntax=docker/dockerfile:1
FROM debian:bullseye-slim as production

ARG binpath=./release/main

#
RUN apt-get update \
  && apt-get install -y ca-certificates libssl-dev libsasl2-dev \
  && update-ca-certificates \
  && rm -rf /var/lib/apt/lists/*

RUN ldconfig

RUN echo $LD_LIBRARY_PATH

# create and enter app directory
WORKDIR /app

COPY $binpath ./main

# specify default port
EXPOSE 16662

# specify persistent volume
VOLUME ["/root/.lodestone"]

# start lodestone_core
CMD ["./main"]
