FROM ekidd/rust-musl-builder as builder
COPY . .
RUN ["cargo", "build" ,"--release"]

FROM scratch
WORKDIR /bin
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/apply-rules .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/filter-ntfs .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/gtfs2ntfs .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/kv12ntfs .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/merge-ntfs .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/merge-stop-areas .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/ntfs2gtfs .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/ntfs2ntfs .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/transfers .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/read-syntus-fares .
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/restrict-validity-period .
VOLUME /app/input
VOLUME /app/output
