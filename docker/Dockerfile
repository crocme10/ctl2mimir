FROM rust:1.44.1 as builder

RUN USER=root cargo new --lib ctl2mimir
WORKDIR ./ctl2mimir
COPY ./Cargo.toml ./Cargo.toml
RUN apt-get update \
    && apt-get install -y sqlite3 \
    && apt-get install -y libzmq5-dev
RUN cargo build --lib --release
RUN rm src/*.rs

ADD . ./

RUN rm ./target/release/deps/ctl2mimir*
RUN cargo build --release
RUN sqlite3 ctl2mimir.db < indexes.sql

FROM debian:buster-slim
ARG APP=/usr/src/app

RUN apt-get update \
    && apt-get install -y libzmq5-dev \
    && apt-get install -y sqlite3 \
    && apt-get install -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*


EXPOSE 8080

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p ${APP}

COPY --from=builder /ctl2mimir/target/release/server ${APP}/server
COPY --from=builder /ctl2mimir/ctl2mimir.db ${APP}/ctl2mimir.db

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

CMD ["./server", "-u", "sqlite://ctl2mimir.db"]
