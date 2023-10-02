use common::{compile_contract, run_contract, Args, Ctx};
use serde_json::json;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // specify your cpntract here
    let contract = r#"
    @public
    contract HelloWorld {
        sum: i32;

        function add(a: i32, b: i32): i32 {
           this.sum = a + b;
        }
    }
    "#;

    // pass the name of `contract` here
    let contract_name = Some("HelloWorld");
    // pass the name of the function to be executed here
    let function_name = "add".to_string();
    // pass the name of the proof file here
    let proof_file_name = "add.proof";

    let (miden_code, abi) = compile_contract(contract, contract_name, &function_name)?;

    let args = Args {
        advice_tape_json: Some("[1, 2]".into()),
        this_values: HashMap::new(),
        this_json: Some(json!({"sum": 0})),
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
