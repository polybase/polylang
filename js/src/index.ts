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
  Collection: Collection
  Function: Function
}

export type Collection = any
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

export async function validateSet (collection: Collection, data: { [k: string]: any }): Promise<void> {
  return unwrap(JSON.parse((await parser).validate_set(JSON.stringify(collection), JSON.stringify(data))))
}

export interface JSCollection {
  code: string
}

export async function generateJSCollection (collection: Collection): Promise<JSCollection> {
  return unwrap(JSON.parse((await parser).generate_js_collection(JSON.stringify(collection))))
}
