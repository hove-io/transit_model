FROM rust:1-slim-stretch as builder
WORKDIR /usr/src/transit_model
ARG DESTDIR="/build_proj"
ARG PROJ_VERSION="6.1.0"
RUN apt update && apt install -y wget build-essential pkg-config sqlite3 libsqlite3-dev libssl-dev clang \
	&& wget https://github.com/OSGeo/proj.4/releases/download/$PROJ_VERSION/proj-$PROJ_VERSION.tar.gz \
	&& tar -xzvf proj-$PROJ_VERSION.tar.gz \
	&& cd proj-$PROJ_VERSION \
	&& ./configure --prefix=/usr \
	&& make \
	&& make install \
	&& cd .. && rm -rf proj-$PROJ_VERSION proj-$PROJ_VERSION.tar.gz \
	&& apt-get purge -y wget build-essential sqlite3 libsqlite3-dev \
	&& apt-get autoremove -y \
	&& rm -rf /var/lib/apt/lists/* \
	&& cp -r /build_proj/usr/include/* /usr/include \
	&& cp -r /build_proj/usr/lib/* /usr/lib
COPY . ./
RUN cargo build --features=proj4 --release \
	&& mkdir /usr/src/bin_transit_model && for file in ls ./target/release/*; do if test -f $file -a -x $file; then cp $file /usr/src/bin_transit_model; fi; done \
	&& cd .. && rm -rf transit_model

FROM debian:stretch-slim
VOLUME /app/input
VOLUME /app/output
RUN apt update && apt install -y  libssl-dev libsqlite3-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder  /build_proj/usr/share/proj/ /usr/share/proj/
COPY --from=builder  /build_proj/usr/include/ /usr/include/
COPY --from=builder  /build_proj/usr/lib/ /usr/lib/
COPY --from=builder /usr/src/bin_transit_model/* /usr/local/bin/
