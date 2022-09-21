const parser = import('../pkg/index.js').then(p => p.default).then(p => {
  p.init()
  return p
})

interface Result<T> {
  Err: {
    message: string
  }
  Ok: T
}

interface Program {
  nodes: RootNode[]
}

interface RootNode {
  Collection: Collection
  Function: Function
}

interface Collection {}
interface Function {}

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

export async function generateJSFunction (func: Function): Promise<string> {
  return unwrap(JSON.parse((await parser).generate_js_function(JSON.stringify(func))))
}
