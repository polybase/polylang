## Fibonacci

This example simply runs the code for the following contract:

```typescript
@public
contract Fibonacci {
    fibVal: u32;

    function main(p: u32, a: u32, b: u32) {
        for (let i: u32 = 0; i < p; i++) {
            let c = a.wrappingAdd(b);
            a = b;
            b = c;
        }

        this.fibVal = a;
    }
}
```

The contract provides a function `main`, which calculates the `p`th Fibonacci number, starting with base values 1 and 1.

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
this_json: {}
result_json: 34
Proof saved to fibonacci.proof
```

## LICENSE

This template is licensed under the [MIT License](LICENSE.md).
