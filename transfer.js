const pkg = require("./pkg");
pkg.__wasm.init();

const Account = pkg.parse(`
collection Account {
    name: string;
    age: number!;
    balance: number;
    publicKey: string;

    @index([field, asc], field2);

    function transfer (a, b, amount) {
        if (a.publicKey != $auth.publicKey) throw error('invalid user');
        
        a.balance -= amount;
        b.balance += amount;
    }
}`);

console.log(JSON.stringify(JSON.parse(Account), "  ", 2));
