## Accounts and Balances

This example simply runs the code for the following contract:

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

This examples demonstrates a simple account with facilities to deposit, withdraw, and report the balance.

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
this_json: {"balance":100,"id":"id1"}
this_json: {"balance":150,"id":""}
this_json: {"balance":125,"id":""}
this_json: {"balance":125,"id":""}
result_json: 125
```

## LICENSE

This template is licensed under the [MIT License](../LICENSE.md).
