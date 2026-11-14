# syntax=docker/dockerfile:1

FROM oraclelinux:8 AS instantclient
RUN dnf -y install oracle-instantclient-release-el8 \
    && (dnf -y install oracle-instantclient-basiclite || dnf -y install oracle-instantclient-basic) \
    && dnf clean all \
    && mkdir -p /opt/oracle-libs \
    && cp -a /usr/lib/oracle /opt/oracle-libs/oracle \
    && (cp -a /usr/lib64/libnsl.so.1* /opt/oracle-libs/ || true) \
    && (cp -a /lib64/libnsl.so.1* /opt/oracle-libs/ || true)

FROM rust:1.91.1-bookworm AS builder
WORKDIR /src
COPY api/Cargo.toml api/Cargo.lock api/openapi.yaml ./
COPY api/src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends libaio1 ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home wattly
COPY --from=instantclient /opt/oracle-libs/ /opt/oracle-libs/
RUN find /opt/oracle-libs -type d > /etc/ld.so.conf.d/wattly-oracle.conf \
    && ldconfig \
    && ldconfig -p | grep -q clntsh
COPY --from=builder /src/target/release/wattly-api /usr/local/bin/wattly-api
USER wattly
ENV WATTLY_BIND=0.0.0.0:8080
EXPOSE 8080
ENTRYPOINT ["wattly-api"]
