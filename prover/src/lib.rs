use abi::{publickey, Abi, Parser, Type, TypeReader, Value};
use error::prelude::*;
use miden::{ExecutionProof, ProofOptions};
use miden_processor::{math::Felt, utils::Serializable, Program, ProgramInfo, StackInputs};
use polylang::compiler;
use std::{collections::HashMap, fmt::Debug};

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

fn json_to_this_value(this_json: &serde_json::Value, this_type: &Type) -> Result<Value> {
    let Type::Struct(struct_) = this_type else {
        return Err(Error::simple("This type is not a struct"));
    };

    let use_defaults = this_json.as_object().map(|o| o.is_empty()).unwrap_or(false);

    let mut struct_values = Vec::new();
    for (field_name, field_type) in &struct_.fields {
        let field_value = match this_json.get(field_name) {
            Some(value) => Parser::parse(field_type, value)?,
            None if use_defaults => field_type.default_value(),
            None if matches!(field_type, Type::Nullable(_)) => field_type.default_value(),
            None => {
                return Err(Error::simple(format!(
                    "missing value for field `{}`",
                    field_name
                )))
            }
        };

        struct_values.push((field_name.clone(), field_value));
    }

    Ok(Value::StructValue(struct_values))
}

pub fn hash_this(type_: Type, this: &Value) -> Result<[u64; 4]> {
    let hasher_program = compiler::compile_hasher(type_)?;

    let assembler = miden::Assembler::default()
        .with_library(&miden_stdlib::StdLibrary::default())
        .wrap_err()?;

    let program = assembler.compile(hasher_program).wrap_err()?;

    let execution_result = miden::execute(
        &program,
        miden::StackInputs::default(),
        miden::MemAdviceProvider::from(
            miden::AdviceInputs::default()
                .with_stack_values(this.serialize().into_iter())
                .wrap_err()?,
        ),
    )
    .wrap_err()?;

    execution_result.stack_outputs().stack()[0..4]
        .try_into()
        .wrap_err()
}

pub fn compile_program(abi: &Abi, miden_code: &str) -> Result<Program> {
    let std_library = match &abi.std_version {
        None => miden_stdlib::StdLibrary::default(),
        Some(version) => match version {
            abi::StdVersion::V0_5_0 => unimplemented!("Unsupported std version: 0.5.0"),
            abi::StdVersion::V0_6_1 => miden_stdlib::StdLibrary::default(),
        },
    };
    let assembler = miden::Assembler::default()
        .with_library(&std_library)
        .wrap_err()?;

    assembler.compile(miden_code).wrap_err()
}

pub struct Inputs {
    pub abi: Abi,
    pub ctx_public_key: Option<publickey::Key>,
    pub this: serde_json::Value,
    pub this_hash: [u64; 4],
    pub args: Vec<serde_json::Value>,
    pub other_records: HashMap<String, Vec<serde_json::Value>>,
}

impl Inputs {
    pub fn stack_values(
        &self,
        other_records: &HashMap<String, Vec<(Type, Value, Value)>>,
    ) -> Vec<u64> {
        let mut other_record_hashes = vec![];
        for x in &self.abi.other_records {
            let records = other_records.get(&x.collection).unwrap();
            let struct_ = self
                .abi
                .other_collection_types
                .iter()
                .find_map(|t| match t {
                    Type::Struct(s) if s.name == x.collection => Some(s),
                    _ => None,
                })
                .unwrap();

            let mut record_hashes = vec![];
            for (_, _, record) in records {
                record_hashes.push(Value::Hash(
                    hash_this(Type::Struct(struct_.clone()), record).unwrap(),
                ));
            }

            other_record_hashes.push(Value::Array(record_hashes));
        }

        [
            self.this_hash.iter().cloned().collect::<Vec<_>>(),
            other_record_hashes
                .into_iter()
                .map(|x| x.serialize())
                .flatten()
                .collect::<Vec<_>>(),
        ]
        .into_iter()
        .flatten()
        .rev()
        .collect::<Vec<_>>()
    }

    fn stack(
        &self,
        other_records: &HashMap<String, Vec<(Type, Value, Value)>>,
    ) -> Result<StackInputs> {
        StackInputs::try_from_values(self.stack_values(other_records)).wrap_err()
    }

    fn this_value(&self) -> Result<Value> {
        let Some(this_type) = &self.abi.this_type else {
            return Err(Error::simple("Missing this type"));
        };

        json_to_this_value(&self.this, this_type)
    }

    /// Returns a map from collection name to a vector of record id type, record id and record value.
    fn other_records(&self) -> Result<HashMap<String, Vec<(Type, Value, Value)>>> {
        let mut result = HashMap::new();

        for x in &self.abi.other_records {
            let records = self.other_records.get(&x.collection);
            let struct_ = self
                .abi
                .other_collection_types
                .iter()
                .find_map(|t| match t {
                    Type::Struct(s) if s.name == x.collection => Some(s),
                    _ => None,
                })
                .unwrap();

            let mut collection_records = Vec::new();
            for record in records.iter().map(|r| r.iter()).flatten() {
                let record = json_to_this_value(record, &Type::Struct(struct_.clone()))?;

                collection_records.push((
                    struct_
                        .fields
                        .iter()
                        .find_map(|(k, t)| if k == "id" { Some(t.clone()) } else { None })
                        .unwrap(),
                    match &record {
                        Value::StructValue(fields) => fields
                            .iter()
                            .find_map(|(k, v)| if k == "id" { Some(v) } else { None })
                            .unwrap()
                            .clone(),
                        _ => unreachable!(),
                    },
                    record,
                ));
            }

            result.insert(x.collection.clone(), collection_records);
        }

        Ok(result)
    }

