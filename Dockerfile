FROM rust as build

# create and enter app directory
WORKDIR /app

# copy over project files
COPY . ./

# build app using 'release' profile
RUN cargo build --release --features "vendored-openssl"

FROM debian:bullseye-slim as production

#
RUN apt-get update \
  && apt-get install -y ca-certificates \
  && update-ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# create and enter app directory
WORKDIR /app

# copy over built app
COPY --from=build /app/target/release/main ./

# specify default port
EXPOSE 16662

RUN groupadd user && useradd -g user user

RUN mkdir -p /home/user/.lodestone
RUN chown user /app
RUN chown user /home/user/.lodestone

USER user

# specify persistent volume
VOLUME ["/home/user/.lodestone"]

# start lodestone_core
CMD ["./main"]
