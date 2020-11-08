ENV RUSTC_VERSION=1.47
ENV COSMOGONY_VERSION=0.8.1
ENV MIMIRSBRUNN_VERSION=1.17.0
ENV SETTINGS=development

FROM rust:$RUSTC_VERSION-buster as builder

RUN USER=root cargo new --lib mimir_ingest

WORKDIR ./mimir_ingest

COPY ./Cargo.toml ./Cargo.toml

RUN apt-get update \
    && apt-get install -y sqlite3 \
    && apt-get install -y libzmq5-dev
RUN cargo build --lib --release
RUN rm src/*.rs

ADD . ./

RUN rm ./target/release/deps/mimir_ingest*

RUN cargo build --release

FROM osmwithoutborders/cosmogony:release-v$COSMOGONY_VERSION as cosmogony

FROM navitia/mimirsbrunn:v$MIMIRSBRUNN_VERSION as mimirsbrunn

FROM debian:buster-slim

RUN apt-get update \
    && apt-get install -y libzmq5-dev \
    && apt-get install -y libgeos-c1v5 libgeos-dev\
    && apt-get install -y netcat \
    && apt-get install -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC \
    RUN_MODE=development \
    APP_USER=appuser

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p /opt/mimir_ingest \
    && mkdir -p /etc/opt/mimir_ingest \
    && mkdir -p /var/opt/mimir_ingest \
    && mkdir -p /opt/mimirsbrunn \
    && mkdir -p /etc/opt/mimirsbrunn \
    && mkdir -p /opt/cosmogony

COPY --from=builder /mimir_ingest/target/release/service /opt/mimir_ingest/service
COPY --from=builder /mimir_ingest/config /etc/opt/mimir_ingest
COPY --from=builder /mimir_ingest/migrations /opt/mimir_ingest/migrations
COPY --from=builder /mimir_ingest/docker/entrypoint.sh /opt/mimir_ingest/entrypoint.sh

COPY --from=cosmogony /usr/bin/cosmogony /opt/cosmogony/cosmogony

COPY --from=mimirsbrunn /usr/bin/osm2mimir /opt/mimirsbrunn/osm2mimir
COPY --from=mimirsbrunn /usr/bin/cosmogony2mimir /opt/mimirsbrunn/cosmogony2mimir
COPY --from=mimirsbrunn /usr/bin/bano2mimir /opt/mimirsbrunn/bano2mimir
COPY --from=mimirsbrunn /usr/bin/openaddresses2mimir /opt/mimirsbrunn/openaddresses2mimir
COPY --from=mimirsbrunn /etc/mimirsbrunn/ /etc/opt/mimirsbrunn

RUN chown -R $APP_USER:$APP_USER /opt/mimir_ingest
RUN chown -R $APP_USER:$APP_USER /opt/cosmogony
RUN chown -R $APP_USER:$APP_USER /opt/mimirsbrunn

USER $APP_USER
WORKDIR /opt/mimir_ingest
CMD ["./entrypoint.sh"]
