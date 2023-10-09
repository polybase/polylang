use polylang_examples::{compile_contract, run_contract, Args, Ctx};
use serde_json::json;
use std::collections::HashMap;

const CONTRACT: &str = r#"
contract Account {
    id: string;
    balance: number;

    constructor(id: string, balance: number) {
        this.id = id;
        this.balance = balance;
    }

    deposit(amt: number) {
        this.balance = this.balance + amt;
    }

    withdraw(amt: number) {
        if (this.balance < 0) {
            error("Insufficient balance");
        }
        this.balance = this.balance - amt;
    }

    getBalance(): number {
        return this.balance;
    }
}
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // initialise
    run(
        json!({"id": "", "balance": 0}),
        "constructor",
        Some("[\"id1\", 100]".to_string()),
    )?;

    deposit(json!({"id": "id1", "balance": 100}), 50)?;
    withdraw(json!( {"id": "id1", "balance": 150}), 25)?;
    get_balance(json!( {"id": "id1", "balance": 125}))?;

    Ok(())
}

fn deposit(this_json: serde_json::Value, amt: u32) -> Result<(), Box<dyn std::error::Error>> {
    run(this_json, "deposit", Some(format!("[{amt}]")))?;

    Ok(())
}

fn withdraw(this_json: serde_json::Value, amt: u32) -> Result<(), Box<dyn std::error::Error>> {
    run(this_json, "withdraw", Some(format!("[{amt}]")))?;

    Ok(())
}

fn get_balance(this_json: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    run(this_json, "getBalance", None)?;

    Ok(())
}

fn run(
    this_json: serde_json::Value,
    function_name: &str,
    advice: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let contract_name = Some("Account");
    let function_name = function_name.to_string();

    let (miden_code, abi) = compile_contract(CONTRACT, contract_name, &function_name)?;

    let args = Args {
        advice_tape_json: advice,
        this_values: HashMap::new(),
        this_json: Some(this_json),
        other_records: HashMap::new(),
        abi,
        ctx: Ctx::default(),
        proof_output: None, // don't generate a proof
    };

    run_contract(miden_code, args)?;

    Ok(())
}
