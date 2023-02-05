FROM rust as build

WORKDIR app
COPY . ./
RUN cargo build --release

FROM rust as final

WORKDIR /app
COPY --from=build /app/target/release/main ./
EXPOSE 16662
VOLUME ["/root/.lodestone"]

CMD ["./main"]