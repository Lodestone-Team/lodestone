FROM rust as build

# create and enter app directory
WORKDIR /app

# copy over project files
COPY . ./

# build app using 'release' profile
RUN cargo build --release

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

# specify persistent volume
VOLUME ["/root/.lodestone"]

# start lodestone_core
CMD ["./main"]
