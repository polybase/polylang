use common::{compile_contract, run_contract, Args, Ctx};
use serde_json::json;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // specify your cpntract here
    let contract = r#"
    @public
    contract Fibonacci {
        fibVal: u32;

        function main(p: u32, a: u32, b: u32) {
            for (let i: u32 = 0; i < p; i++) {
                let c = a.wrappingAdd(b);
                a = b;
                b = c;
            }

            this.fibVal = a;
        }
    }
    "#;

    // pass the name of `contract` here
    let contract_name = Some("Fibonacci");
    // pass the name of the function to be executed here
    let function_name = "main".to_string();
    // pass the name of the proof file here
    let proof_file_name = "fibonacci.proof";

    let (miden_code, abi) = compile_contract(contract, contract_name, &function_name)?;

    let args = Args {
        advice_tape_json: Some("[8, 1, 1]".to_string()),
        this_values: HashMap::new(),
        this_json: Some(json!({"fibVal": 0})),
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
