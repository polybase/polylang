const parser = import('../pkg/index.js').then(p => p.default).then(p => {
  p.init()
  return p
})

export interface Result<T> {
  Err: {
    message: string
  }
  Ok: T
}

export interface Program {
  nodes: RootNode[]
}

export interface RootNode {
  Contract: Contract
  Function: Function
}

export type Contract = any
export type Function = any

function unwrap<T> (value: Result<T>): T {
  if (value.Err) {
    throw new Error(value.Err.message)
  }

  return value.Ok
}

export async function parse (code: string): Promise<Program> {
  return unwrap(JSON.parse((await parser).parse(code)))
}

export async function validateSet (contract: Contract, data: { [k: string]: any }): Promise<void> {
  return unwrap(JSON.parse((await parser).validate_set(JSON.stringify(contract), JSON.stringify(data))))
}

export interface JSFunction {
  code: string
}

export async function generateJSFunction (func: Function): Promise<JSFunction> {
  return unwrap(JSON.parse((await parser).generate_js_function(JSON.stringify(func))))
}
