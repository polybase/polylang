use std::collections::HashMap;

use base64::Engine;
use error::prelude::*;
use polylang_prover::{compile_program, Inputs, ProgramExt};
use serde::Deserialize;

type OtherRecordsType = HashMap<String, Vec<(serde_json::Value, Vec<u32>)>>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProveRequest {
    pub miden_code: String,
    pub abi: abi::Abi,
    pub ctx_public_key: Option<abi::publickey::Key>,
    pub this: Option<serde_json::Value>, // this_json
    pub this_salts: Option<Vec<u32>>,
<<<<<<< HEAD
    pub args: Vec<serde_json::Value>,
    pub other_records: Option<OtherRecordsType>,
=======
    pub args: Vec<serde_json::Value>, // args_json
    pub other_records: Option<HashMap<String, Vec<(serde_json::Value, Vec<u32>)>>>,
>>>>>>> 1f3e90b (Changes:)
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

    let this_salts = req
        .abi
        .this_type
        .as_ref()
        .map(|ty| match ty {
            abi::Type::Struct(st) => Ok(st.fields.iter().map(|_| 0).collect()),
            _ => Err(Error::simple("this type must be a struct")),
        })
        .transpose()?
        .unwrap_or(vec![]);

    let inputs = Inputs::new(
        req.abi.clone(),
        req.ctx_public_key.clone(),
        this_salts,
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
    let proof_len = output.proof.len();
    let result_hash = output
        .run_output
        .result_hash(&req.abi)
        .map(|h| h.into_iter().map(|x| x.to_string()).collect::<Vec<_>>());

    Ok(serde_json::json!({
        "old": {
            "this": this,
            "hashes": inputs.this_field_hashes,
        },
        "new": {
            "selfDestructed": output.run_output.self_destructed()?,
            "this": new_this,
            "hashes": output.new_hashes.into_iter().map(|h| {
                [
                    h[0].to_string(),
                    h[1].to_string(),
                    h[2].to_string(),
                    h[3].to_string(),
                ]
            }).collect::<Vec<_>>()
        },
        "stack": {
            "input": output.input_stack.into_iter().map(|h| h.to_string()).collect::<Vec<_>>(),
            "output": output.stack.into_iter().map(|h| h.to_string()).collect::<Vec<_>>(),
            "overflowAddrs": output.overflow_addrs.into_iter().map(|h| h.to_string()).collect::<Vec<_>>(),
        },
        "result": if req.abi.result_type.is_some() {
            serde_json::json!({
                "value": output.run_output.result(&req.abi).map(TryInto::<serde_json::Value>::try_into)??,
                "hash": result_hash,
            })
        } else { serde_json::Value::Null },
        "programInfo": base64::engine::general_purpose::STANDARD.encode(program_info),
        "proof": base64::engine::general_purpose::STANDARD.encode(output.proof),
        "debug": {
            "logs": output.run_output.logs(),
        },
        "cycleCount": output.run_output.cycle_count,
        "proofLength": proof_len, // raw unencoded length
        "logs": output.run_output.logs(),
        "readAuth": output.run_output.read_auth(),
    }))
}
