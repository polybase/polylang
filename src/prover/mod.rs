use std::collections::HashMap;

use crate::compiler::{
    self,
    abi::{Parser, TypeReader, Value},
    Abi, Type,
};
use miden::{ExecutionProof, ProofOptions};
use miden_processor::{AdviceProvider, Program, StackInputs};

const fn mont_red_cst(x: u128) -> u64 {
    // See reference above for a description of the following implementation.
    let xl = x as u64;
    let xh = (x >> 64) as u64;
    let (a, e) = xl.overflowing_add(xl << 32);

    let b = a.wrapping_sub(a >> 32).wrapping_sub(e as u64);

    let (r, c) = xh.overflowing_sub(b);
    r.wrapping_sub(0u32.wrapping_sub(c as u32) as u64)
}

pub struct Output {
    pub run_output: RunOutput,
    pub stack: Vec<u64>,
    pub self_destructed: bool,
    pub new_this: Value,
    pub new_hash: [u64; 4],
    pub proof: Vec<u8>,
}

fn json_to_this_value(
    this_json: &serde_json::Value,
    this_type: &Type,
) -> Result<Value, Box<dyn std::error::Error>> {
    let Type::Struct(struct_) = this_type else {
            return Err("This type is not a struct".into());
        };

    let use_defaults = this_json.as_object().map(|o| o.is_empty()).unwrap_or(false);

    let mut struct_values = Vec::new();
    for (field_name, field_type) in &struct_.fields {
        let field_value = match this_json.get(field_name) {
            Some(value) => Parser::parse(field_type, value)?,
            None if use_defaults => field_type.default_value(),
            None if matches!(field_type, Type::Nullable(_)) => field_type.default_value(),
            None => return Err(format!("missing value for field `{}`", field_name).into()),
        };

        struct_values.push((field_name.clone(), field_value));
    }

    Ok(Value::StructValue(struct_values))
}

pub fn hash_this(struct_type: Type, this: &Value) -> Result<[u64; 4], Box<dyn std::error::Error>> {
    let Type::Struct(struct_type) = struct_type else {
        return Err("This type is not a struct".into());
    };

    let hasher_program = compiler::compile_struct_hasher(struct_type.clone());

    let assembler =
        miden::Assembler::default().with_library(&miden_stdlib::StdLibrary::default())?;

    let program = assembler.compile(hasher_program)?;

    let execution_result = miden::execute(
        &program,
        miden::StackInputs::default(),
        miden::MemAdviceProvider::from(
            miden::AdviceInputs::default().with_stack_values(this.serialize().into_iter())?,
        ),
    )?;

    Ok(execution_result.stack_outputs().stack()[0..4].try_into()?)
}

pub struct Inputs {
    pub abi: Abi,
    pub ctx_public_key: Option<compiler::Key>,
    pub this: serde_json::Value,
    pub this_hash: [u64; 4],
    pub args: Vec<serde_json::Value>,
}

impl Inputs {
    pub fn stack_values(&self) -> Vec<u64> {
        self.this_hash.iter().cloned().rev().collect::<Vec<_>>()
    }

    fn stack(&self) -> Result<StackInputs, Box<dyn std::error::Error>> {
        Ok(StackInputs::try_from_values(self.stack_values())?)
    }

    fn this_value(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let Some(this_type) = &self.abi.this_type else {
            return Err("Missing this type".into());
        };

        json_to_this_value(&self.this, this_type)
    }

    fn advice_tape(&self) -> Result<impl AdviceProvider + Clone, Box<dyn std::error::Error>> {
        let mut advice_tape = vec![];
        advice_tape.extend(
            // This should probably be on the stack
            Value::Nullable(
                self.ctx_public_key
                    .clone()
                    .map(|pk| Box::new(Value::PublicKey(pk))),
            )
            .serialize(),
        );
        advice_tape.extend(self.this_value()?.serialize());
        for (i, t) in self.abi.param_types.iter().enumerate() {
            advice_tape.extend_from_slice(&t.parse(&self.args[i])?.serialize());
        }

        Ok(miden::MemAdviceProvider::from(
            miden::AdviceInputs::default().with_stack_values(advice_tape)?,
        ))
    }
}

