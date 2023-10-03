use common::{compile_contract, run_contract, Args, Ctx};
use serde_json::json;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // specify your cpntract here
    let contract = r#"
      @public
      contract ReverseArray {
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
    "#;

    // pass the name of `contract` here
    let contract_name = Some("ReverseArray");
    // pass the name of the function to be executed here
    let function_name = "reverse".to_string();
    // pass the name of the proof file here
    let proof_file_name = "reverse.proof";

    let (miden_code, abi) = compile_contract(contract, contract_name, &function_name)?;

    let args = Args {
        advice_tape_json: None,
        this_values: HashMap::new(),
        this_json: Some(json!({"elements": [1, 3, 4, 5, 7, 6, 2, 3]})),
        other_records: HashMap::new(),
        abi,
        ctx: Ctx::default(),
        proof_output: Some(proof_file_name.to_string()),
    };

    // Run the contract. In addition to the output (if any), you should see the proof file
    // generated in the same directpry: `<proof_file_name>.proof`.
    run_contract(miden_code, args)?;

    Ok(())
}
