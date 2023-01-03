## Build

### Javascript

Install wasm-pack: `cargo install wasm-pack`

```bash
cd js
yarn build
EOF
```

### Go

```bash
cargo build --release
cp target/release/libpolylang.a go/parser/

cd go
go run .
```

## Test

```bash
cargo test && (cd parser && cargo test)
```

You can download and test that collections from the testnet still parse by running:

```bash
./pull-collections.sh && cargo test
```