pub fn prove(program: &Program, inputs: &Inputs) -> Result<Output, Box<dyn std::error::Error>> {
    let (output, prove) = run(&program, inputs)?;
    let proof = prove()?;

    Ok(Output {
        self_destructed: output.self_destructed()?,
        new_this: output.this(&inputs.abi)?,
        new_hash: output.stack[0..4].try_into()?,
        proof: proof.to_bytes(),
        stack: output.stack.clone(),
        run_output: output,
    })
}

#[derive(Debug)]
pub struct RunOutput {
    stack: Vec<u64>,
    memory: HashMap<u64, [u64; 4]>,
}

impl RunOutput {
    pub fn hash(&self) -> &[u64] {
        &self.stack[0..4]
    }

    pub fn logs(&self) -> Vec<String> {
        let get_mem_value = |addr: u64| {
            self.memory
                .get(&addr)
                .map(|word| mont_red_cst(word[0] as u128))
        };
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

        log_messages
    }

    pub fn this(&self, abi: &Abi) -> Result<Value, Box<dyn std::error::Error>> {
        let Some(this_type) = &abi.this_type else {
            return Err("Missing this type".into());
        };

        let Some(this_addr) = abi.this_addr else {
            return Err("Missing this addr".into());
        };

        this_type.read(
            &|addr| {
                self.memory
                    .get(&addr)
                    .map(|x| x.map(|v| mont_red_cst(v as u128)))
            },
            this_addr as u64,
        )
    }

    pub fn self_destructed(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let self_destructed = self.stack[4];
        if self_destructed == 0 {
            Ok(false)
        } else if self_destructed == 1 {
            Ok(true)
        } else {
            Err(format!("Invalid self destructed value: {}", self_destructed).into())
        }
    }
}

pub fn run<'a>(
    program: &'a Program,
    inputs: &Inputs,
) -> Result<
    (
        RunOutput,
        impl FnOnce() -> Result<ExecutionProof, Box<dyn std::error::Error>> + 'a,
    ),
    Box<dyn std::error::Error + 'static>,
> {
    let input_stack = inputs.stack()?;
    let advice_tape = inputs.advice_tape()?;

    let mut last_ok_state = None;
    let mut err = None;

    for state in miden_processor::execute_iter(program, input_stack.clone(), advice_tape.clone()) {
        match state {
            Ok(state) => {
                last_ok_state = Some(state);
            }
            Err(e) => {
                err = Some(e);
            }
        }
    }

    let last_ok_state = match (last_ok_state, err) {
        (None, Some(e)) => {
            return Err(Box::new(e));
        }
        (Some(state), Some(e)) => {
            let Value::String(s) = Type::String.read(
                &|addr| {
                    state
                        .memory
                        .iter()
                        .find(|(a, _)| *a == addr)
                        .map(|(_, x)| x.map(|x| mont_red_cst(x.inner() as u128)))
                },
                1,
            )? else {
                return Err(Box::new(e));
            };

            if s.is_empty() {
                return Err(Box::new(e));
            } else {
                return Err(format!("{}: {}", s, e).into());
            }
        }
        (Some(state), _) => state,
        (None, None) => unreachable!(),
    };

    let output_stack = last_ok_state
        .stack
        .iter()
        .map(|x| mont_red_cst(x.inner() as _))
        .collect::<Vec<_>>();

    let memory = last_ok_state
        .memory
        .iter()
        .map(|(addr, word)| {
            (
                *addr,
                [
                    word[0].inner(),
                    word[1].inner(),
                    word[2].inner(),
                    word[3].inner(),
                ],
            )
        })
        .collect::<HashMap<_, _>>();

    Ok((
        RunOutput {
            stack: output_stack,
            memory,
        },
        move || {
            let (_stack_outputs, proof) =
                miden::prove(&program, input_stack, advice_tape, ProofOptions::default())?;

            Ok(proof)
        },
    ))
}
