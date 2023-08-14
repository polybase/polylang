## Build

### Javascript

Install wasm-pack: `cargo install wasm-pack`

```bash
cd js
yarn build
```

### Go

```bash
cargo build --release
cp target/release/libpolylang.a go/parser/

cd go
go run .
```

## Compiling Polylang to Miden

You can use the `compile` binary to compile Polylang functions to Miden. Compile outputs the generated Miden assembly to stdout, you can pipe it to `miden-run` to run it.

Example:
```bash
cargo run --bin compile function:test <<<'function test(a: number) {
  return a;
}' | cargo run -p miden-run -- --advice-tape 123
Output: ProgramOutputs { stack: [123, 123, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], overflow_addrs: [0, 20, 22] }

## Test

```bash
cargo test && (cd parser && cargo test)
```

You can download and test that collections from the testnet still parse by running:

```bash
./pull-collections.sh && cargo test
```
