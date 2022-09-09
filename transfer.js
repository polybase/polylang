const pkg = require("./pkg");
pkg.__wasm.init();

const Account = `
collection Account {
    name: string;
    age: number!;
    balance: number;
    publicKey: string;

    @index([field, asc], field2);

    function transfer (a, b, amount) {
        if (a.publicKey != auth.publicKey) throw error('invalid user');
        
        a.balance -= amount;
        b.balance += amount;
    }
}`;

const result = pkg.interpret(
  Account,
  "Account",
  "transfer",
  JSON.stringify({
    auth: {
      value: {
        Map: {
          publicKey: { value: { String: "0x123" } },
        },
      },
    },
    a: {
      value: {
        Map: {
          publicKey: { value: { String: "0x123" } },
          balance: { value: { Number: 100 } },
        },
      },
    },
    b: {
      value: {
        Map: {
          balance: { value: { Number: 100 } },
        },
      },
    },
    amount: {
      value: {
        Number: 10,
      },
    },
  })
);

console.log(JSON.stringify(JSON.parse(result), "  ", 2));
