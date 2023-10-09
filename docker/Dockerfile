FROM rust:1-bookworm AS builder

WORKDIR /build

COPY Cargo.lock ./
COPY Cargo.toml ./
COPY server ./server/
COPY server-routes ./server-routes/
COPY prover ./prover/
COPY abi ./abi
COPY error ./error/
COPY miden-run ./miden-run/
COPY tests ./tests/
COPY wasm-api ./wasm-api/
COPY src ./src/
COPY parser ./parser/

RUN cd server && cargo build --release

# Server
FROM debian:bookworm-slim AS server

RUN apt-get update && apt-get install -y openssl ca-certificates

COPY --from=builder /build/target/release/server /usr/local/bin/server

CMD ["server"]
