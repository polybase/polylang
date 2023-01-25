import { unwrap } from './common'

const parser = import('../pkg/index.js').then(p => p.default).then(p => {
  p.init()
  return p
})

export interface Program {
  nodes: RootNode[]
}

export interface RootNode {
  Collection: Collection
  Function: Function
}

export type Collection = any
export type Function = any

export async function parse (code: string, namespace: string): Promise<[Program, any]> {
  return unwrap(JSON.parse((await parser).parse(code, namespace)))
}

export interface JSCollection {
  code: string
}

export async function generateJSCollection (collection: any): Promise<JSCollection> {
  return unwrap(JSON.parse((await parser).generate_js_collection(JSON.stringify(collection))))
}
