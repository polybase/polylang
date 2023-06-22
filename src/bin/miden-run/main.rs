use std::{
    collections::HashMap,
    io::{Read, Write},
};

use polylang::{
    compiler::{
        self,
        abi::{self, Parser, TypeReader, Value},
    },
    prover::{self, Inputs},
};
struct Args {
    advice_tape: Vec<u64>,
    advice_tape_json: Option<String>,
    this_values: HashMap<String, String>,
    this_json: Option<serde_json::Value>,
    abi: polylang::compiler::Abi,
    ctx: Ctx,
    proof_output: Option<String>,
}

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ctx {
    public_key: Option<compiler::Key>,
}

impl Args {
    fn parse(args: std::env::Args) -> Result<Self, String> {
        let mut args = args.skip(1);
        let mut advice_tape = Vec::new();
        let mut advice_tape_json = None;
        let mut abi = polylang::compiler::Abi::default();
        let mut this_values = HashMap::new();
        let mut this_json = None;
        let mut ctx = None;
        let mut proof_output = None;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--advice-tape" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    let values = value
                        .split(',')
                        .map(|s| s.parse::<u64>())
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| format!("invalid value for argument {}: {}", arg, e))?;

                    advice_tape.extend(values);
                }
                "--advice-tape-json" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    advice_tape_json = Some(value);
                }
                "--abi" => {
                    let abi_json = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    abi = serde_json::from_str::<polylang::compiler::Abi>(&abi_json)
                        .map_err(|e| format!("invalid value for argument {}: {}", arg, e))?;
                }
                "--this-json" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    let this_value = serde_json::from_str::<serde_json::Value>(&value)
                        .map_err(|e| format!("invalid value for argument {}: {}", value, e))?;

                    this_json = Some(this_value);
                }
                "--ctx" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    let c = serde_json::from_str::<Ctx>(&value)
                        .map_err(|e| format!("invalid value for argument {}: {}", value, e))?;

                    ctx = Some(c);
                }
                "--proof-output" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    proof_output = Some(value);
                }
                this_param if this_param.starts_with("--this.") => {
                    let field = this_param
                        .strip_prefix("--this.")
                        .ok_or_else(|| format!("invalid argument: {}", arg))?;

                    // TODO: store these values in something, hash them (and pass the hash), serialize and send them over the advice tape
                    let value = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    this_values.insert(field.to_string(), value);
                }
                _ => return Err(format!("unknown argument: {}", arg)),
            }
        }

        Ok(Self {
            advice_tape,
            advice_tape_json,
            abi,
            this_values,
            this_json,
            ctx: ctx.unwrap_or_default(),
            proof_output,
        })
    }

    fn this_value(&self) -> Result<abi::Value, Box<dyn std::error::Error>> {
        if self.this_json.is_some() {
            self.this_value_json()
        } else {
            self.this_value_str()
        }
    }

    fn this_value_str(&self) -> Result<abi::Value, Box<dyn std::error::Error>> {
        let this_type = self
            .abi
            .this_type
            .as_ref()
            .ok_or("ABI does not specify a `this` type")?;
        let polylang::compiler::Type::Struct(struct_) = this_type else {
            return Err("This type is not a struct".into());
        };

        let mut struct_values = Vec::new();

        for (field_name, field_type) in &struct_.fields {
            let value_str = self.this_values.get(field_name).ok_or_else(|| {
                format!(
                    "missing value for field `{}` of type `{:?}`",
                    field_name, field_type
                )
            })?;

            let field_value = Parser::parse(field_type, value_str.as_str())?;

            struct_values.push((field_name.clone(), field_value));
        }

        Ok(abi::Value::StructValue(struct_values))
    }

    fn this_value_json(&self) -> Result<abi::Value, Box<dyn std::error::Error>> {
        let Some(this_json) = &self.this_json else {
            return Err("No JSON value for `this`".into());
        };

        let this_type = self
            .abi
            .this_type
            .as_ref()
            .ok_or("ABI does not specify a `this` type")?;
        let polylang::compiler::Type::Struct(struct_) = this_type else {
            return Err("This type is not a struct".into());
        };

        let use_defaults = this_json.as_object().map(|o| o.is_empty()).unwrap_or(false);

        let mut struct_values = Vec::new();
        for (field_name, field_type) in &struct_.fields {
            let field_value = match this_json.get(field_name) {
                Some(value) => Parser::parse(field_type, value)?,
                None if use_defaults => field_type.default_value(),
                None if matches!(field_type, polylang::compiler::Type::Nullable(_)) => {
                    field_type.default_value()
                }
                None => return Err(format!("missing value for field `{}`", field_name).into()),
            };

            struct_values.push((field_name.clone(), field_value));
        }

        Ok(abi::Value::StructValue(struct_values))
    }

    fn inputs(
        &self,
        hasher: impl Fn(&abi::Value) -> Result<[u64; 4], Box<dyn std::error::Error>>,
    ) -> Result<prover::Inputs, Box<dyn std::error::Error>> {
        let this = self.this_value()?;
        let this_hash = hasher(&this)?;

        Ok(prover::Inputs {
            abi: self.abi.clone(),
            ctx_public_key: self.ctx.public_key.clone(),
            this: this.into(),
            this_hash,
            args: serde_json::from_str(
                &self
                    .advice_tape_json
                    .as_ref()
                    .map(|x| x.as_str())
                    .unwrap_or("[]"),
            )?,
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse(std::env::args())?;

    let inputs = args.inputs(|v| prover::hash_this(args.abi.this_type.clone().unwrap(), v))?;

    let mut masm_code = String::new();
    std::io::stdin().read_to_string(&mut masm_code)?;

    let assembler = miden::Assembler::default()
        .with_library(&miden_stdlib::StdLibrary::default())
        .expect("Failed to load stdlib");
    let program = assembler
        .compile(&masm_code)
        .expect("Failed to compile miden assembly");

    let (output, prove) = prover::run(&program, &inputs)?;

    dbg!(output.hash());
    dbg!(output.logs());
    dbg!(output.self_destructed());

    println!(
        "this_json: {}",
        Into::<serde_json::Value>::into(output.this(&args.abi)?)
    );

    if let Some(out) = args.proof_output {
        let proof = prove()?;
        let mut file = std::fs::File::create(&out)?;
        file.write_all(&proof.to_bytes())?;

        println!("Proof saved to {out}");
    }

    Ok(())
}
