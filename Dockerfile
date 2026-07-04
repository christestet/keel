# Try Keel with no local Rust/Go install:
#   docker build -t keel .
#   docker run --rm keel                                  # runs examples/hello.keel
#   docker run --rm -v "$PWD":/work -w /work keel run my.keel
#   docker run --rm -it --entrypoint bash keel             # explore the toolchain
#
# `keel run|build|test` shell out to the Go toolchain (see docs/getting-started.md),
# so the final image keeps a full Go install, not just the compiled binaries.

FROM rust:1-bookworm AS builder
WORKDIR /src
COPY . .
RUN cargo build --release -p keelc-driver

FROM golang:1.22-bookworm
COPY --from=builder /src/target/release/keel /src/target/release/keelc /usr/local/bin/
COPY --from=builder /src/examples /keel/examples
WORKDIR /keel
ENTRYPOINT ["keel"]
CMD ["run", "examples/hello.keel"]
