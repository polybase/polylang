export const EXAMPLES = [{
  name: 'Hello, World!',
  code: ` function hello() {
    log("Hello, world!");
}`,
  inputs: `{
  "init_params": "",
  "params": [],
  "contract_name": "",
  "fn": "hello"
}`,
}, {
  name: 'Addition',
  code: `function main(a: u32, b: u32): u32 {
  return a + b;
}`,
  inputs: `{
  "init_params": "",
  "params": [10, 20],
  "contract_name": "",
  "fn": "main"
}`,
}, {
  name: 'Fibonacci',
  code: `function main(p: u32, a: u32, b: u32) {
  for (let i: u32 = 0; i < p; i++) {
    let c = a.wrappingAdd(b);
    a = b;
    b = c;
  }
}`,
  inputs: `{
  "init_params": "",
  "params": [8, 1, 1],
  "contract_name": "",
  "fn": "main"
}`,
}, {
  name: 'Hello, Contracts!',
  code: `contract HelloContracts {
  function hello() {
    log("Hello, contracts!");
  }
}`,
  inputs: `{
  "init_params": "",
  "params": [],
  "contract_name": "HelloContracts",
  "fn": "hello"
}`,
}, {
  name: 'Reverse Array',
  code: `contract ReverseArray {
  elements: number[];

  constructor (elements: number[]) {
    this.elements = elements;
  }

  function reverse(): number[] {
    let reversed: u32[] = [];
    let i: u32 = 0;
    let one: u32 = 1;
    let len: u32 = this.elements.length;

    while (i < len) {
      let idx: u32 = len - i - one;
      reversed.push(this.elements[idx]);
      i = i + one;
    }

    return reversed;
  }
}
`,
  inputs: `{
  "init_params": { "elements": [1, 2, 3, 4, 5] },
  "params": [],
  "contract_name": "ReverseArray",
  "fn": "reverse"
}`,
}, {
  name: 'Binary Search',
  code: `contract BinarySearch {
  arr: i32[];
  found: boolean; // indicates whether element was found
  foundPos: u32; // gives the position of the found element

  constructor (arr: i32[]) {
    this.arr = arr;
  }

  function search(elem: i32): map<string, u32> {
    let low: u32 = 0;
    let high: u32 = this.arr.length;
    let one: u32 = 1;
    let two: u32 = 2;

    while (low <= high) {
      let mid: u32 = low + high;
      mid = mid / two;

      if (this.arr[mid] < elem) {
        low = mid + one;
      } else {
        if (this.arr[mid] > elem) {
          high = mid - one;
        } else {
          this.found = true; // found
          this.foundPos = mid;
          break;
        }
      }
    }
  }
}
`,
  inputs: `{
  "init_params" : { "arr": [1, 2, 3, 3, 5, 6, 11], "found": false, "foundPos": 0 },
  "params": [5],
  "contract_name": "BinarySearch",
  "fn": "search"
}`,
}, {
  name: 'City and Country',
  code: `contract City {
  id: string;
  name: string;
  country: Country; // reference to the Country contract

  constructor (id: string, name: string, country: Country) {
    this.id = id;
    this.name = name;
    this.country = country;
  }
}

contract Country {
  id: string;
  name: string;

  constructor (id: string, name: string) {
    this.id = id;
    this.name = name;
  }
}`,
  inputs: `{
  "init_params": { "id": "", "name": "", "country": { "id": "", "name": "" } },
  "params": ["boston", "BOSTON", { "id": "usa", "name": "USA"}],
  "contract_name": "City",
  "fn": "constructor"
}`,
}]