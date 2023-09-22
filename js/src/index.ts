import { unwrap } from './common'
export * as AST from './ast'

const parser = import('../pkg/index.js').then(p => p.default).then(p => {
  p.init()
  return p
})

export interface Program {
  nodes: RootNode[]
}

export interface RootNode {
  Contract: Contract
  Function: Function
}

export type Contract = any
export type Function = any

export async function parse(code: string, namespace: string): Promise<[Program, any]> {
  return unwrap(JSON.parse((await parser).parse(code, namespace)))
}

export interface JSContract {
  code: string
}

export async function generateJSContract(contract: any): Promise<JSContract> {
  return unwrap(JSON.parse((await parser).generate_js_contract(JSON.stringify(contract))))
}
