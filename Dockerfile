# See docs/adr/0005-container-image.md.
# Works with the classic builder and BuildKit alike (no cache mounts).

ARG RUST_VERSION=1.98

FROM rust:${RUST_VERSION}-slim-trixie AS build
WORKDIR /app

# 1. Dependencies layer: rebuilt only when the manifest, lockfile or contract change.
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto ./proto
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && touch src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

# 2. Application layer: touch so cargo rebuilds the crate, not the cached stub.
COPY src ./src
RUN touch src/main.rs src/lib.rs \
    && cargo build --release --locked

# distroless/cc: glibc + libgcc only, no shell, runs as non-root.
FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=build /app/target/release/cisa /usr/local/bin/cisa
# Inside a container the server must listen on all interfaces to be reachable.
ENV CISA_ADDR=0.0.0.0:8080 \
    NO_COLOR=1
EXPOSE 8080
USER nonroot
ENTRYPOINT ["/usr/local/bin/cisa"]
