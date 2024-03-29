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
    inputs.contract_name === '' ? null : inputs.contract_name,
    inputs.fn
  )

  let output = program.run(
    JSON.stringify(inputs.init_params),
    JSON.stringify(inputs.params),
    // true == generate a proof
    true
  )

  return output
}

// Server prover/verifier

export interface ServerOutput {
  old: {
    this: any,
    hashes: string[],
  },
  new: {
    selfDestructed: boolean,
    this: any,
    hashes: string[],
  },
  stack: {
    input: string[],
    output: string[],
    overflowAddrs: string[],
  },
  result?: {
    value: any,
    hash: string,
  },
  programInfo: string,
  proof: string,
  debug: {
    logs: string[],
  },
  cycleCount: number,
  proofLength: number,
  logs: any[],
  readAuth: boolean,
}

export function compile(code: string, inputs: Inputs) {
  let program = pkg.compile(
    code,
    inputs.contract_name === '' ? null : inputs.contract_name,
    inputs.fn
  )

  const midenCode = program.miden_code()

  const abiStringMatch = midenCode.match(/# ABI: (.+?)\n/)

  if (!abiStringMatch) {
    console.log('Could not extract abi from miden code')
    return null
  }

  const abiString = abiStringMatch[1]
  const abi = JSON.parse(abiString)
  return { midenCode: midenCode, abi: abi }
}