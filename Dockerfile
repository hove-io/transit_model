ARG PROJ_VERSION="7.1.0"

FROM rust:1-slim-stretch as builder
ARG PROJ_VERSION
ENV PROJ_DEB "proj_${PROJ_VERSION}_amd64.deb"
ENV GPG_KEY "C60D758F807A525534C5DFD57B639E3638A8112A"
RUN apt update && apt install --yes apt-transport-https gnupg2 wget
RUN wget --quiet --output-document - "https://kisiodigital.jfrog.io/kisiodigital/api/gpg/key/public" | apt-key add -
RUN echo "deb [arch=amd64] https://kisiodigital.jfrog.io/kisiodigital/debian-local stretch main" > /etc/apt/sources.list.d/kisio-digital.list
RUN apt update && apt install --yes pkg-config libssl-dev clang libtiff-dev libcurl4-nss-dev proj=${PROJ_VERSION}

WORKDIR /usr/src/app
COPY . ./
RUN cargo build --workspace --release \
	&& mkdir /usr/src/bin && for file in ls ./target/release/*; do if test -f $file -a -x $file; then cp $file /usr/src/bin; fi; done \
	&& cd .. && rm -rf app

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
	&& apt install --yes libtiff-dev libcurl4-nss-dev proj=${PROJ_VERSION} \
	&& apt purge --yes ${BUILD_DEPENDENCIES} \
	&& apt autoremove --yes \
	&& rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/bin/* /usr/local/bin/
