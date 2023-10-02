use std::collections::HashMap;

use abi::Abi;
use error::prelude::*;
use miden::{verify, ProgramInfo, StackInputs, StackOutputs};
use polylang_prover::{Inputs, RunOutput};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Program {
    miden_code: String,
    abi: Abi,
}

#[wasm_bindgen]
pub fn compile(
    code: String,
    contract_name: Option<String>,
    fn_name: &str,
) -> Result<Program, JsError> {
    let program = polylang::parse_program(&code)?;
    let (miden_code, mut abi) =
        polylang::compiler::compile(program, contract_name.as_deref(), fn_name)?;

    if contract_name.is_none() {
        abi.this_type = Some(abi::Type::Struct(abi::Struct {
            name: "Empty".to_string(),
            fields: Vec::new(),
        }));
    }

    Ok(Program { miden_code, abi })
}

#[wasm_bindgen]
impl Program {
    pub fn run(
        &self,
        this_json: String,
        args_json: String,
        generate_proof: bool,
    ) -> Result<Output, JsError> {
        let this = match serde_json::from_str(&this_json)? {
            serde_json::Value::Null => serde_json::Value::Object(serde_json::Map::new()),
            other => other,
        };
        let args = serde_json::from_str(&args_json)?;

        let program = polylang_prover::compile_program(&self.abi, &self.miden_code)?;
        let inputs = Inputs::new(
            self.abi.clone(),
            None,
            self.abi
                .this_type
                .as_ref()
                .map(|ty| match ty {
                    abi::Type::Struct(st) => Ok(st.fields.iter().map(|_| 0).collect()),
                    _ => Err(Error::simple("this type must be a struct")),
                })
                .transpose()?
                .unwrap_or(vec![]),
            this,
            args,
            HashMap::new(),
        )?;

        let (output, prove) = polylang_prover::run(&program, &inputs)?;

        let hash = program.hash();
        let kernal = program.kernel();
        let info = ProgramInfo::new(hash, kernal.clone());

        let maybe_proof = if generate_proof { Some(prove()?) } else { None };
        let proof = maybe_proof.as_ref().map(|(p, _)| p.to_bytes());
        let output_stack = maybe_proof.map(|(_, os)| os);

        // let _ = verify(
        //     info.clone(),
        //     inputs.stack(&HashMap::new()).unwrap(),
        //     output_stack.clone().unwrap(),
        //     miden::ExecutionProof::from_bytes(&proof.clone().unwrap().to_vec())
        //         .map_err(|err| JsError::new(&format!("failed to parse proof: {}", err)))?,
        // );

        Ok(Output {
            info,
            abi: self.abi.clone(),
            // inputs,
            output,
            proof,
            output_stack,
        })
    }
}

#[wasm_bindgen]
pub struct Output {
    info: ProgramInfo,
    abi: Abi,
    // inputs: StackInputs,
    output: RunOutput,
    proof: Option<Vec<u8>>,
    output_stack: Option<StackOutputs>,
}

#[wasm_bindgen]
impl Output {
    pub fn cycle_count(&self) -> u32 {
        self.output.cycle_count
    }

    pub fn proof(&self) -> Option<Vec<u8>> {
        self.proof.clone()
    }

    pub fn verify(&self) -> Result<bool, JsError> {
        verify(
            self.info.clone(),
            StackInputs::try_from_values(self.output.input_stack.clone().into_iter())
                .map_err(|e| JsError::new(&e.to_string()))?,
            self.output_stack.clone().unwrap(),
            miden::ExecutionProof::from_bytes(&self.proof.clone().unwrap().to_vec())
                .map_err(|err| JsError::new(&format!("failed to parse proof: {}", err)))?,
        )
        .map_err(|e| JsError::new(&e.to_string()))
        .map(|_| true)
    }

    pub fn this(&self) -> Result<JsValue, JsError> {
        let json_value: serde_json::Value = self.output.this(&self.abi)?.try_into()?;
        Ok(serde_wasm_bindgen::to_value(&json_value)?)
    }

    pub fn hashes(&self) -> Result<JsValue, JsError> {
        let hashes = self
            .output
            .hashes()
            .into_iter()
            .map(|h| {
                [
                    // the full-range of u64 doesn't fit in JavaScript's Number,
                    // so we convert it to string
                    h[0].to_string(),
                    h[1].to_string(),
                    h[2].to_string(),
                    h[3].to_string(),
                ]
            })
            .collect::<Vec<_>>();
        Ok(serde_wasm_bindgen::to_value(&hashes)?)
    }

    pub fn logs(&self) -> Result<JsValue, JsError> {
        let logs = self.output.logs();
        Ok(serde_wasm_bindgen::to_value(&logs)?)
    }

    pub fn self_destructed(&self) -> Result<bool, JsError> {
        Ok(self.output.self_destructed()?)
    }

    pub fn read_auth(&self) -> bool {
        self.output.read_auth()
    }
}
