## Hello, World

This example simply runs the code for the following contract:

```typescript
@public
contract HelloWorld {
    sum: i32;

    function add(a: i32, b: i32) {
       this.sum = a + b;
    }
}
```

The contract provides a function `add`, which takes in two integers, and adds their values, storing the result in the field `sum`.

## Build and Run

```bash
$ cargo run --release
```

## Demo

```bash
$ cargo run --release
```

Output:

```bash
this_json: {"sum":3}
Proof saved to add.proof
```

## LICENSE

This template is licensed under the [MIT License](LICENSE.md).
