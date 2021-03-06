## Building Stage ##
FROM messense/rust-musl-cross:x86_64-musl as builder
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /build
# We'll get to what this file is below!
RUN mkdir src
RUN echo "fn main() {}" > dummy.rs
# If this changed likely the Cargo.toml changed so lets trigger the
# recopying of it anyways
COPY Cargo.lock .
COPY Cargo.toml .
# We'll get to what this substitution is for but replace main.rs with
# lib.rs if this is a library
RUN sed -i 's%src/main.rs%dummy.rs%' Cargo.toml
# Drop release if you want debug builds. This step cache's our deps!
RUN cargo build --release --target x86_64-unknown-linux-musl
# Now return the file back to normal
RUN sed -i 's%dummy.rs%src/main.rs%' Cargo.toml
ADD . .
RUN cargo build --release --target x86_64-unknown-linux-musl


## Running stage ##
FROM amd64/alpine:3
EXPOSE 4000
WORKDIR /data
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/potatosync-files .
ENV ADDRESS=127.0.0.1:4000
CMD ["./potatosync-files"]
