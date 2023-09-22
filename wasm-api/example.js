const pkg = require("./pkg");

pkg.init();

function justMain() {
  let program = pkg.compile(
    "function main(x: string) { log(x); }",
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

function withCollection() {
  let program = pkg.compile(
    // If the log was absent, we wouldn't get `id` in the output,
    // because the compiler optimizes it away for performance
    "@public collection Account { id: string; function main() { log(this.id); } }",
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

function report(output, hasThis) {
  return {
    proofLength: output.proof().length,
    cycleCount: output.cycle_count(),
    this: hasThis ? output.this() : null,
    logs: output.logs(),
    hashes: output.hashes(),
    selfDestructed: output.self_destructed(),
    readAuth: output.read_auth(),
  };
}

console.log(report(justMain(), false));
console.log(report(withCollection(), true));
