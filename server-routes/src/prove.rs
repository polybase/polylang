use std::collections::HashMap;

use base64::Engine;
use error::prelude::*;
use polylang_prover::{compile_program, Inputs, ProgramExt};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProveRequest {
    pub miden_code: String,
    pub abi: abi::Abi,
    pub ctx_public_key: Option<abi::publickey::Key>,
    pub this: Option<serde_json::Value>,
    pub this_salts: Option<Vec<u32>>,
    pub args: Vec<serde_json::Value>,
    pub other_records: Option<HashMap<String, Vec<(serde_json::Value, Vec<u32>)>>>,
}

pub async fn prove(mut req: ProveRequest) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let program = compile_program(&req.abi, &req.miden_code)?;

    let has_this = req.abi.this_type.is_some();
    let this = req.this.clone().unwrap_or(if has_this {
        req.abi.default_this_value()?.try_into()?
    } else {
        serde_json::Value::Null
    });

    if !has_this {
        req.abi.this_type = Some(abi::Type::Struct(abi::Struct {
            name: "Empty".to_string(),
            fields: Vec::new(),
        }));
        req.abi.this_addr = Some(0);
    }

    let inputs = Inputs::new(
        req.abi.clone(),
        req.ctx_public_key.clone(),
        req.this_salts.clone().unwrap_or_default(),
        this.clone(),
        req.args.clone(),
        req.other_records.clone().unwrap_or_default(),
    )?;

    let program_info = program.clone().to_program_info_bytes();
    let output = tokio::task::spawn_blocking({
        let inputs = inputs.clone();
        move || polylang_prover::prove(&program, &inputs).map_err(|e| e.to_string())
    })
    .await??;
    let new_this = TryInto::<serde_json::Value>::try_into(output.new_this)?;

    Ok(serde_json::json!({
        "old": {
            "this": this,
            "hashes": inputs.this_field_hashes,
        },
        "new": {
            "selfDestructed": output.run_output.self_destructed()?,
            "this": new_this,
            "hashes": output.new_hashes,
        },
        "stack": {
            "input": output.input_stack,
            "output": output.stack,
        },
        "result": if req.abi.result_type.is_some() {
            serde_json::json!({
                "value": output.run_output.result(&req.abi).map(TryInto::<serde_json::Value>::try_into)??,
                "hash": output.run_output.result_hash(&req.abi),
            })
        } else { serde_json::Value::Null },
        "programInfo": base64::engine::general_purpose::STANDARD.encode(program_info),
        "proof": base64::engine::general_purpose::STANDARD.encode(output.proof),
        "debug": {
            "logs": output.run_output.logs(),
        }
    }))
}
