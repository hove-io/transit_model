ARG PROJ_VERSION="6.1.0"

FROM rust:1-slim-stretch as builder
ARG PROJ_VERSION
ENV PROJ_DEB "proj_${PROJ_VERSION}_amd64.deb"
ENV DESTDIR "/build_proj"
ENV GPG_KEY "C60D758F807A525534C5DFD57B639E3638A8112A"

WORKDIR /usr/src/transit_model
RUN apt update && apt install --yes apt-transport-https gnupg2 wget
RUN wget --quiet --output-document - "https://kisiodigital.jfrog.io/kisiodigital/api/gpg/key/public" | apt-key add -
RUN echo "deb [arch=amd64] https://kisiodigital.jfrog.io/kisiodigital/debian-local stretch main" > /etc/apt/sources.list.d/kisio-digital.list
RUN apt update && apt install --yes pkg-config libssl-dev clang proj=${PROJ_VERSION}
COPY . ./
RUN cargo build --features=proj --release \
	&& mkdir /usr/src/bin_transit_model && for file in ls ./target/release/*; do if test -f $file -a -x $file; then cp $file /usr/src/bin_transit_model; fi; done \
	&& cd .. && rm -rf transit_model

FROM debian:stretch-slim
ARG PROJ_VERSION
ENV PROJ_DEB "proj_${PROJ_VERSION}_amd64.deb"
ENV GPG_KEY "C60D758F807A525534C5DFD57B639E3638A8112A"

VOLUME /app/input
VOLUME /app/output

RUN BUILD_DEPENDENCIES="apt-transport-https gnupg2 wget" \
	&& apt update \
	&& apt install --yes ${BUILD_DEPENDENCIES} \
	&& wget --quiet --output-document - "https://kisiodigital.jfrog.io/kisiodigital/api/gpg/key/public" | apt-key add - \
	&& echo "deb [arch=amd64] https://kisiodigital.jfrog.io/kisiodigital/debian-local stretch main" > /etc/apt/sources.list.d/kisio-digital.list \
	&& apt update \
	&& apt install --yes proj=${PROJ_VERSION} \
	&& apt purge --yes ${BUILD_DEPENDENCIES} \
	&& apt autoremove --yes \
	&& rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/bin_transit_model/* /usr/local/bin/
