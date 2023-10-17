use base64::Engine;
use miden::{
    utils::Deserializable, verify as miden_verify, ProgramInfo, StackInputs, StackOutputs,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyRequest {
    proof: String,
    program_info: String,
    stack_inputs: Vec<String>,
    output_stack: Vec<String>,
    overflow_addrs: Vec<String>,
}

pub async fn verify(mut req: VerifyRequest) -> Result<bool, Box<dyn std::error::Error>> {
    let proof = base64::engine::general_purpose::STANDARD
        .decode(&req.proof)
        .unwrap();

    let program_info = ProgramInfo::read_from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(&req.program_info)
            .unwrap(),
    )
    .unwrap();

    let mut stack_inputs = req
        .stack_inputs
        .into_iter()
        .map(|s| s.parse::<u64>().unwrap())
        .collect::<Vec<_>>();

    stack_inputs.reverse();
    let mut stack_inputs = StackInputs::try_from_values(stack_inputs).unwrap();

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

    miden_verify(
        program_info,
        stack_inputs,
        output_stack,
        miden::ExecutionProof::from_bytes(&proof).unwrap(),
    )
    .unwrap();

    println!("Proof verified... no issues");

    Ok(true)
}
