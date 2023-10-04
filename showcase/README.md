# Polylang Showcase

These are some sample applications demonstrating `Polylang`'s features.


## Run 

All examples are part of the same `Cargo` package. To run a specific example, pass in the example name:

```bash
$ cargo run --release --example  <example-name>
```

For instance:

```bash
$ cargo run --release --example hello_world
```

Output:

```bash
$ cargo run --release --example hello_world
this_json: {}
result_json: 3
Proof saved to add.proof
```

A demo run of each example is provided as well.


## Examples

### Hello, World

This introductory example simply runs the code for the following contract:

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

Demo:

```bash
$ cargo run --release --example hello_world
this_json: {}
result_json: 3
Proof saved to add.proof

```

### Fibonacci

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

Demo:

```bash
$ cargo run --release --example fibonacci
this_json: {}
result_json: 34
Proof saved to fibonacci.proof
```

### Reversing an Array

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

Demo:

```bash
$ cargo run --release --example reverse_array
this_json: {"elements":[1,3,4,5,7,6,2,3]}
result_json: [3,2,6,7,5,4,3,1]
Proof saved to reverse.proof
```

### Binary Search

```typescript
 contract BinarySearch {
     arr: i32[];
     found: boolean;
     foundPos: u32;

     constructor (arr: i32[]) {
         this.arr = arr;
     }

     function search(elem: i32) {
         let low: u32 = 0;
         let high: u32 = this.arr.length;
         let one: u32 = 1;
         let two: u32 = 2;

         while (low <= high) {
             let mid: u32 = low + high;
             mid = mid / two;

             if (this.arr[mid] < elem) {
                 low = mid + one;
             } else {
                 if (this.arr[mid] > elem) {
                     high = mid - one;
                 } else {
                     this.found = true;
                     this.foundPos = mid;
                     break;
                 }
             }
         }

         if (low > high) {
             this.found = false;
         }
     }
 }

```

The example showcases iterative Binary Search.

Demo:

```bash
$ cargo run --release --example binary_search
this_json: {"arr":[1,2,3,3,5,6,11],"found":false,"foundPos":0}
this_json: {"arr":[1,2,3,3,5,6,11],"found":true,"foundPos":4}
```

### City and Country 

```typescript
contract City {
    id: string;
    name: string;
    country: Country;

    constructor(id: string, name: string, country: Country) {
        this.id = id;
        this.name = name;
        this.country = country;
    }
}

contract Country {
    id: string;
    name: string;

    constructor (id: string, name: string) {
        this.id = id;
        this.name = name;
    }
}

```
We have a contract `City` which has a field, `country` of type `Country` (which is itself a contract). This example showcases how we can cross-reference contracts by creating an instance of
`City` with a reference to an instance of `Country`.

Demo:

```bash
$ cargo run --release --example city_country
this_json: {"country":{"id":"usa"},"id":"boston","name":"BOSTON"}
Proof saved to city_country.proof
```

### Accounts and Balances

```typescript
contract Account {
    id: string;
    balance: number;

    constructor(id: string, balance: number) {
        this.id = id;
        this.balance = balance;
    }

    deposit(amt: number) {
        this.balance = this.balance + amt;
    }

    withdraw(amt: number) {
        if (this.balance < 0) {
            error("Insufficient balance");
        }
        this.balance = this.balance - amt;
    }

    getBalance(): number {
        return this.balance;
    }
}
```

This example demonstrates a simple account with facilities to deposit, withdraw, and report the balance.

Demo:

```bash
$ cargo run --release --example accounts
this_json: {"balance":100,"id":"id1"}
this_json: {"balance":150,"id":""}
this_json: {"balance":125,"id":""}
this_json: {"balance":125,"id":""}
result_json: 125

```


## Licensing

All examples are licensed under the [MIT License](LICENSE.md).

