use std::{collections::HashMap, io::Read};

use polylang::compiler::{
    self,
    abi::{self, Parser, TypeReader, Value},
};

// Copied from https://github.com/novifinancial/winterfell/blob/1a1815adb51757e57f8f3844c51ff538e6c17a32/math/src/field/f64/mod.rs#L572
const fn mont_red_cst(x: u128) -> u64 {
    // See reference above for a description of the following implementation.
    let xl = x as u64;
    let xh = (x >> 64) as u64;
    let (a, e) = xl.overflowing_add(xl << 32);

    let b = a.wrapping_sub(a >> 32).wrapping_sub(e as u64);

    let (r, c) = xh.overflowing_sub(b);
    r.wrapping_sub(0u32.wrapping_sub(c as u32) as u64)
}

struct Args {
    advice_tape: Vec<u64>,
    advice_tape_json: Option<String>,
    this_values: HashMap<String, String>,
    this_json: Option<serde_json::Value>,
    abi: polylang::compiler::Abi,
    ctx: Ctx,
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

    fn args_advice_tape(&self) -> Result<Vec<u64>, Box<dyn std::error::Error>> {
        if let Some(advice_tape_json) = &self.advice_tape_json {
            let mut tape = Vec::new();
            let advice_tape_json = serde_json::from_str::<Vec<serde_json::Value>>(advice_tape_json)
                .map_err(|e| format!("invalid value for argument: {}", e))?;

            for (i, t) in self.abi.param_types.iter().enumerate() {
                tape.extend_from_slice(&t.parse(&advice_tape_json[i])?.serialize());
            }

            Ok(tape)
        } else {
            Ok(self.advice_tape.clone())
        }
    }

    fn ctx_advice_tape(&self) -> Result<Vec<u64>, Box<dyn std::error::Error>> {
        let ctx = Value::StructValue(vec![(
            "publicKey".to_owned(),
            Value::Nullable(
                self.ctx
                    .public_key
                    .clone()
                    .map(|pk| Box::new(Value::PublicKey(pk))),
            ),
        )]);

        Ok(ctx.serialize())
    }
}

fn hash(struct_type: polylang::compiler::Struct, value: &abi::Value) -> Vec<u64> {
    let hasher_program = polylang::compiler::compile_struct_hasher(struct_type);

    let assembler = miden::Assembler::default()
        .with_library(&miden_stdlib::StdLibrary::default())
        .expect("Failed to load stdlib");

    let program = assembler
        .compile(hasher_program)
        .expect("Failed to compile miden assembly");

    let execution_result = miden::execute(
        &program,
        miden::StackInputs::default(),
        miden::MemAdviceProvider::from(
            miden::AdviceInputs::default()
                .with_tape_values(value.serialize().into_iter())
                .unwrap(),
        ),
    )
    .unwrap();

    execution_result.stack_outputs().stack().to_vec()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse(std::env::args())?;
    let mut advice_tape = vec![];
    let mut stack = Vec::<u64>::new();

    advice_tape.extend(&args.ctx_advice_tape()?);

    if let Some(this_type) = &args.abi.this_type {
        let this_value = args.this_value()?;
        advice_tape.extend(this_value.serialize());
        let this_hash = hash(
            match this_type {
                polylang::compiler::Type::Struct(s) => s.clone(),
                _ => Err("This type is not a struct")?,
            },
            &this_value,
        );
        eprintln!(
            "Hash of input this: {:?}",
            this_hash.iter().take(4).rev().collect::<Vec<_>>()
        );

        stack.extend(this_hash.iter().take(4).rev());
    }
    advice_tape.extend(&args.args_advice_tape()?);

    let mut masm_code = String::new();
    std::io::stdin().read_to_string(&mut masm_code)?;

    let assembler = miden::Assembler::default()
        .with_library(&miden_stdlib::StdLibrary::default())
        .expect("Failed to load stdlib");
    let program = assembler
        .compile(&masm_code)
        .expect("Failed to compile miden assembly");

    let advice_provider = miden::MemAdviceProvider::from(
        miden::AdviceInputs::default()
            .with_tape_values(advice_tape)
            .expect("Invalid advice tape"),
    );
    let stack_inputs = miden::StackInputs::try_from_values(stack).unwrap();
    let mut last_ok_state = None;
    let mut err = None;

    for state in miden_processor::execute_iter(&program, stack_inputs, advice_provider) {
        match state {
            Ok(state) => {
                last_ok_state = Some(state);
            }
            Err(e) => {
                err = Some(e);
            }
        }
    }

    let stack = last_ok_state
        .as_ref()
        .unwrap()
        .stack
        .iter()
        .map(|x| mont_red_cst(x.inner() as _))
        .collect::<Vec<_>>();

    let get_mem_values = |addr| {
        last_ok_state
            .as_ref()
            .unwrap()
            .memory
            .iter()
            .find(|(a, _)| *a == addr)
            .map(|(_, word)| {
                [
                    mont_red_cst(word[0].inner() as _),
                    mont_red_cst(word[1].inner() as _),
                    mont_red_cst(word[2].inner() as _),
                    mont_red_cst(word[3].inner() as _),
                ]
            })
    };
    let get_mem_value = |addr| get_mem_values(addr).map(|word| word[0]);
    let read_string = |len: u64, data_ptr: u64| {
        let mut str_bytes = Vec::new();
        for i in 0..len {
            let c = get_mem_value(data_ptr + i).unwrap() as u8;
            str_bytes.push(c);
        }

        String::from_utf8(str_bytes).unwrap()
    };

    let mut log_messages = Vec::new();
    let (mut prev, mut str_ptr) = (get_mem_value(4), get_mem_value(5));
    loop {
        if str_ptr == Some(0) || str_ptr.is_none() {
            break;
        }

        let len = get_mem_value(str_ptr.unwrap()).unwrap();
        let data_ptr = get_mem_value(str_ptr.unwrap() + 1).unwrap();
        let str = read_string(len, data_ptr);
        log_messages.push(str);

        str_ptr = get_mem_value(prev.unwrap() + 1);
        prev = get_mem_value(prev.unwrap());
    }
    log_messages.reverse();

    for msg in log_messages {
        println!("Log: {}", msg);
    }

    match err {
        None => {
            println!("Output: {:?}", stack);

            if let Some(type_) = args.abi.this_type {
                let value = type_.read(&get_mem_values, args.abi.this_addr.unwrap() as _);
                println!("this: {:?}", value);
                println!(
                    "this_json: {}",
                    value
                        .map(|v| Into::<serde_json::Value>::into(v))
                        .map(|v| serde_json::to_string(&v).unwrap())
                        .unwrap_or_else(|_| "null".to_string())
                );

                let self_destruct_flag = stack[4];
                println!("Self-destructed: {}", self_destruct_flag != 0);
            }

            Ok(())
        }
        Some(miden::ExecutionError::FailedAssertion(_)) => {
            println!("Output: {:?}", stack);

            // read the error string out from the memory
            let str_len = get_mem_value(1).ok_or("Got an error, but no error string")?;
            let str_data_ptr = get_mem_value(2).unwrap();

            if str_data_ptr == 0 {
                Err("Foreign (not from our language) assertion failed".into())
            } else {
                let mut error_str_bytes = Vec::new();
                for i in 0..str_len {
                    let c = get_mem_value(str_data_ptr + i).unwrap() as u8;
                    error_str_bytes.push(c);
                }

                Err(format!("Assertion failed: {}", read_string(str_len, str_data_ptr)).into())
            }
        }
        Some(e) => Err(format!("Execution error: {:?}", e).into()),
    }
}
