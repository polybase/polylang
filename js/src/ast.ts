export type Root = Node[]

export type Node = Collection

export interface Collection {
  kind: 'collection'
  namespace: Namespace
  name: string
  attributes: CollectionAttribute[]
}

export interface Namespace {
  kind: 'namespace',
  value: string
}

export type CollectionAttribute = Property | Index | Method | Directive

export interface Property {
  kind: 'property',
  name: string,
  type: Type
  directives: Directive[]
}

export interface Index {
  kind: 'index',
  fields: IndexField[]
}

export interface IndexField {
  direction: IndexFieldDirection
  fieldPath: string[]
}

export type IndexFieldDirection = 'asc' | 'desc'

export interface Method {
  kind: 'method'
  name: string
  code: string
  attributes: MethodAttribute[]
}

export type MethodAttribute = Parameter | ReturnValue | Directive

export interface Parameter {
  kind: 'parameter'
  name: string
  type: Type
  required: boolean
  directives: Directive[]
}

export type Type = Primitive

export interface Primitive {
  kind: 'primitive',
  value: 'string' | 'number' | 'boolean' | 'bytes'
}

export interface Array {
  kind: 'array',
  value: Type[]
}

export interface Map {
  kind: 'map',
  key: Type
  value: Type
}

export interface Object {
  kind: 'object',
  fields: ObjectField[]
}

export interface ObjectField {
  name: string
  type: Type
  required: boolean
}

export interface ForeignRecod {
  kind: 'foreignrecord',
  collection: string
}

export interface PublicKey {
  kind: 'publickey',
}

export interface Directive {
  kind: 'directive'
  name: string
  arguments: DirectiveArgument[]
}

export type DirectiveArgument = FieldReference

export interface FieldReference {
  kind: 'fieldreference',
  path: string[]
}

export interface ReturnValue {
  kind: 'returnvalue'
  name: string
  type: Type
}
