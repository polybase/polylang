# Cross-referencing Contracts Demo

This example simply runs the code for the following contract:

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
this_json: {"country":{"id":"usa"},"id":"boston","name":"BOSTON"}
Proof saved to city_country.proof
```

## LICENSE

This template is licensed under the [MIT License](../LICENSE.md).
