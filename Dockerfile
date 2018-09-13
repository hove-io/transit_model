FROM ekidd/rust-musl-builder as builder
COPY . .
RUN ["cargo", "build" ,"--release"]

FROM scratch
WORKDIR /bin
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/gtfs2ntfs .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/transfers .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/merge-ntfs .
VOLUME /app/input
VOLUME /app/output
