use common::{compile_contract, run_contract, Args, Ctx};
use serde_json::json;
use std::collections::HashMap;

const CONTRACT: &str = r#"
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
    "#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let elems = vec![1, 2, 3, 3, 5, 6, 11];
    binary_search(&elems, 15)?;
    binary_search(&elems, 5)?;

    Ok(())
}

fn binary_search(arr: &Vec<i32>, elem: i32) -> Result<(), Box<dyn std::error::Error>> {
    let contract_name = Some("BinarySearch");
    let function_name = "search".to_string();

    let (miden_code, abi) = compile_contract(CONTRACT, contract_name, &function_name)?;

    let args = Args {
        advice_tape_json: Some(format!("[{elem}]")),
        this_values: HashMap::new(),
        this_json: Some(json!({"arr": arr, "found": false, "foundPos": 0 })),
        other_records: HashMap::new(),
        abi,
        ctx: Ctx::default(),
        proof_output: None,
    };

    run_contract(miden_code, args)?;

    Ok(())
}
