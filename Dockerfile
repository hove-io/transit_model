## Inspired by Docker image `kisiodigital/rust-ci:latest-proj` for `proj` installation
## See https://github.com/CanalTP/ci-images/blob/master/rust/proj/Dockerfile
ARG PROJ_VERSION="7.2.1"
# For running `proj`, the following Debian packages are needed:
# - 'clang' provides 'llvm-config', 'libclang.so' and 'stddef.h' needed for compiling 'proj-sys'
# - 'libtiff5' provides 'libtiff.so', needed for linking when 'proj-sys' is used
# - 'libcurl3-nss' provides 'libcurl-nss.so', needed for linking when 'proj-sys' is used
# - 'proj' provides 'proj.h' and 'libproj.so', needed for compiling 'proj-sys' (installed manually below)
ARG RUNTIME_DEPENDENCIES="clang libtiff5 libcurl3-nss"

FROM debian:stretch as proj-builder
ARG PROJ_VERSION
ARG RUNTIME_DEPENDENCIES
# For building `libproj' and the Rust's crate `proj-sys`, the following Debian packages are needed:
ENV BUILD_DEPENDENCIES="libcurl4-nss-dev libsqlite3-dev libtiff5-dev cmake pkg-config sqlite3 wget"
RUN apt update
RUN apt install --yes ${BUILD_DEPENDENCIES} ${RUNTIME_DEPENDENCIES}
RUN wget https://github.com/OSGeo/PROJ/releases/download/${PROJ_VERSION}/proj-${PROJ_VERSION}.tar.gz
RUN tar -xzvf proj-${PROJ_VERSION}.tar.gz
RUN mv proj-${PROJ_VERSION} /tmp/proj-src
WORKDIR /tmp/proj-src
RUN ./configure --prefix=/usr
RUN make -j$(nproc)
# copy all the installation files inside a temporary folder
# so they are copyable from the following stages of the Docker image
RUN make DESTDIR=/tmp/proj-build install

FROM debian:stretch as rust-builder
ARG RUNTIME_DEPENDENCIES
RUN apt update
RUN apt install --yes ${RUNTIME_DEPENDENCIES}
# install 'proj'
# we copy all the files to the root ('/tmp/proj-build' contains a 'usr/' folder)
COPY --from=proj-builder /tmp/proj-build /
WORKDIR /usr/src/app
COPY . ./
# install rustup
RUN apt install --yes curl
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH "/root/.cargo/bin:$PATH"
# build the project
RUN cargo build --workspace --release
RUN mkdir /usr/src/bin && for file in ls ./target/release/*; do if test -f $file -a -x $file; then cp $file /usr/src/bin; fi; done

FROM debian:stretch-slim
ARG RUNTIME_DEPENDENCIES
# install 'proj'
# We copy all the files to the root ('/tmp/proj-build' contains a 'usr/' folder)
COPY --from=proj-builder /tmp/proj-build /
RUN apt update \
    && apt install --yes ${RUNTIME_DEPENDENCIES} \
    && apt autoremove --yes \
    && rm -rf /var/lib/apt/lists/*
COPY --from=rust-builder /usr/src/bin/* /usr/local/bin/
