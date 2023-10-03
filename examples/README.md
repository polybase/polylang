# Polylang Showcase

These are some sample applications built using `Polylang`.

## Run 

All the examples are part of a `Cargo` workspace. To run a specific example (from anywhere inside the `examples` directory):

```bash
$ cargo run --release -p <package-name>
```

For instance:

```bash
$ cargo run --release -p hello-world
```

Output:

```bash
$ cargo run --release -p hello-world
    Finished release [optimized] target(s) in 0.89s
     Running `target/release/hello-world`
this_json: {"sum":3}
Proof saved to add.proof
```

## Examples

  * [Hello, world](hello-world/README.md)
  * [Fibonacci](fibonacci/README.md)
  * [Reverse an array](reverse-array/README.md)
  * [City and country](city-country/README.md)