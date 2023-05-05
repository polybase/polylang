use std::{collections::HashMap, io::Read};

use miden_processor::math::Felt;
use polylang::compiler::abi::{self, Parser, TypeReader};

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
    this_values: HashMap<String, String>,
    abi: polylang::compiler::Abi,
}

impl Args {
    fn parse(args: std::env::Args) -> Result<Self, String> {
        let mut args = args.skip(1);
        let mut advice_tape = Vec::new();
        let mut abi = polylang::compiler::Abi::default();
        let mut this_values = HashMap::new();

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
                "--abi" => {
                    let abi_json = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    abi = serde_json::from_str::<polylang::compiler::Abi>(&abi_json)
                        .map_err(|e| format!("invalid value for argument {}: {}", arg, e))?;
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
            abi,
            this_values,
        })
    }

    fn this_value(&self) -> Result<abi::Value, Box<dyn std::error::Error>> {
        let this_type = self
            .abi
            .out_this_type
            .as_ref()
            .ok_or_else(|| "ABI does not specify a `this` type")?;
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

            let field_value = Parser::parse(field_type, value_str)?;

            struct_values.push((field_name.clone(), field_value));
        }

        Ok(abi::Value::StructValue(struct_values))
    }
}

fn hash(struct_type: polylang::compiler::Struct, value: &abi::Value) -> Vec<u64> {
    let hasher_program = polylang::compiler::compile_struct_hasher(struct_type);
    // println!("{}", hasher_program);
    // panic!();

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

    if let Some(this_type) = &args.abi.out_this_type {
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
            this_hash.iter().take(4).collect::<Vec<_>>()
        );

        stack.extend(this_hash.iter().take(4).rev());
    }
    advice_tape.extend(args.advice_tape);

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
        if str_ptr == Some(0) || str_ptr == None {
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

            if let Some(type_) = args.abi.out_this_type {
                let value = type_.read(&get_mem_values, args.abi.out_this_addr.unwrap() as _);
                println!("this: {:?}", value);
            }

            Ok(())
        }
        Some(miden::ExecutionError::FailedAssertion(_)) => {
            println!("Output: {:?}", stack);

            // read the error string out from the memory
            let str_len = get_mem_value(1).ok_or_else(|| "Got an error, but no error string")?;
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
