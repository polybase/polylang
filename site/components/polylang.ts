import * as pkg from './pkg'
export * from './pkg'

export interface Inputs {
  init_params: any,
  params: any[],
  contract_name: string | null,
  fn: string,
}

pkg.init()

export function run(code: string, inputs: Inputs) {
  console.log(code, inputs)
  const program = pkg.compile(
    code,
    inputs.contract_name === "" ? null : inputs.contract_name,
    inputs.fn
<<<<<<< HEAD
  );
  let output = program.run(
    JSON.stringify(inputs.init_params),
=======
  )
  const output = program.run(
    JSON.stringify(null),
>>>>>>> 05ae6ff (Changes:)
    JSON.stringify(inputs.params),
    // true == generate a proof
    true
  )

  return output
}
