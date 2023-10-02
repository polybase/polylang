import * as pkg from './pkg'
export * from './pkg'

export interface Inputs {
  params: any[],
  fn: string,
}

pkg.init();

export function run(code: string, inputs: Inputs) {
  console.log(code, inputs)
  let program = pkg.compile(
    code,
    null,
    inputs.fn
  );
  let output = program.run(
    JSON.stringify(null),
    JSON.stringify(inputs.params),
    // true == generate a proof
    true
  );

  return output;
}
