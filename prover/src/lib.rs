use std::collections::HashMap;

use abi::{publickey, Abi, Parser, Type, TypeReader, Value};
use error::prelude::*;
use miden::{ExecutionProof, ProofOptions};
use miden_processor::{
    math::Felt, utils::Serializable, Program, ProgramInfo, StackInputs, StackOutputs,
};
use polylang::compiler;

#[derive(Debug)]
enum MidenError {
    Assembly(miden::AssemblyError),
    Execution(miden::ExecutionError),
    Input(miden::InputError),
}

impl std::fmt::Display for MidenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidenError::Assembly(e) => write!(f, "{}", e),
            MidenError::Execution(e) => write!(f, "{}", e),
            MidenError::Input(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for MidenError {}

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
    pub input_stack: Vec<u64>,
    pub self_destructed: bool,
    pub new_this: Value,
    pub new_hashes: Vec<[u64; 4]>,
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

pub fn hash_this(type_: Type, this: &Value, salts: Option<&[u32]>) -> Result<[u64; 4]> {
    let hasher_program = compiler::compile_hasher(type_, salts)?;

    let assembler = miden::Assembler::default()
        .with_library(&miden_stdlib::StdLibrary::default())
        .map_err(MidenError::Assembly)
        .wrap_err()?;

    let program = assembler
        .compile(hasher_program)
        .map_err(MidenError::Assembly)
        .wrap_err()?;

    let execution_result = miden::execute(
        &program,
        miden::StackInputs::default(),
        miden::MemAdviceProvider::from(
            miden::AdviceInputs::default()
                .with_stack_values(this.serialize().into_iter())
                .map_err(MidenError::Input)
                .wrap_err()?,
        ),
    )
    .map_err(MidenError::Execution)
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
        .map_err(MidenError::Assembly)
        .wrap_err()?;

    assembler
        .compile(miden_code)
        .map_err(MidenError::Assembly)
        .wrap_err()
}

pub struct Inputs {
    pub abi: Abi,
    pub ctx_public_key: Option<publickey::Key>,
    pub this: serde_json::Value,
    pub this_field_hashes: Vec<[u64; 4]>,
    pub this_salts: Vec<u32>,
    pub args: Vec<serde_json::Value>,
    /// Map from contract name to a vector of record value and field salts
    pub other_records: HashMap<String, Vec<(serde_json::Value, Vec<u32>)>>,
}

impl Inputs {
    pub fn new(
        abi: Abi,
        ctx_public_key: Option<publickey::Key>,
        this_salts: Vec<u32>,
        this: serde_json::Value,
        args: Vec<serde_json::Value>,
        other_records: HashMap<String, Vec<(serde_json::Value, Vec<u32>)>>,
    ) -> Result<Self> {
        let this_field_hashes = if let Some(Type::Struct(this_struct)) = &abi.this_type {
            this_struct
                .fields
                .iter()
                .map(|(k, _)| {
                    this_struct
                        .fields
                        .iter()
                        .find_map(|(k2, _)| if k == k2 { Some(k) } else { None })
                        .unwrap()
                })
                .enumerate()
                .map(|(i, k)| {
                    let field_value = this
                        .get(k)
                        .ok_or_else(|| Error::simple(format!("Missing field `{}`", k)))?;
                    let field_type = this_struct
                        .fields
                        .iter()
                        .find_map(|(k2, t)| if k == k2 { Some(t) } else { None })
                        .unwrap();

                    let field_value = Parser::parse(field_type, field_value)?;

                    hash_this(field_type.clone(), &field_value, Some(&[this_salts[i]]))
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            vec![]
        };

        Ok(Self {
            abi,
            ctx_public_key,
            this,
            this_field_hashes,
            this_salts,
            args,
            other_records,
        })
    }

    pub fn stack_values(
        &self,
        other_records: &HashMap<String, Vec<(Type, Value, Value, Vec<u32>)>>,
    ) -> Vec<u64> {
        let mut other_record_hashes = vec![];
        for or in &self.abi.other_records {
            let records = other_records.get(&or.contract).unwrap();
            let struct_ = self
                .abi
                .other_contract_types
                .iter()
                .find_map(|t| match t {
                    Type::Struct(s) if s.name == or.contract => Some(s),
                    _ => None,
                })
                .unwrap();

            let mut record_hashes = vec![];
            for (_, _, record, salts) in records {
                record_hashes.push(Value::Hash(
                    hash_this(Type::Struct(struct_.clone()), record, Some(salts)).unwrap(),
                ));
            }

            other_record_hashes.push(Value::Array(record_hashes));
        }

        let dependent_fields = if let Some(Type::Struct(this_struct)) = &self.abi.this_type {
            this_struct
                .fields
                .iter()
                .map(|(k, _)| {
                    self.abi
                        .dependent_fields
                        .iter()
                        .find(|(k2, _)| k == k2)
                        .is_some()
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        [
            self.this_field_hashes
                .iter()
                .enumerate()
                .filter_map(|(i, x)| if dependent_fields[i] { Some(*x) } else { None })
                .flatten()
                .collect::<Vec<_>>(),
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

    pub fn stack(
        &self,
        other_records: &HashMap<String, Vec<(Type, Value, Value, Vec<u32>)>>,
    ) -> Result<StackInputs> {
        StackInputs::try_from_values(self.stack_values(other_records))
            .map_err(MidenError::Input)
            .wrap_err()
    }

    fn this_value(&self) -> Result<Value> {
        let Some(this_type) = &self.abi.this_type else {
            return Err(Error::simple("Missing this type"));
        };

        json_to_this_value(&self.this, this_type)
    }

    /// Returns a map from contract name to a vector of record id type, record id and record value.
    fn other_records(&self) -> Result<HashMap<String, Vec<(Type, Value, Value, Vec<u32>)>>> {
        let mut result = HashMap::new();

        for x in &self.abi.other_records {
            let records = self.other_records.get(&x.contract);
            let struct_ = self
                .abi
                .other_contract_types
                .iter()
                .find_map(|t| match t {
                    Type::Struct(s) if s.name == x.contract => Some(s),
                    _ => None,
                })
                .unwrap();

            let mut contract_records = Vec::new();
            for (record, salts) in records.iter().map(|r| r.iter()).flatten() {
                let record = json_to_this_value(record, &Type::Struct(struct_.clone()))?;

                contract_records.push((
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
                    salts.clone(),
                ));
            }

            result.insert(x.contract.clone(), contract_records);
        }

        Ok(result)
    }

    fn all_known_records(
        &self,
        other_records: &HashMap<String, Vec<(Type, Value, Value, Vec<u32>)>>,
    ) -> Result<Vec<(Type, Value)>> {
        let mut result = vec![];

        let this_value = self.this_value()?;
        let known_records = other_records
            .iter()
            .map(|(_, r)| r.iter())
            .flatten()
            .map(|(_, _, r, _)| r)
            .chain([&this_value]);

        for known_record in known_records {
            known_record
                .visit(&mut |value| {
                    if let Value::ContractReference(id) = value {
                        result.push((Type::String, Value::String(String::from_utf8(id.clone())?)));
                    }

                    Ok::<_, std::string::FromUtf8Error>(())
                })
                .wrap_err()?;
        }

        Ok(result)
    }

    fn advice_provider(
        &self,
        other_records: &HashMap<String, Vec<(Type, Value, Value, Vec<u32>)>>,
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

        if let Some(Type::Struct(this_struct)) = &self.abi.this_type {
            for (i, _) in this_struct.fields.iter().enumerate() {
                advice_tape.push(self.this_salts[i] as u64);
            }
        }

        for (i, t) in self.abi.param_types.iter().enumerate() {
            advice_tape.extend_from_slice(&t.parse(&self.args[i])?.serialize());
        }

        let mut advice_map = Vec::<([u8; 32], _)>::new();

        let Value::StructValue(this_value) = self.this_value()? else {
            return Err(Error::simple("This value is not a struct"));
        };

        for (i, (_, value)) in this_value.into_iter().enumerate() {
            let key = [
                Felt::new(self.abi.this_addr.unwrap() as u64 + i as u64),
                Felt::new(0),
                Felt::new(0),
                Felt::new(1),
            ];

            advice_map.push((
                key.iter()
                    .map(|f| f.to_bytes())
                    .flatten()
                    .collect::<Vec<u8>>()
                    .try_into()
                    .unwrap(),
                value
                    .serialize()
                    .into_iter()
                    .map(Felt::from)
                    .collect::<Vec<_>>(),
            ));
        }

        for (_contract, records) in other_records {
            for (position, (id_type, id, record, salts)) in records.iter().enumerate() {
                let id_hash = hash_this(id_type.clone(), id, None)?;

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
                        .chain(
                            salts
                                .iter()
                                .flat_map(|s| Value::UInt32(*s as u32).serialize().into_iter()),
                        )
                        .chain(record.serialize().into_iter())
                        .map(Felt::from)
                        .collect(),
                ));
            }
        }

        for (id_type, id_value) in self.all_known_records(other_records)? {
            let id_hash = hash_this(id_type, &id_value, None)?;
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
                .map_err(MidenError::Input)
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
        new_hashes: output.hashes(),
        proof: proof.0.to_bytes(),
        stack: output.stack.clone(),
        input_stack: output.input_stack.clone(),
        run_output: output,
    })
}

#[derive(Debug)]
pub struct RunOutput {
    abi: Abi,
    memory: HashMap<u64, [u64; 4]>,
    pub cycle_count: u32,
    pub stack: Vec<u64>,
    pub input_stack: Vec<u64>,
}

impl RunOutput {
    pub fn hashes(&self) -> Vec<[u64; 4]> {
        let mut hashes = Vec::new();

        let hashes_offset = 1;
        for (i, _) in self.abi.dependent_fields.iter().enumerate() {
            let offset = hashes_offset + i * 4;
            let field_hash = &self.stack[offset..offset + 4];

            hashes.push(field_hash.try_into().unwrap());
        }

        hashes
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
                Some(
                    self.memory
                        .get(&addr)
                        .map(|x| x.map(|v| mont_red_cst(v as u128)))
                        .unwrap_or_default(),
                )
            },
            this_addr as u64,
        )
    }

    pub fn result(&self, abi: &Abi) -> Result<Value> {
        let Some(result_type) = &abi.result_type else {
            return Ok(abi::Value::Nullable(None));
        };

        let Some(result_addr) = abi.result_addr else {
            return Err(Error::simple("Missing result addr"));
        };

        result_type.read(
            &|addr| {
                Some(
                    self.memory
                        .get(&addr)
                        .map(|x| x.map(|v| mont_red_cst(v as u128)))
                        .unwrap_or_default(),
                )
            },
            result_addr as u64,
        )
    }

    pub fn result_hash(&self, abi: &Abi) -> Option<[u64; 4]> {
        if abi.result_type.is_none() {
            return None;
        }

        let offset = self.abi.dependent_fields.len() * 4 + 1; // + 1 for self_destructed
        let result_hash = [
            self.stack[offset],
            self.stack[offset + 1],
            self.stack[offset + 2],
            self.stack[offset + 3],
        ];

        Some(result_hash)
    }

    pub fn self_destructed(&self) -> Result<bool> {
        let self_destructed = self.stack[0];
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
        let offset = self.abi.dependent_fields.len() * 4 + 1; // + 1 for self_destructed
        self.stack[offset] == 1
    }
}

pub fn run<'a>(
    program: &'a Program,
    inputs: &Inputs,
) -> Result<(
    RunOutput,
    impl FnOnce() -> Result<(ExecutionProof, StackOutputs)> + 'a,
)> {
    let other_records = inputs.other_records()?;
    let input_stack = inputs.stack(&other_records)?;
    let advice_tape = inputs.advice_provider(&other_records)?;

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
                    err = Some(Error::wrapped(Box::new(MidenError::Execution(e))));
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
            )?
            else {
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

    let input_stack_values = input_stack
        .values()
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
            abi: inputs.abi.clone(),
            stack: output_stack,
            cycle_count: last_ok_state.clk,
            input_stack: input_stack_values,
            memory,
        },
        move || {
            let (stack_outputs, proof) =
                miden_prover::prove(program, input_stack, advice_tape, ProofOptions::default())
                    .map_err(MidenError::Execution)
                    .wrap_err()?;

            Ok((proof, stack_outputs))
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
