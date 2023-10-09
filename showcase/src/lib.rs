use abi::Abi;
use std::{collections::HashMap, io::Write};

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ctx {
    pub public_key: Option<abi::publickey::Key>,
}

pub struct Args {
    pub advice_tape_json: Option<String>,
    pub this_values: HashMap<String, String>,
    pub this_json: Option<serde_json::Value>,
    pub other_records: HashMap<String, Vec<(serde_json::Value, Vec<u32>)>>,
    pub abi: Abi,
    pub ctx: Ctx,
    pub proof_output: Option<String>,
}

impl Args {
    pub fn inputs(
        &self,
        hasher: impl Fn(
            abi::Type,
            &abi::Value,
            Option<&[u32]>,
        ) -> Result<[u64; 4], Box<dyn std::error::Error>>,
    ) -> Result<polylang_prover::Inputs, Box<dyn std::error::Error>> {
        let this = self.this_value()?;
        let abi::Value::StructValue(sv) = &this else {
            return Err("This value is not a struct".into());
        };
        let this_fields = match self.abi.this_type.as_ref().unwrap() {
            abi::Type::Struct(s) => &s.fields,
            _ => unreachable!(),
        };
        let this_field_hashes = sv
            .iter()
            .enumerate()
            .map(|(i, (_, v))| hasher(this_fields[i].1.clone(), &v, Some(&[0])))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(polylang_prover::Inputs {
            abi: self.abi.clone(),
            ctx_public_key: self.ctx.public_key.clone(),
            this_salts: sv.iter().map(|_| 0).collect(),
            this: this.try_into()?,
            this_field_hashes,
            args: serde_json::from_str(
                &self
                    .advice_tape_json
                    .as_ref()
                    .map(|x| x.as_str())
                    .unwrap_or("[]"),
            )?,
            other_records: self.other_records.clone(),
        })
    }

    fn this_value(&self) -> Result<abi::Value, Box<dyn std::error::Error>> {
        self.this_value_json()
    }

    fn this_value_json(&self) -> Result<abi::Value, Box<dyn std::error::Error>> {
        let Some(this_json) = &self.this_json else {
            return Err("No JSON value for `this`".into());
        };

        let this_type = self
            .abi
            .this_type
            .as_ref()
            .ok_or_else(|| "ABI does not specify a `this` type")?;

        let abi::Type::Struct(struct_) = this_type else {
            return Err("This type is not a struct".into());
        };

        let use_defaults = this_json.as_object().map(|o| o.is_empty()).unwrap_or(false);

        let mut struct_values = Vec::new();
        for (field_name, field_type) in &struct_.fields {
            let field_value = match this_json.get(field_name) {
                Some(value) => abi::Parser::parse(field_type, value)?,
                None if use_defaults => field_type.default_value(),
                None if matches!(field_type, abi::Type::Nullable(_)) => field_type.default_value(),
                None => return Err(format!("missing value for field `{}`", field_name).into()),
            };

            struct_values.push((field_name.clone(), field_value));
        }

        Ok(abi::Value::StructValue(struct_values))
    }
}

pub fn compile_contract(
    contract: &'static str,
    contract_name: Option<&str>,
    function_name: &str,
) -> Result<(String, abi::Abi), Box<dyn std::error::Error>> {
    let program = polylang_parser::parse(&contract)?;

    Ok(
        polylang::compiler::compile(program, contract_name, &function_name)
            .map_err(|e| e.add_source(contract))
            .unwrap_or_else(|e| panic!("{e}")),
    )
}

pub fn run_contract(miden_code: String, mut args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let has_this_type = if args.abi.this_type.is_none() {
        args.abi.this_type = Some(abi::Type::Struct(abi::Struct {
            name: "Empty".to_string(),
            fields: Vec::new(),
        }));

        false
    } else {
        true
    };

    let inputs = args.inputs(|t, v, s| Ok(polylang_prover::hash_this(t, v, s)?))?;

    let program = polylang_prover::compile_program(&args.abi, &miden_code)
        .map_err(|e| e.add_source(miden_code))?;

    let (output, prove) = polylang_prover::run(&program, &inputs)?;

    if has_this_type {
        println!(
            "this_json: {}",
            TryInto::<serde_json::Value>::try_into(output.this(&args.abi)?)?
        );
    }

    if args.abi.result_type.is_some() {
        println!(
            "result_json: {}",
            TryInto::<serde_json::Value>::try_into(output.result(&args.abi)?)?
        );
    }

    if let Some(out) = args.proof_output {
        let proof = prove()?;
        let mut file = std::fs::File::create(&out)?;
        file.write_all(&proof.to_bytes())?;

        println!("Proof saved to {out}");
    }

    Ok(())
}
