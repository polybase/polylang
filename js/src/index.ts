import * as parser from '../pkg/spacetime_parser.js'

parser.init()

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
  Collection: any
  Function: any
}

interface Collection {}

function unwrap<T> (value: Result<T>): T {
  if (value.Err) {
    throw new Error(value.Err.message)
  }

  return value.Ok
}

export function parse (code: string): Program {
  return unwrap(JSON.parse(parser.parse(code)))
}

export function validateSet (collection: Collection, data: { [k: string]: any }): void {
  return unwrap(JSON.parse(parser.validate_set(JSON.stringify(collection), JSON.stringify(data))))
}
