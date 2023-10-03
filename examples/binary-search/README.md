## Binary Search

The contract for this example is:

```typescript
 contract BinarySearch {
     arr: i32[];
     found: boolean;
     foundPos: u32;

     constructor (arr: i32[]) {
         this.arr = arr;
     }

     function search(elem: i32) {
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
                     this.found = true;
                     this.foundPos = mid;
                     break;
                 }
             }
         }

         if (low > high) {
             this.found = false;
         }
     }
 }

```

The example showcases iterative Binary Search.

## Build and Run

```bash
$ cargo run --release
```

## Demo

```bash
$ cargo run --release
```

Output:

```bash
this_json: {"arr":[1,2,3,3,5,6,11],"found":false,"foundPos":0}
this_json: {"arr":[1,2,3,3,5,6,11],"found":true,"foundPos":4}
```

## LICENSE

This template is licensed under the [MIT License](../LICENSE.md).
