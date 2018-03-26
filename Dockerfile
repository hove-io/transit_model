FROM ekidd/rust-musl-builder as builder
COPY . .
RUN ["cargo", "build" ,"--release"]

FROM scratch
WORKDIR /app
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/gtfs2ntfs .
VOLUME /app/input
VOLUME /app/output

ENTRYPOINT ["./gtfs2ntfs", "-i", "./input", "-o", "./output"]
