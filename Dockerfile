# Multi-stage build for the cloud gateway binary.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /src/target/release/placenet-cloud-gateway /usr/local/bin/placenet-cloud-gateway
# The DB lives on a mounted volume (see compose); default to /data.
ENV GATEWAY_DATABASE_URL=sqlite:///data/placenet_gateway.db
ENTRYPOINT ["placenet-cloud-gateway"]
CMD ["serve"]
