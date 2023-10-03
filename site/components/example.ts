export const EXAMPLES = [{
  name: 'Add',
  code: `function main(a: u32, b: u32) {
  return a + b;
}`,
  inputs: `{
  "params": [10, 20],
  "fn": "main"
}`
}, {
  name: 'Fib',
  code: `function main(p: u32, a: u32, b: u32) {
  for (let i: u32 = 0; i < p; i++) {
    let c = a.wrappingAdd(b);
    a = b;
    b = c;
  }
}`,
  inputs: `{
  "params": [8, 1, 1],
  "fn": "main"
}`
}]