    fn all_known_records(
        &self,
        other_records: &HashMap<String, Vec<(Type, Value, Value)>>,
    ) -> Result<Vec<(Type, Value)>> {
        let mut result = vec![];

        let this_value = self.this_value()?;
        let known_records = other_records
            .iter()
            .map(|(_, r)| r.iter())
            .flatten()
            .map(|(_, _, r)| r)
            .chain([&this_value]);

        for known_record in known_records {
            known_record
                .visit(&mut |value| {
                    if let Value::CollectionReference(id) = value {
                        result.push((Type::String, Value::String(String::from_utf8(id.clone())?)));
                    }

                    Ok::<_, std::string::FromUtf8Error>(())
                })
                .wrap_err()?;
        }

        Ok(result)
    }

    fn advice_tape(
        &self,
        other_records: &HashMap<String, Vec<(Type, Value, Value)>>,
    ) -> Result<miden::MemAdviceProvider> {
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

        let mut advice_map = Vec::<([u8; 32], _)>::new();
        for (_collection, records) in other_records {
            for (position, (id_type, id, record)) in records.iter().enumerate() {
                let id_hash = hash_this(id_type.clone(), id)?;

                advice_map.push((
                    {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(
                            &id_hash
                                .into_iter()
                                .rev()
                                .map(Felt::new)
                                .map(|f| f.to_bytes())
                                .flatten()
                                .collect::<Vec<u8>>(),
                        );
                        arr
                    },
                    Value::Nullable(Some(Box::new(Value::UInt32(position as u32))))
                        .serialize()
                        .into_iter()
                        .chain(record.serialize().into_iter())
                        .map(Felt::from)
                        .collect(),
                ));
            }
        }

        for (id_type, id_value) in self.all_known_records(other_records)? {
            let id_hash = hash_this(id_type, &id_value)?;
            let id_hash = {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(
                    &id_hash
                        .into_iter()
                        .rev()
                        .map(Felt::new)
                        .map(|f| f.to_bytes())
                        .flatten()
                        .collect::<Vec<u8>>(),
                );
                arr
            };

            if advice_map.iter().find(|(k, _)| *k == id_hash).is_none() {
                advice_map.push((
                    id_hash,
                    Value::Nullable(None)
                        .serialize()
                        .into_iter()
                        .map(Felt::from)
                        .collect(),
                ));
            }
        }

        Ok(miden::MemAdviceProvider::from(
            miden::AdviceInputs::default()
                .with_stack_values(advice_tape)
                .wrap_err()?
                .with_map(advice_map),
        ))
    }
}

pub fn prove(program: &Program, inputs: &Inputs) -> Result<Output> {
    let (output, prove) = run(program, inputs)?;
    let proof = prove()?;

    Ok(Output {
        self_destructed: output.self_destructed()?,
        new_this: output.this(&inputs.abi)?,
        new_hash: output.stack[0..4].try_into().wrap_err()?,
        proof: proof.to_bytes(),
        stack: output.stack.clone(),
        run_output: output,
    })
}

#[derive(Debug)]
pub struct RunOutput {
    memory: HashMap<u64, [u64; 4]>,
    stack: Vec<u64>,
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

    pub fn this(&self, abi: &Abi) -> Result<Value> {
        let Some(this_type) = &abi.this_type else {
            return Err(Error::simple("Missing this type"));
        };

        let Some(this_addr) = abi.this_addr else {
            return Err(Error::simple("Missing this addr"));
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

    pub fn self_destructed(&self) -> Result<bool> {
        let self_destructed = self.stack[4];
        if self_destructed == 0 {
            Ok(false)
        } else if self_destructed == 1 {
            Ok(true)
        } else {
            Err(Error::simple(format!(
                "Invalid self destructed value: {}",
                self_destructed
            )))
        }
    }

    pub fn read_auth(&self) -> bool {
        self.stack[5] == 1
    }
}

pub fn run<'a>(
    program: &'a Program,
    inputs: &Inputs,
) -> Result<(RunOutput, impl FnOnce() -> Result<ExecutionProof> + 'a)> {
    let other_records = inputs.other_records()?;
    let input_stack = inputs.stack(&other_records)?;
    let advice_tape = inputs.advice_tape(&other_records)?;

    let mut last_ok_state = None;
    let mut err = None;

    for state in miden_processor::execute_iter(program, input_stack.clone(), advice_tape.clone()) {
        match state {
            Ok(state) => {
                last_ok_state = Some(state);
            }
            Err(e) => {
                // TODO: store vector of errors instead.
                if err.is_none() {
                    err = Some(Error::wrapped(Box::new(e)));
                }
            }
        }
    }

    let last_ok_state = match (last_ok_state, err) {
        (None, Some(e)) => {
            return Err(e);
        }
        (Some(state), Some(e)) => {
            if state.memory.iter().find(|(a, _)| *a == 1).is_none() {
                return Err(e);
            }

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
                return Err(e);
            };

            return if s.is_empty() {
                Err(e)
            } else {
                Err(Error::simple(format!("{}: {}", s, e)))
            };
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
                miden::prove(program, input_stack, advice_tape, ProofOptions::default())
                    .wrap_err()?;

            Ok(proof)
        },
    ))
}

pub trait ProgramExt {
    fn to_program_info_bytes(self) -> Vec<u8>;
}

impl ProgramExt for Program {
    fn to_program_info_bytes(self) -> Vec<u8> {
        ProgramInfo::from(self).to_bytes()
    }
}
