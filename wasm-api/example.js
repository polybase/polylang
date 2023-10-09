const pkg = require("./pkg");

pkg.init();

function justMain() {
  let program = pkg.compile(
    "function main(x: string): string { log(x); return 'x: ' + x; }",
    null,
    "main"
  );
  let output = program.run(
    JSON.stringify(null),
    JSON.stringify(["hello world"]),
    // true == generate a proof
    true
  );

  return output;
}

function withContracts() {
  let program = pkg.compile(
    // If the log was absent, we wouldn't get `id` in the output,
    // because the compiler optimizes it away for performance
    "contract Account { id: string; function main() { log(this.id); } }",
    "Account",
    "main"
  );
  let output = program.run(
    JSON.stringify({ id: "test" }),
    JSON.stringify([]),
    true
  );

  return output;
}

function withArrays() {
  let program = pkg.compile(
    `contract ReverseArray {
        elements: number[];

        constructor (elements: number[]) {
            this.elements = elements;
        }

        function reverse(): number[] {
          let reversed: u32[] = [];
          let i: u32 = 0;
          let one: u32 = 1;
          let len: u32 = this.elements.length;

          while (i < len) {
            let idx: u32 = len - i - one;
            reversed.push(this.elements[idx]);
            i = i + one;
          }

          return reversed;
        }
    }`,
    "ReverseArray",
    "reverse"
  );
  let output = program.run(
    JSON.stringify({ elements: [1, 2, 3, 4, 5] }),
    JSON.stringify([]),
    true
  );

  return output;
}

function withCountryCity() {
  let program = pkg.compile(`
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
    `,
    "City",
    "constructor")

  let output = program.run(
    JSON.stringify({ id: "", name: "", country: { id: "", name: "" } }),
    JSON.stringify(["boston", "BOSTON", { id: "usa", name: "USA" }]),
    true
  );

  return output;
}

function report(output, hasThis) {
  return {
    proof: output.proof(),
    proofLength: output.proof().length,
    cycleCount: output.cycle_count(),
    this: hasThis ? output.this() : null,
    result: output.result(),
    resultHash: output.result_hash(),
    logs: output.logs(),
    hashes: output.hashes(),
    // selfDestructed: output.self_destructed(),
    readAuth: output.read_auth(),
  };
}

const mainOutput = justMain();
console.log(report(mainOutput, false));
console.log("Proof is valid?", mainOutput.verify());

const contractOutput = withContracts();
console.log(report(contractOutput, true));
console.log("Proof is valid?", contractOutput.verify());

const arraysOutput = withArrays();
console.log(report(arraysOutput, true));
console.log("Proof is valid?", arraysOutput.verify());

const countryCityOutput = withCountryCity();
console.log(report(countryCityOutput, true));
console.log("Proof is valid?", countryCityOutput.verify());

