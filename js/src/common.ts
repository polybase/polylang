export interface Result<T> {
  Err: {
    message: string
  }
  Ok: T
}

export function unwrap<T> (value: Result<T>): T {
  if (value.Err) {
    throw new Error(value.Err.message)
  }

  return value.Ok
}
