## Reverse Array

This example simply runs the code for the following contract:

```typescript
@public
contract ReverseArray {
    elements: number[];

    constructor (elements: number[]) {
        this.elements = elements;
    }

    reverse() {
      let reversed: u32[] = [];
      let i: u32 = 0;
      let one: u32 = 1;
      let len: u32 = this.elements.length;

      while (i < len) {
          let idx: u32 = len - i - one;
          reversed.push(this.elements[idx]);
          i = i + one;
      }

      this.elements = reversed;
    }
}
```

The contract provides a function `reverse` which reverses the numbers in the array `elements`.

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
this_json: {"elements":[1,3,4,5,7,6,2,3]}
result_json: [3,2,6,7,5,4,3,1]
Proof saved to reverse.proof
```

## LICENSE

This template is licensed under the [MIT License](../LICENSE.md).
