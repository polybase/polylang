use base64::Engine;
use miden::{
    utils::Deserializable, verify as miden_verify, ProgramInfo, StackInputs, StackOutputs,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyRequest {
    proof: String,
    program_info: String,
    stack_inputs: Vec<String>,
    output_stack: Vec<String>,
    overflow_addrs: Vec<String>,
}

pub async fn verify(req: VerifyRequest) -> Result<bool, Box<dyn std::error::Error>> {
    let proof = base64::engine::general_purpose::STANDARD.decode(&req.proof)?;

    let program_info = ProgramInfo::read_from_bytes(
        &base64::engine::general_purpose::STANDARD.decode(&req.program_info)?,
    )
    .map_err(|e| e.to_string())?;

    let mut stack_inputs = req
        .stack_inputs
        .into_iter()
        .map(|s| s.parse::<u64>().unwrap())
        .collect::<Vec<_>>();

    stack_inputs.reverse();
    let stack_inputs = StackInputs::try_from_values(stack_inputs).map_err(|e| e.to_string())?;

    let overflow_addrs = req
        .overflow_addrs
        .into_iter()
        .map(|s| s.parse::<u64>().unwrap())
        .collect::<Vec<_>>();

    let output_stack = req
        .output_stack
        .into_iter()
        .map(|s| s.parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    let output_stack = StackOutputs::new(output_stack, overflow_addrs);

    Ok(miden_verify(
        program_info,
        stack_inputs,
        output_stack,
        miden::ExecutionProof::from_bytes(&proof).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
    .map(|_| {
        println!("Proof verified");
        true
    })?)
}
