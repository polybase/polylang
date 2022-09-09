## Build

### Javascript

Install wasm-pack: `cargo install wasm-pack`

```bash
wasm-pack build --target nodejs

node <<EOF
const pkg = require("./pkg");
pkg.__wasm.init(); // nicer error messages for panics

console.log(JSON.parse(pkg.parse("collection Test { name: string!; }")));
EOF
```

### Go

```bash
cargo build --release
cp target/release/libspacetime_parser.a go/parser/

(cd go && go run .)
```
