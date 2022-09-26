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
cp target/release/libspacetime_parser.a go/parser/

cd go
go run .
```
