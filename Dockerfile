FROM kisiodigital/rust-ci:latest-proj7.2.1 as builder
WORKDIR /usr/src/app
COPY . ./
RUN git describe --tags --always && git status
RUN cargo build --workspace --release
RUN mkdir /usr/src/bin && for file in ls ${CARGO_TARGET_DIR:-./target}/release/*; do if test -f $file -a -x $file; then cp $file /usr/src/bin; fi; done

FROM kisiodigital/proj-ci:7.2.1
COPY --from=builder /usr/src/bin/* /usr/local/bin/
