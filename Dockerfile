FROM rust:1.40.0-slim-stretch

WORKDIR /gatekeeper

COPY ./gatekeeper     gatekeeper
COPY ./model          model
COPY ./Cargo.lock     .
COPY ./Cargo.toml     .
COPY ./rust-toolchain .

RUN cargo build

EXPOSE 1080

ENV RUST_LOG gatekeeper=info

ENTRYPOINT ["./target/debug/gatekeeperd"]

CMD []

