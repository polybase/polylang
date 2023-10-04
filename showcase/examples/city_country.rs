use polylang_examples::{compile_contract, run_contract, Args, Ctx};
use serde_json::json;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // specify your cpntract here
    let contract = r#"
    contract City {
        id: string;
        name: string;
        country: Country;

        constructor(id: string, name: string, country: Country) {
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
    }
    "#;

    let contract_name = Some("City");
    let function_name = "constructor".to_string();
    let proof_file_name = "city_country.proof";

    let (miden_code, abi) = compile_contract(contract, contract_name, &function_name)?;

    let args = Args {
        advice_tape_json: Some(
            "[\"boston\", \"BOSTON\",  {\"id\": \"usa\", \"name\": \"USA\" }]".to_string(),
        ),
        this_values: HashMap::new(),
        this_json: Some(json!({"id": "", "name": "", "country": { "id": "", "name": "" }})),
        other_records: HashMap::new(),
        abi,
        ctx: Ctx::default(),
        proof_output: Some(proof_file_name.to_string()),
    };

    run_contract(miden_code, args)?;

    Ok(())
}
