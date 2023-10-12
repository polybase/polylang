use abi::Abi;
use error::prelude::*;
use std::{
    collections::HashMap,
    io::{Read, Write},
};

struct Args {
    advice_tape_json: Option<String>,
    this_values: HashMap<String, String>,
    this_json: Option<serde_json::Value>,
    /// Map of contract name to a list of records and field salts
    other_records: HashMap<String, Vec<(serde_json::Value, Vec<u32>)>>,
    abi: Abi,
    ctx: Ctx,
    proof_output: Option<String>,
}

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ctx {
    public_key: Option<abi::publickey::Key>,
}

impl Args {
    fn parse(args: std::env::Args, masm_code: &str) -> Result<Self, String> {
        let mut args = args.skip(1);
        let mut advice_tape_json = None;
        let mut abi = None;
        let mut this_values = HashMap::new();
        let mut this_json = None;
        let mut other_records = HashMap::new();
        let mut ctx = None;
        let mut proof_output = None;

        while let Some(arg) = args.next() {
            match arg.as_str() {
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

                    abi = Some(
                        serde_json::from_str::<Abi>(&abi_json)
                            .map_err(|e| format!("invalid value for argument {}: {}", arg, e))?,
                    );
                }
                "--this-json" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    let this_value = serde_json::from_str::<serde_json::Value>(&value)
                        .map_err(|e| format!("invalid value for argument {}: {}", value, e))?;

                    this_json = Some(this_value);
                }
                "--other-record" => {
                    let contract_name = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    let record_json_value = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    let record_json = serde_json::from_str::<serde_json::Value>(&record_json_value)
                        .map_err(|e| {
                            format!("invalid value for argument {}: {}", record_json_value, e)
                        })?;

                    other_records
                        .entry(contract_name)
                        .or_insert_with(Vec::new)
                        .push((record_json, vec![]));
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

        let abi = match abi {
            None => {
                let abi_comment = masm_code
                    .lines()
                    .find(|l| l.starts_with("# ABI: "))
                    .ok_or_else(|| {
                        "missing ABI. Please specify it with `--abi` or add a `# ABI: ...` comment".to_string()
                    })?;

                let abi_json = abi_comment["# ABI: ".len()..].trim();
                serde_json::from_str::<Abi>(abi_json).map_err(|e| format!("invalid ABI: {}", e))?
            }
            Some(abi) => abi,
        };

        for (contract, records) in other_records.iter_mut() {
            let col_struct = abi.other_contract_types.iter().find_map(|t| match t {
                abi::Type::Struct(s) if s.name == *contract => Some(s),
                _ => None,
            });

            for (_, salts) in records {
                *salts = vec![0; col_struct.map(|s| s.fields.len()).unwrap_or_default()];
            }
        }

        Ok(Self {
            advice_tape_json,
            abi,
            this_values,
            this_json,
            other_records,
            ctx: ctx.unwrap_or_default(),
            proof_output,
        })
    }

    fn this_value(&self) -> Result<abi::Value> {
        if self.this_json.is_some() {
            self.this_value_json()
        } else {
            self.this_value_str()
        }
    }

    fn this_value_str(&self) -> Result<abi::Value> {
        let this_type = self
            .abi
            .this_type
            .as_ref()
            .ok_or_else(|| Error::simple("ABI does not specify a `this` type"))?;
        let abi::Type::Struct(struct_) = this_type else {
            return Err(Error::simple("This type is not a struct"));
        };

        let mut struct_values = Vec::new();

        for (field_name, field_type) in &struct_.fields {
            let value_str = self.this_values.get(field_name).ok_or_else(|| {
                // FIXME: add a separate variant
                Error::simple(format!(
                    "missing value for field `{}` of type `{:?}`",
                    field_name, field_type
                ))
            })?;

            let field_value = abi::Parser::parse(field_type, value_str.as_str())?;

            struct_values.push((field_name.clone(), field_value));
        }

        Ok(abi::Value::StructValue(struct_values))
    }

    fn this_value_json(&self) -> Result<abi::Value> {
        let Some(this_json) = &self.this_json else {
            return Err(Error::simple("No JSON value for `this`"));
        };

        let this_type = self
            .abi
            .this_type
            .as_ref()
            .ok_or_else(|| Error::simple("ABI does not specify a `this` type"))?;
        let abi::Type::Struct(struct_) = this_type else {
            return Err(Error::simple("This type is not a struct"));
        };

        let use_defaults = this_json.as_object().map(|o| o.is_empty()).unwrap_or(false);

        let mut struct_values = Vec::new();
        for (field_name, field_type) in &struct_.fields {
            let field_value = match this_json.get(field_name) {
                Some(value) => abi::Parser::parse(field_type, value)?,
                None if use_defaults => field_type.default_value(),
                None if matches!(field_type, abi::Type::Nullable(_)) => field_type.default_value(),
                // FIXME: add a separate variant
                None => {
                    return Err(Error::simple(format!(
                        "missing value for field `{}`",
                        field_name
                    )))
                }
            };

            struct_values.push((field_name.clone(), field_value));
        }

        Ok(abi::Value::StructValue(struct_values))
    }

    fn inputs(
        &self,
        hasher: impl Fn(abi::Type, &abi::Value, Option<&[u32]>) -> Result<[u64; 4]>,
    ) -> Result<polylang_prover::Inputs> {
        let this = self.this_value()?;
        let abi::Value::StructValue(sv) = &this else {
            return Err(Error::simple("This value is not a struct"));
        };
        let this_fields = match self.abi.this_type.as_ref().unwrap() {
            abi::Type::Struct(s) => &s.fields,
            _ => unreachable!(),
        };
        let this_field_hashes = sv
            .iter()
            .enumerate()
            .map(|(i, (_, v))| hasher(this_fields[i].1.clone(), v, Some(&[0])))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(polylang_prover::Inputs {
            abi: self.abi.clone(),
            ctx_public_key: self.ctx.public_key.clone(),
            this_salts: sv.iter().map(|_| 0).collect(),
            this: this.try_into()?,
            this_field_hashes,
            args: serde_json::from_str(
                self
                    .advice_tape_json.as_deref()
                    .unwrap_or("[]"),
            )
            .wrap_err()?,
            other_records: self.other_records.clone(),
        })
    }
}

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let mut masm_code = String::new();
    std::io::stdin()
        .read_to_string(&mut masm_code)
        .context(IoSnafu)?;

    let mut args = Args::parse(std::env::args(), &masm_code).map_err(Error::simple)?;

    let has_this_type = if args.abi.this_type.is_none() {
        args.abi.this_type = Some(abi::Type::Struct(abi::Struct {
            name: "Empty".to_string(),
            fields: Vec::new(),
        }));

        false
    } else {
        true
    };

    let inputs = args.inputs(polylang_prover::hash_this)?;

    let program = polylang_prover::compile_program(&args.abi, &masm_code)
        .map_err(|e| e.add_source(masm_code))?;

    let (output, prove) = polylang_prover::run(&program, &inputs)?;

    dbg!(&output);
    dbg!(output.hashes());
    dbg!(output.logs());
    dbg!(output.cycle_count);

    if has_this_type {
        dbg!(output.self_destructed()?);
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
        let mut file = std::fs::File::create(&out).context(IoSnafu)?;
        file.write_all(&proof.0.to_bytes()).context(IoSnafu)?;

        println!("Proof saved to {out}");
    }

    Ok(())
}

fn main() {
    if let Err(e) = try_main() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
