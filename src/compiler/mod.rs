mod array;
mod boolean;
mod bytes;
mod encoder;
mod float32;
mod float64;
mod int32;
mod int64;
mod ir;
mod map;
mod nullable;
mod publickey;
mod string;
mod uint32;
mod uint64;

use std::collections::HashMap;

use abi::{Abi, PrimitiveType, StdVersion, Struct, Type};
use error::prelude::*;

use crate::ast::{self, Expression, Statement};

#[derive(Debug, Clone)]
enum TypeConstraint {
    Exact(Type),
    Array,
}

impl TypeConstraint {
    fn matches(&self, type_: &Type) -> bool {
        match self {
            TypeConstraint::Exact(expected) => expected == type_,
            TypeConstraint::Array => matches!(type_, Type::Array(_)),
        }
    }
}

macro_rules! comment {
    ($compiler:expr, $($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $compiler.comment(format!($($arg)*));
    };
}

lazy_static::lazy_static! {
    // TODO: fix early return, so that we can do `if (length == 0) return '0';`
    static ref UINT32_TO_STRING: ast::Function = polylang_parser::parse_function(r#"
        function uint32ToString(value: number): string {
            let isZero = value == 0;

            let length = 0;
            let i = value;
            while (i >= 1) {
                i = i / 10;
                length = length + 1;
            }

            if (isZero) length = 1;

            let dataPtr = dynamicAlloc(length); 

            let offset = length;
            while (value >= 1) {
                offset = offset - 1;
                let digit = value % 10;
                value = value / 10;
                writeMemory(dataPtr + offset, digit + 48);
            }
    
            if (isZero) {
                writeMemory(dataPtr, 48);
            }
            
            return unsafeToString(length, dataPtr);
        }
    "#).unwrap();
    // TODO: rewrite this in raw instructions for better performance
    // TODO: We shouldn't have to copy the current message into a new string, but we do because `addressOf(message)` is always the same. This error surfaces when we try to log in a for or while loop.
    static ref LOG_STRING: ast::Function = polylang_parser::parse_function(r#"
        function logString(message: string) {
            let currentLog = dynamicAlloc(u32_(2));
            writeMemory(currentLog, deref(addressOf(message)));
            writeMemory(currentLog + u32_(1), deref(addressOf(message) + u32_(1)));

            let newLog = dynamicAlloc(u32_(2));
            writeMemory(newLog, deref(u32_(4)));
            writeMemory(newLog + u32_(1), deref(u32_(5)));
            writeMemory(u32_(4), newLog);
            writeMemory(u32_(5), currentLog);
        }
    "#).unwrap();
    static ref BUILTINS_SCOPE: &'static Scope<'static, 'static> = {
        let mut scope = Scope::new();

        for (name, type_, func) in HIDDEN_BUILTINS.iter() {
            match type_ {
                None => scope.add_function(name.clone(), func.clone()),
                Some(type_) => scope.add_method(TypeConstraint::Exact(type_.clone()), name.clone(), func.clone()),
            }
        }

        for (name, type_, func) in USABLE_BUILTINS.iter() {
            match type_ {
                None => scope.add_function(name.clone(), func.clone()),
                Some(type_) => scope.add_method(type_.clone(), name.clone(), func.clone()),
            }
        }

        Box::leak(Box::new(scope))
    };
    static ref HIDDEN_BUILTINS: &'static [(String, Option<Type>, Function<'static>)] = {
        let mut builtins = Vec::new();

        builtins.push((
            "hiddenNoopMarker".to_string(),
            None,
            Function::Builtin(|_, _, _| {
                panic!("this function should never be called");
            }),
        ));

        builtins.push((
            "dynamicAlloc".to_string(),
            None,
            Function::Builtin(|compiler, _scope, args| dynamic_alloc(compiler, args)),
        ));

        builtins.push((
            "writeMemory".to_string(),
            None,
            Function::Builtin(|compiler, _, args| {
                ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });
                let address = args.get(0).unwrap();
                let value = args.get(1).unwrap();

                ensure_eq_type!(address, Type::PrimitiveType(PrimitiveType::UInt32));
                ensure_eq_type!(value, Type::PrimitiveType(PrimitiveType::UInt32));

                compiler.memory.read(
                    compiler.instructions,
                    value.memory_addr,
                    value.type_.miden_width(),
                );
                // [value]
                compiler.memory.read(
                    compiler.instructions,
                    address.memory_addr,
                    address.type_.miden_width(),
                );
                // [address, value]
                compiler
                    .instructions
                    .push(encoder::Instruction::MemStore(None));
                // []

                Ok(Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::UInt32),
                    memory_addr: 0,
                })
            }),
        ));

        builtins.push((
            "readAdvice".to_string(),
            None,
            Function::Builtin(|compiler, _, _| {
                let symbol = compiler
                    .memory
                    .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

                compiler.instructions.push(encoder::Instruction::AdvPush(1));
                compiler.memory.write(
                    compiler.instructions,
                    symbol.memory_addr,
                    &[ValueSource::Stack],
                );

                Ok(symbol)
            }),
        ));

        builtins.push((
            "unsafeToString".to_string(),
            None,
            Function::Builtin(|compiler, _, args| {
                ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });
                let length = args.get(0).unwrap();
                let address_ptr = args.get(1).unwrap();

                ensure_eq_type!(length, Type::PrimitiveType(PrimitiveType::UInt32));
                ensure_eq_type!(address_ptr, Type::PrimitiveType(PrimitiveType::UInt32));

                let two = uint32::new(compiler, 2);
                let mut s = dynamic_alloc(compiler, &[two])?;
                s.type_ = Type::String;

                compiler.memory.read(
                    compiler.instructions,
                    length.memory_addr,
                    length.type_.miden_width(),
                );
                compiler.memory.write(
                    compiler.instructions,
                    string::length(&s).memory_addr,
                    &vec![ValueSource::Stack; length.type_.miden_width() as _],
                );

                compiler.memory.read(
                    compiler.instructions,
                    address_ptr.memory_addr,
                    address_ptr.type_.miden_width(),
                );
                compiler.memory.write(
                    compiler.instructions,
                    string::data_ptr(&s).memory_addr,
                    &vec![ValueSource::Stack; address_ptr.type_.miden_width() as _],
                );

                Ok(s)
            }),
        ));

        builtins.push((
            "unsafeToBytes".to_string(),
            None,
            Function::Builtin(|compiler, _, args| {
                ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });
                let length = args.get(0).unwrap();
                let address_ptr = args.get(1).unwrap();

                ensure_eq_type!(length, Type::PrimitiveType(PrimitiveType::UInt32));
                ensure_eq_type!(address_ptr, Type::PrimitiveType(PrimitiveType::UInt32));

                let s = compiler.memory.allocate_symbol(Type::Bytes);

                compiler.memory.read(
                    compiler.instructions,
                    length.memory_addr,
                    length.type_.miden_width(),
                );
                compiler.memory.write(
                    compiler.instructions,
                    string::length(&s).memory_addr,
                    &vec![ValueSource::Stack; length.type_.miden_width() as _],
                );

                compiler.memory.read(
                    compiler.instructions,
                    address_ptr.memory_addr,
                    address_ptr.type_.miden_width(),
                );
                compiler.memory.write(
                    compiler.instructions,
                    string::data_ptr(&s).memory_addr,
                    &vec![ValueSource::Stack; address_ptr.type_.miden_width() as _],
                );

                Ok(s)
            }),
        ));

        builtins.push((
            "unsafeToPublicKey".to_string(),
            None,
            Function::Builtin(|compiler, _, args| {
                let [kty, crv, alg, use_, extra_ptr] = args else {
                    return ArgumentsCountSnafu { found: args.len(), expected: 5usize }.fail().map_err(Into::into);
                };
                ensure_eq_type!(kty, Type::PrimitiveType(PrimitiveType::UInt32));
                ensure_eq_type!(crv, Type::PrimitiveType(PrimitiveType::UInt32));
                ensure_eq_type!(alg, Type::PrimitiveType(PrimitiveType::UInt32));
                ensure_eq_type!(use_, Type::PrimitiveType(PrimitiveType::UInt32));
                ensure_eq_type!(extra_ptr, Type::PrimitiveType(PrimitiveType::UInt32));

                let pk = compiler.memory.allocate_symbol(Type::PublicKey);

                compiler.memory.read(
                    compiler.instructions,
                    kty.memory_addr,
                    kty.type_.miden_width(),
                );

                compiler.memory.write(
                    compiler.instructions,
                    publickey::kty(&pk).memory_addr,
                    &vec![ValueSource::Stack; kty.type_.miden_width() as _],
                );

                compiler.memory.read(
                    compiler.instructions,
                    crv.memory_addr,
                    crv.type_.miden_width(),
                );

                compiler.memory.write(
                    compiler.instructions,
                    publickey::crv(&pk).memory_addr,
                    &vec![ValueSource::Stack; crv.type_.miden_width() as _],
                );

                compiler.memory.read(
                    compiler.instructions,
                    alg.memory_addr,
                    alg.type_.miden_width(),
                );

                compiler.memory.write(
                    compiler.instructions,
                    publickey::alg(&pk).memory_addr,
                    &vec![ValueSource::Stack; alg.type_.miden_width() as _],
                );

                compiler.memory.read(
                    compiler.instructions,
                    use_.memory_addr,
                    use_.type_.miden_width(),
                );

                compiler.memory.write(
                    compiler.instructions,
                    publickey::use_(&pk).memory_addr,
                    &vec![ValueSource::Stack; use_.type_.miden_width() as _],
                );

                compiler.memory.read(
                    compiler.instructions,
                    extra_ptr.memory_addr,
                    extra_ptr.type_.miden_width(),
                );

                compiler.memory.write(
                    compiler.instructions,
                    publickey::extra_ptr(&pk).memory_addr,
                    &vec![ValueSource::Stack; extra_ptr.type_.miden_width() as _],
                );

                Ok(pk)
            }),
        ));

        builtins.push(("deref".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });
            let address = args.get(0).unwrap();

            ensure_eq_type!(address, Type::PrimitiveType(PrimitiveType::UInt32));

            let result = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                compiler.instructions,
                address.memory_addr,
                address.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::MemLoad(None));
            compiler.memory.write(
                compiler.instructions,
                result.memory_addr,
                &[ValueSource::Stack],
            );

            Ok(result)
         })));

        builtins.push(("addressOf".to_string(), None, Function::Builtin(|compiler, _, args| {
           ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });
           let a = args.get(0).unwrap();
           Ok(uint32::new(compiler, a.memory_addr))
        })));


        builtins.push(("hashString".to_string(), None, Function::Builtin(|compiler, scope, args| string::hash(compiler, scope, args))));

        // bytes and contract reference have the same layout as strings,
        // so we can reuse the hashing function
        builtins.push(("hashBytes".to_owned(), None, Function::Builtin(|compiler, scope, args| string::hash(compiler, scope, args))));
        builtins.push(("hashContractReference".to_owned(), None, Function::Builtin(|compiler, scope, args| string::hash(compiler, scope, args))));

        builtins.push(("hashArray".to_owned(), None, Function::Builtin(array::hash)));

        builtins.push(("hashMap".to_owned(), None, Function::Builtin(|compiler, _scope, args| {
           ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });
           let map = args.get(0).unwrap();

           let (keys, values) = map::key_values_arr(map)?;

           let (_, _, hash_array_fn) = HIDDEN_BUILTINS.iter().find(|(name, _, _)| name == "hashArray").unwrap();

           let keys_hash = compile_function_call(compiler, hash_array_fn, &[keys], None)?.unwrap();
           let values_hash = compile_function_call(compiler, hash_array_fn, &[values], None)?.unwrap();

           let result = compiler
               .memory
               .allocate_symbol(Type::Hash);

           compiler.memory.read(
               compiler.instructions,
               keys_hash.memory_addr,
               keys_hash.type_.miden_width(),
           );
           compiler.memory.read(
               compiler.instructions,
               values_hash.memory_addr,
               values_hash.type_.miden_width(),
           );

           compiler.instructions.push(encoder::Instruction::HMerge);

           compiler.memory.write(
               compiler.instructions,
               result.memory_addr,
               &[ValueSource::Stack, ValueSource::Stack, ValueSource::Stack, ValueSource::Stack],
           );

           Ok(result)
       })));

       builtins.push(("hashPublicKey".to_owned(), None, Function::Builtin(|compiler, _, args| {
           publickey::hash(compiler, args)
       })));

       builtins.push((
           "uintToFloat".to_string(),
           None,
           Function::Builtin(|compiler, _scope, args| {
               ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });
               Ok(float32::from_uint32(compiler, &args[0]))
           })
       ));

       builtins.push((
           "intToFloat".to_string(),
           None,
           Function::Builtin(|compiler, _scope, args| {
               ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });
               Ok(float32::from_int32(compiler, &args[0]))
           })
       ));

       Box::leak(Box::new(builtins))
    };
    static ref USABLE_BUILTINS: &'static [(String, Option<TypeConstraint>, Function<'static>)] = {
        let mut builtins = Vec::new();

        builtins.push((
            "assert".to_string(),
            None,
            Function::Builtin(|compiler, _, args| {
                ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });
                let condition = &args[0];
                let message = &args[1];

                ensure_eq_type!(condition, Type::PrimitiveType(PrimitiveType::Boolean));
                ensure_eq_type!(message, Type::String);

                let mut failure_branch = vec![];
                let mut failure_compiler = Compiler::new(&mut failure_branch, compiler.memory, compiler.root_scope);

                let error_fn = &USABLE_BUILTINS
                    .iter()
                    .find(|(name, _, _)| name == "error")
                    .unwrap()
                    .2;
                compile_function_call(&mut failure_compiler, error_fn, &[message.clone()], None)?;

                compiler.instructions.push(encoder::Instruction::If {
                    condition: vec![encoder::Instruction::MemLoad(Some(condition.memory_addr))],
                    then: vec![],
                    else_: failure_branch,
                });

                Ok(Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::Boolean),
                    memory_addr: 0,
                })
            }),
        ));

        builtins.push((
            "error".to_string(),
            None,
            Function::Builtin(|compiler, _, args| {
                ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });
                let message = &args[0];
                ensure_eq_type!(message, Type::String);

                let str_len = string::length(message);
                let str_data_ptr = string::data_ptr(message);

                compiler.memory.write(
                    compiler.instructions,
                    1,
                    &[ValueSource::Memory(str_len.memory_addr),
                        ValueSource::Memory(str_data_ptr.memory_addr)],
                );

                compiler
                    .instructions
                    .push(encoder::Instruction::Push(0));
                compiler
                    .instructions
                    .push(encoder::Instruction::Assert);

                Ok(Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::Boolean),
                    memory_addr: 0,
                })
            }),
        ));

        builtins.push((
            "log".to_string(),
            None,
            Function::Builtin(|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let mut scope = compiler.root_scope.deeper();
                let result = log(compiler, &mut scope, args);
                compiler.root_scope = old_root_scope;
                result
            }),
        ));

        builtins.push((
            "readAdviceString".to_string(),
            None,
            Function::Builtin(|compiler, _, args| {
                assert_eq!(args.len(), 0);

                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;

                let result = read_advice_string(compiler)?;
                compiler.root_scope = old_root_scope;
                Ok(result)
            }),
        ));

        builtins.push((
            "readAdviceBytes".to_string(),
            None,
            Function::Builtin(|compiler, _, _args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;

                let result_str = read_advice_string(compiler)?;
                let result = Symbol {
                    type_: Type::Bytes,
                    ..result_str
                };

                compiler.root_scope = old_root_scope;
                Ok(result)
            }),
        ));

        builtins.push((
            "readAdviceContractReference".to_string(),
            None,
            Function::Builtin(|compiler, _, _args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;

                let result_str = read_advice_string(compiler)?;
                let result = Symbol {
                    type_: Type::Bytes,
                    ..result_str
                };

                compiler.root_scope = old_root_scope;

                Ok(Symbol {
                    type_: Type::ContractReference { contract: "".to_owned() },
                    ..result
                })
            }),
        ));

        builtins.push((
            "readAdvicePublicKey".to_string(),
            None,
            Function::Builtin(|compiler, _, _args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;

                let result = read_advice_public_key(compiler);

                compiler.root_scope = old_root_scope;
                result
            }),
        ));

        builtins.push(("readAdviceUInt32".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.is_empty(), ArgumentsCountSnafu { found: args.len(), expected: 0usize });

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a u32
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            Ok(symbol)
        })));

        builtins.push(("readAdviceUInt64".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.is_empty(), ArgumentsCountSnafu { found: args.len(), expected: 0usize });

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

            // high
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);

            // low
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr + 1, &[ValueSource::Stack]);

            Ok(result)
        })));

        builtins.push(("readAdviceInt32".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.is_empty(), ArgumentsCountSnafu { found: args.len(), expected: 0usize });

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a u32
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            Ok(symbol)
        })));

        builtins.push(("readAdviceInt64".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.is_empty(), ArgumentsCountSnafu { found: args.len(), expected: 0usize });

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Int64));

            // high
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);

            // low
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr + 1, &[ValueSource::Stack]);

            Ok(result)
        })));

        builtins.push(("readAdviceFloat32".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.is_empty(), ArgumentsCountSnafu { found: args.len(), expected: 0usize });

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a u32
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Float32));
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            Ok(symbol)
        })));


        builtins.push(("readAdviceFloat64".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.is_empty(), ArgumentsCountSnafu { found: args.len(), expected: 0usize });

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Float64));

            // high
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);

            // low
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr + 1, &[ValueSource::Stack]);

            Ok(result)
        })));

        builtins.push(("readAdviceBoolean".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.is_empty(), ArgumentsCountSnafu { found: args.len(), expected: 0usize });

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a boolean
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            Ok(symbol)
        })));

        builtins.push(("uint32ToString".to_string(), None, Function::Builtin(|compiler, _, args| {
            let old_root_scope = compiler.root_scope;
            compiler.root_scope = &BUILTINS_SCOPE;
            let result = compile_ast_function_call(&UINT32_TO_STRING, compiler, args, None)?.unwrap();
            compiler.root_scope = old_root_scope;
            Ok(result)
        })));

        builtins.push(("wrappingAdd".to_string(), Some(TypeConstraint::Exact(Type::PrimitiveType(PrimitiveType::UInt32))), Function::Builtin(|compiler, _, args| {
            ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });
            let a = &args[0];
            let b = &args[1];
            ensure_eq_type!(a, Type::PrimitiveType(PrimitiveType::UInt32));
            ensure_eq_type!(b, Type::PrimitiveType(PrimitiveType::UInt32));

            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.read(
                compiler.instructions,
                b.memory_addr,
                b.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::U32WrappingAdd);
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            Ok(symbol)
        })));

        builtins.push(("uint32WrappingSub".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });
            let a = &args[0];
            let b = &args[1];
            ensure_eq_type!(a, Type::PrimitiveType(PrimitiveType::UInt32));
            ensure_eq_type!(b, Type::PrimitiveType(PrimitiveType::UInt32));

            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.read(
                compiler.instructions,
                b.memory_addr,
                b.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::U32WrappingSub);
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            Ok(symbol)
        })));

        builtins.push(("uint32WrappingMul".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });
            let a = &args[0];
            let b = &args[1];
            ensure_eq_type!(a, Type::PrimitiveType(PrimitiveType::UInt32));
            ensure_eq_type!(b, Type::PrimitiveType(PrimitiveType::UInt32));

            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.read(
                compiler.instructions,
                b.memory_addr,
                b.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::U32WrappingMul);
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            Ok(symbol)
        })));

        builtins.push(("uint32CheckedXor".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });
            let a = &args[0];
            let b = &args[1];
            ensure_eq_type!(a, Type::PrimitiveType(PrimitiveType::UInt32));
            ensure_eq_type!(b, Type::PrimitiveType(PrimitiveType::UInt32));

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.read(
                compiler.instructions,
                b.memory_addr,
                b.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::U32CheckedXOR);
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);
            Ok(result)
        })));

        builtins.push(("int32".to_string(), None, Function::Builtin(|compiler, _, args| {
            ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });
            let a = &args[0];
            ensure_eq_type!(a, Type::PrimitiveType(PrimitiveType::UInt32));

            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

            compiler.memory.read(
                compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);

            Ok(symbol)
        })));

        builtins.push((
            "toHex".to_string(),
            Some(TypeConstraint::Exact(Type::PublicKey)),
            Function::Builtin(|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let result = publickey::to_hex(compiler, args);
                compiler.root_scope = old_root_scope;
                Ok(result)
            }),
        ));

        builtins.push((
            "indexOf".to_string(),
            Some(TypeConstraint::Array),
            Function::Builtin(|compiler, _, args| {
                ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });

                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let result = array::find_index(compiler, &args[0], &args[1])?;
                compiler.root_scope = old_root_scope;

                Ok(result)
            }),
        ));

        builtins.push((
            "includes".to_string(),
            Some(TypeConstraint::Array),
            Function::Builtin(|compiler, _, args| {
                ensure!(args.len() == 2, ArgumentsCountSnafu { found: args.len(), expected: 2usize });

                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let result = array::includes(compiler, &args[0], &args[1])?;
                compiler.root_scope = old_root_scope;

                Ok(result)
            }),
        ));

        builtins.push((
            "push".to_string(),
            Some(TypeConstraint::Array),
            Function::Builtin(|compiler, scope, args| {
                array::push(compiler, scope, args)
            }),
        ));

        builtins.push((
            "splice".to_string(),
            Some(TypeConstraint::Array),
            Function::Builtin(|compiler, _scope, args| {
                ensure!(args.len() == 3, ArgumentsCountSnafu { found: args.len(), expected: 3usize });
                let arr = &args[0];
                let start = &args[1];
                let delete_count = &args[2];

                array::splice(compiler, arr, start, delete_count)
            }),
        ));

        builtins.push((
            "unshift".to_string(),
            Some(TypeConstraint::Array),
            Function::Builtin(|compiler, _, args| {
                let arr = &args[0];
                let args = &args[1..];
                array::unshift(compiler, arr, args)
            }),
        ));

        builtins.push((
            "slice".to_string(),
            Some(TypeConstraint::Array),
            Function::Builtin(|compiler, _scope, args| {
                ensure!(args.len() <= 3, ArgumentsCountSnafu { found: args.len(), expected: 3usize });
                let arr = &args[0];
                let start = args.get(1);
                let end = args.get(2);

                array::slice(compiler, arr, start.cloned(), end)
            }),
        ));

        builtins.push((
            "mapLength".to_string(),
            None,
            Function::Builtin(|_compiler, _scope, args| {
                ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });
                let m = &args[0];
                ensure_eq_type!(m, Type::Map(_, _));

                Ok(array::length(&map::keys_arr(m)?))
            })
        ));

        builtins.push((
            "selfdestruct".to_string(),
            None,
            Function::Builtin(|compiler, _scope, _args| {
                compiler.memory.write(
                    compiler.instructions,
                    6,
                    &[ValueSource::Immediate(1)],
                );

                Ok(Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::Boolean),
                    memory_addr: 0,
                })
            }),
        ));

        builtins.push((
            "hashRPO".to_string(),
            None,
            Function::Builtin(|compiler, _scope, args| {
                ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });

                array::hash_width_1(compiler, &args[0], array::HashFn::Rpo)
            }),
        ));

        builtins.push((
            "hashSHA256".to_string(),
            None,
            Function::Builtin(|compiler, _scope, args| {
                ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });

                array::hash_width_1(compiler, &args[0], array::HashFn::Sha256)
            }),
        ));

        builtins.push((
            "hashBlake3".to_string(),
            None,
            Function::Builtin(|compiler, _scope, args| {
                ensure!(args.len() == 1, ArgumentsCountSnafu { found: args.len(), expected: 1usize });

                array::hash_width_1(compiler, &args[0], array::HashFn::Blake3)
            }),
        ));

        builtins.extend(string::builtins());

        Box::leak(Box::new(builtins))
    };
}

fn struct_field(
    _compiler: &mut Compiler,
    struct_symbol: &Symbol,
    field_name: &str,
) -> Result<Symbol> {
    let struct_ = match &struct_symbol.type_ {
        Type::Struct(struct_) => struct_,
        Type::ContractReference { contract: _ } if field_name == "id" => {
            return Ok(Symbol {
                type_: Type::String,
                memory_addr: struct_symbol.memory_addr,
            });
        }
        Type::String if field_name == "length" => {
            return Ok(string::length(struct_symbol));
        }
        Type::Array(_) if field_name == "length" => {
            return Ok(array::length(struct_symbol));
        }
        t => {
            return Err(ErrorKind::TypeMismatch {
                context: format!("expected struct, got: {:?}", t),
            }
            .into())
        }
    };

    let mut offset = 0;
    for (name, field_type) in &struct_.fields {
        if name == field_name {
            return Ok(Symbol {
                type_: field_type.clone(),
                memory_addr: struct_symbol.memory_addr + offset,
            });
        }

        offset += field_type.miden_width();
    }

    NotFoundSnafu {
        type_name: "struct field",
        item: field_name,
    }
    .fail()
    .map_err(Into::into)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Symbol {
    type_: Type,
    memory_addr: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct ContractField {
    name: String,
    type_: Type,
    delegate: bool,
    read: bool,
}

#[derive(Debug, Clone)]
struct Contract<'ast> {
    name: String,
    fields: Vec<ContractField>,
    functions: Vec<(String, &'ast ast::Function)>,
    call_directive: bool,
    read_directive: bool,
}

impl From<Contract<'_>> for Struct {
    fn from(contract: Contract<'_>) -> Self {
        let mut fields = Vec::new();
        for field in contract.fields {
            fields.push((field.name, field.type_));
        }

        Struct {
            name: contract.name,
            fields,
        }
    }
}

type BuiltinFn = fn(&mut Compiler, &mut Scope, &[Symbol]) -> Result<Symbol>;

#[derive(Clone)]
enum Function<'ast> {
    Ast(&'ast ast::Function),
    Builtin(BuiltinFn),
}

impl std::fmt::Debug for Function<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Function::Ast(ast) => write!(f, "Function::AST({:?})", ast),
            Function::Builtin(_) => write!(f, "Function::Builtin"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Scope<'ast, 'b> {
    parent: Option<&'b Scope<'ast, 'b>>,
    symbols: Vec<(String, Symbol)>,
    non_null_symbol_addrs: Vec<u32>,
    functions: Vec<(String, Function<'ast>)>,
    methods: Vec<(TypeConstraint, String, Function<'ast>)>,
    contracts: Vec<(String, Contract<'ast>)>,
}

impl<'ast> Scope<'ast, '_> {
    fn new() -> Self {
        Scope {
            parent: None,
            symbols: vec![],
            non_null_symbol_addrs: vec![],
            functions: vec![],
            methods: vec![],
            contracts: vec![],
        }
    }

    fn deeper<'b>(&'b self) -> Scope<'ast, 'b> {
        Scope {
            parent: Some(self),
            symbols: vec![],
            non_null_symbol_addrs: vec![],
            functions: vec![],
            methods: vec![],
            contracts: vec![],
        }
    }

    fn add_symbol(&mut self, name: String, symbol: Symbol) {
        self.symbols.push((name, symbol));
    }

    fn find_symbol(&self, name: &str) -> Option<Symbol> {
        if let Some(symbol) = self
            .symbols
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s)
        {
            return Some(symbol.clone());
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.find_symbol(name);
        }

        None
    }

    fn add_function(&mut self, name: String, function: Function<'ast>) {
        self.functions.push((name, function));
    }

    fn find_function(&self, name: &str) -> Option<&Function<'ast>> {
        if let Some(func) = self
            .functions
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f)
        {
            return Some(func);
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.find_function(name);
        }

        None
    }

    fn add_method(&mut self, type_: TypeConstraint, name: String, function: Function<'ast>) {
        self.methods.push((type_, name, function));
    }

    fn find_method(&self, type_: &Type, name: &str) -> Option<&Function<'ast>> {
        if let Some(func) = self
            .methods
            .iter()
            .rev()
            .find(|(t, n, _)| n == name && t.matches(type_))
            .map(|(_, _, f)| f)
        {
            return Some(func);
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.find_method(type_, name);
        }

        None
    }

    fn add_contract(&mut self, name: String, contract: Contract<'ast>) {
        if self.find_contract(&name).is_some() {
            panic!("Contract {} already exists", name);
        }

        self.contracts.push((name, contract));
    }

    fn find_contract(&self, name: &str) -> Option<&Contract<'ast>> {
        if let Some(contract) = self
            .contracts
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c)
        {
            return Some(contract);
        }

        self.parent.and_then(|p| p.find_contract(name))
    }
}

#[derive(Copy, Clone)]
enum ValueSource {
    Immediate(u32),
    Memory(u32),
    Stack,
}

impl ValueSource {
    fn load(&self, instructions: &mut Vec<encoder::Instruction>) {
        match self {
            ValueSource::Immediate(v) => instructions.push(encoder::Instruction::Push(*v)),
            ValueSource::Memory(addr) => {
                instructions.push(encoder::Instruction::MemLoad(Some(*addr)));
            }
            ValueSource::Stack => {}
        }
    }
}

struct Memory {
    static_alloc_ptr: u32,
}

impl Memory {
    fn new() -> Self {
        Memory {
            // 0 is reserved for the null pointer
            // 1, 2 and reserved for the error string
            // 3 is reserved for the dynamic allocation pointer
            // 4, 5 are reserved for logging
            // 6 is reserved for the selfdestruct flag
            static_alloc_ptr: 7,
        }
    }

    fn allocate(&mut self, size: u32) -> u32 {
        let addr = self.static_alloc_ptr;
        self.static_alloc_ptr += size;
        addr
    }

    fn allocate_symbol(&mut self, type_: Type) -> Symbol {
        let addr = self.allocate(type_.miden_width());
        Symbol {
            type_,
            memory_addr: addr,
        }
    }

    /// write(vec![], addr, &[ValueSource::Immediate(0), ValueSource::Immediate(1)])
    /// will set addr to 0 and addr + 1 to 1
    fn write(
        &self,
        instructions: &mut Vec<encoder::Instruction>,
        start_addr: u32,
        values: &[ValueSource],
    ) {
        let mut addr = start_addr;
        for v in values {
            v.load(instructions);
            instructions.push(encoder::Instruction::MemStore(Some(addr)));
            addr += 1;
        }
    }

    /// read reads the values from the memory starting at start_addr and pushes them to the stack.
    ///
    /// The top most stack item will be the value of start_addr.
    ///
    /// The bottom most stack item will be the value of start_addr + count - 1.
    fn read(&self, instructions: &mut Vec<encoder::Instruction>, start_addr: u32, count: u32) {
        for i in 1..=count {
            ValueSource::Memory(start_addr + count - i).load(instructions);
        }
    }
}

pub(crate) struct Compiler<'ast, 'c, 's> {
    instructions: &'c mut Vec<encoder::Instruction<'ast>>,
    memory: &'c mut Memory,
    root_scope: &'c Scope<'ast, 's>,
    record_depenencies: Vec<(abi::RecordHashes, Symbol)>,
}

impl<'ast, 'c, 's> Compiler<'ast, 'c, 's> {
    fn new(
        instructions: &'c mut Vec<encoder::Instruction<'ast>>,
        memory: &'c mut Memory,
        root_scope: &'c Scope<'ast, 's>,
    ) -> Self {
        Compiler {
            instructions,
            memory,
            root_scope,
            record_depenencies: Vec::new(),
        }
    }

    fn comment(&mut self, comment: String) {
        self.instructions
            .push(encoder::Instruction::Comment(comment));
    }

    fn get_record_dependency(&mut self, col: &Contract) -> Option<Symbol> {
        self.record_depenencies
            .iter()
            .find(|(hashes, _)| hashes.contract == col.name)
            .map(|(_, symbol)| symbol.clone())
    }
}

/// Returns None if converting would result in silent truncation
fn convert_f64_to_f32(n: f64) -> Option<f32> {
    if n as f32 as f64 != n {
        None
    } else {
        Some(n as f32)
    }
}

fn compile_expression(expr: &Expression, compiler: &mut Compiler, scope: &Scope) -> Result<Symbol> {
    comment!(compiler, "Compiling expression {expr:?}");

    maybe_start!(expr.span());

    use ast::ExpressionKind;
    let symbol: Symbol = match &**expr {
        ExpressionKind::Ident(id) => scope.find_symbol(id).not_found("symbol", id)?,
        ExpressionKind::Primitive(ast::Primitive::Number(n, _has_decimal_point)) => {
            let n = convert_f64_to_f32(*n).ok_or_else(|| Error::simple("silent f64 truncation"))?;

            float32::new(compiler, n)
        }
        ExpressionKind::Primitive(ast::Primitive::String(s)) => string::new(compiler, s).0,
        ExpressionKind::Boolean(b) => boolean::new(compiler, *b),
        ExpressionKind::Add(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_add(compiler, &a, &b)?
        }
        ExpressionKind::Subtract(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_sub(compiler, &a, &b)
        }
        ExpressionKind::Modulo(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_mod(compiler, &a, &b)
        }
        ExpressionKind::Divide(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_div(compiler, &a, &b)
        }
        ExpressionKind::Multiply(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_mul(compiler, &a, &b)
        }
        ExpressionKind::Equal(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_eq(compiler, &a, &b)?
        }
        ExpressionKind::NotEqual(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_neq(compiler, &a, &b)
        }
        ExpressionKind::Not(x) => {
            let x = compile_expression(x, compiler, scope)?;
            match x.type_ {
                Type::PrimitiveType(PrimitiveType::Boolean) => {
                    compiler.memory.read(
                        compiler.instructions,
                        x.memory_addr,
                        x.type_.miden_width(),
                    );
                    compiler.instructions.push(encoder::Instruction::Not);

                    let result = compiler
                        .memory
                        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
                    compiler.memory.write(
                        compiler.instructions,
                        result.memory_addr,
                        &[ValueSource::Stack],
                    );
                    result
                }
                Type::Nullable(_) => {
                    compiler.memory.read(
                        compiler.instructions,
                        nullable::is_not_null(&x).memory_addr,
                        nullable::is_not_null(&x).type_.miden_width(),
                    );
                    compiler.instructions.push(encoder::Instruction::Not);

                    let result = compiler
                        .memory
                        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
                    compiler.memory.write(
                        compiler.instructions,
                        result.memory_addr,
                        &[ValueSource::Stack],
                    );
                    result
                }
                _ => panic!("expected boolean or nullable for NOT (!)"),
            }
        }
        ExpressionKind::Call(func, args) => {
            let is_in_hidden_builtin = scope.find_function("hiddenNoopMarker").is_some();
            let (func, args_symbols) = match &***func {
                ExpressionKind::Ident(id) if id == "u32_" && is_in_hidden_builtin => {
                    ensure!(
                        args.len() == 1,
                        ArgumentsCountSnafu {
                            found: args.len(),
                            expected: 1usize
                        }
                    );

                    match &*args[0] {
                        ExpressionKind::Primitive(ast::Primitive::Number(
                            n,
                            _has_decimal_point,
                        )) => return Ok(uint32::new(compiler, *n as u32)),
                        _ => {
                            return TypeMismatchSnafu {
                                context: "expected number at u32_",
                            }
                            .fail()
                            .map_err(Into::into)
                        }
                    }
                }
                ExpressionKind::Ident(func_name) => (
                    scope
                        .find_function(func_name)
                        .not_found("function", func_name)?,
                    args.iter()
                        .map(|arg| compile_expression(arg, compiler, scope))
                        .collect::<Result<Vec<_>>>()?,
                ),
                ExpressionKind::Dot(obj_expr, func_name) => {
                    let obj = compile_expression(obj_expr, compiler, scope)?;

                    let func = scope
                        .find_method(&obj.type_, func_name)
                        .not_found("object method", func_name)?;

                    (func, {
                        let mut args_symbols = vec![obj];
                        for arg in args {
                            args_symbols.push(compile_expression(arg, compiler, scope)?);
                        }
                        args_symbols
                    })
                }
                _ => {
                    return TypeMismatchSnafu {
                        context: "tried to call function by not ident",
                    }
                    .fail()
                    .map_err(Into::into)
                }
            };

            compile_function_call(compiler, func, &args_symbols, None)?.unwrap_or(Symbol {
                type_: Type::Nullable(Box::new(Type::PrimitiveType(PrimitiveType::Boolean))),
                memory_addr: 0,
            })
        }
        ExpressionKind::Assign(a, b) => {
            if let (ExpressionKind::Index(a, index), b) = (&***a, b) {
                let a = compile_expression(a, compiler, scope)?;
                let b = compile_expression(b, compiler, scope)?;
                let index = compile_expression(index, compiler, scope)?;

                let (_key, _value, value_ptr, did_find) = map::get(compiler, &a, &index)?;

                let mut if_found_instructions = vec![];
                {
                    std::mem::swap(compiler.instructions, &mut if_found_instructions);

                    // write b to value_ptr
                    for i in 0..b.type_.miden_width() {
                        compiler
                            .memory
                            .read(compiler.instructions, b.memory_addr + i, 1);
                        // [b[i]]
                        compiler
                            .memory
                            .read(compiler.instructions, value_ptr.memory_addr, 1);
                        // [value_ptr, b[i]]
                        compiler.instructions.push(encoder::Instruction::Push(i));
                        // [1, value_ptr, b[i]]
                        compiler
                            .instructions
                            .push(encoder::Instruction::U32CheckedAdd);
                        // [value_ptr + i, b[i]]
                        compiler
                            .instructions
                            .push(encoder::Instruction::MemStore(None));
                        // []
                    }

                    std::mem::swap(compiler.instructions, &mut if_found_instructions);
                }

                let mut if_not_found = vec![];
                {
                    std::mem::swap(compiler.instructions, &mut if_not_found);

                    let (keys, values) = map::key_values_arr(&a)?;
                    array::push(compiler, scope, &[keys, index])?;
                    array::push(compiler, scope, &[values, b.clone()])?;

                    std::mem::swap(compiler.instructions, &mut if_not_found);
                }

                compiler.instructions.extend([encoder::Instruction::If {
                    condition: vec![encoder::Instruction::MemLoad(Some(did_find.memory_addr))],
                    then: if_found_instructions,
                    else_: if_not_found,
                }]);

                return Ok(b);
            }

            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            match (&a.type_, &b.type_) {
                (Type::Struct(a_struct), Type::Struct(_b_struct)) => {
                    for (field, ty) in &a_struct.fields {
                        let a_field = struct_field(compiler, &a, field)?;
                        let b_field = struct_field(compiler, &b, field)?;

                        ensure_eq_type!(b_field, @ty);

                        compiler.memory.read(
                            compiler.instructions,
                            b_field.memory_addr,
                            ty.miden_width(),
                        );
                        compiler.memory.write(
                            compiler.instructions,
                            a_field.memory_addr,
                            &vec![ValueSource::Stack; ty.miden_width() as usize],
                        );
                    }
                }
                (Type::Nullable(a_inner_type), b_type) if !matches!(b_type, Type::Nullable(_)) => {
                    ensure_eq_type!(@a_inner_type.as_ref(), @b_type);

                    compiler.memory.write(
                        compiler.instructions,
                        nullable::is_not_null(&a).memory_addr,
                        &[ValueSource::Immediate(1)],
                    );

                    compiler.memory.read(
                        compiler.instructions,
                        b.memory_addr,
                        b_type.miden_width(),
                    );
                    compiler.memory.write(
                        compiler.instructions,
                        nullable::value(a.clone()).memory_addr,
                        &vec![ValueSource::Stack; b_type.miden_width() as usize],
                    );
                }
                (a_type, b_type) => {
                    ensure_eq_type!(@a_type, @b_type);

                    compiler.memory.read(
                        compiler.instructions,
                        b.memory_addr,
                        b_type.miden_width(),
                    );
                    compiler.memory.write(
                        compiler.instructions,
                        a.memory_addr,
                        &vec![ValueSource::Stack; b_type.miden_width() as usize],
                    );
                }
            }

            a
        }
        ExpressionKind::AssignAdd(a, b) => compile_expression(
            &Expression::T(ExpressionKind::Assign(
                a.clone(),
                Box::new(Expression::T(ExpressionKind::Add(
                    Box::new(*a.clone()),
                    b.clone(),
                ))),
            )),
            compiler,
            scope,
        )?,
        ExpressionKind::Increment(a) => {
            let a = match &***a {
                ExpressionKind::Ident(id) => scope.find_symbol(id).not_found("symbol", id)?,
                _ => {
                    return TypeMismatchSnafu {
                        context: "tried to increment non-ident",
                    }
                    .fail()
                    .map_err(Into::into)
                }
            };

            let one = match &a.type_ {
                Type::PrimitiveType(PrimitiveType::UInt32) => uint32::new(compiler, 1),
                Type::PrimitiveType(PrimitiveType::Float32) => float32::new(compiler, 1.0),
                _ => panic!("increment not supported for type {:?}", a.type_),
            };

            let incremented = compile_add(compiler, &a, &one)?;

            compiler.memory.read(
                compiler.instructions,
                incremented.memory_addr,
                incremented.type_.miden_width(),
            );
            compiler.memory.write(
                compiler.instructions,
                a.memory_addr,
                &vec![ValueSource::Stack; incremented.type_.miden_width() as usize],
            );

            incremented
        }
        ExpressionKind::Dot(a, b) => {
            let a = compile_expression(a, compiler, scope)?;

            struct_field(compiler, &a, b)?
        }
        ExpressionKind::GreaterThanOrEqual(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_gte(compiler, &a, &b)
        }
        ExpressionKind::GreaterThan(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_gt(compiler, &a, &b)
        }
        ExpressionKind::LessThanOrEqual(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_lte(compiler, &a, &b)
        }
        ExpressionKind::LessThan(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_lt(compiler, &a, &b)
        }
        ExpressionKind::ShiftLeft(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_shift_left(compiler, &a, &b)
        }
        ExpressionKind::ShiftRight(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_shift_right(compiler, &a, &b)
        }
        ExpressionKind::And(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            boolean::compile_and(compiler, &a, &b)
        }
        ExpressionKind::Or(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            boolean::compile_or(compiler, &a, &b)
        }
        ExpressionKind::Array(exprs) => {
            let all_zeros = exprs.iter().all(|expr| match &**expr {
                ExpressionKind::Primitive(ast::Primitive::Number(n, _has_decimal_point)) => {
                    *n == 0.0
                }
                _ => false,
            });
            let mut symbols = vec![];
            if !all_zeros {
                for expr in exprs {
                    symbols.push(compile_expression(expr, compiler, scope)?);
                }
            }
            let type_ = if symbols.is_empty() {
                Type::PrimitiveType(PrimitiveType::Float32)
            } else {
                symbols[0].type_.clone()
            };

            for (a, b) in symbols.iter().zip(symbols.iter().skip(1)) {
                ensure_eq_type!(@a.type_, @b.type_);
            }

            if exprs.is_empty() {
                array::new(
                    compiler,
                    0,
                    // TODO: We need to infer what the type of the array is,
                    // for example, if the user does `this.array = []` we need
                    // the type to be the same as this.array
                    type_.clone(),
                )
                .0
            } else {
                let (array, data_ptr) = array::new(compiler, exprs.len() as u32, type_);

                for (i, symbol) in symbols.iter().enumerate() {
                    compiler.memory.read(
                        compiler.instructions,
                        symbol.memory_addr,
                        symbol.type_.miden_width(),
                    );
                    compiler.memory.write(
                        compiler.instructions,
                        data_ptr + i as u32 * symbols[0].type_.miden_width(),
                        &vec![ValueSource::Stack; symbol.type_.miden_width() as usize],
                    );
                }

                array
            }
        }
        ExpressionKind::Object(obj) => {
            let mut types = Vec::new();
            let mut values = Vec::new();
            for (field, expr) in &obj.fields {
                let symbol = compile_expression(expr, compiler, scope)?;
                types.push((field.clone(), symbol.type_.clone()));
                values.push((field, symbol));
            }

            let struct_type = Type::Struct(Struct {
                name: "anonymous".to_owned(),
                fields: types,
            });

            let symbol = compiler.memory.allocate_symbol(struct_type);
            for (field, expr_symbol) in values {
                let field = struct_field(compiler, &symbol, field)?;
                compiler.memory.read(
                    compiler.instructions,
                    expr_symbol.memory_addr,
                    field.type_.miden_width(),
                );
                compiler.memory.write(
                    compiler.instructions,
                    field.memory_addr,
                    &vec![ValueSource::Stack; field.type_.miden_width() as usize],
                );
            }

            symbol
        }
        ExpressionKind::Index(a, b) => {
            let a = compile_expression(a, compiler, scope)?;
            let b = compile_expression(b, compiler, scope)?;

            compile_index(compiler, &a, &b)?
        }
        e => return Err(Error::unimplemented(format!("compile {e:?}"))),
    };

    let symbol = match &symbol.type_ {
        Type::Nullable(_)
            if scope
                .non_null_symbol_addrs
                .iter()
                .any(|addr| *addr == symbol.memory_addr) =>
        {
            nullable::value(symbol)
        }
        _ => symbol,
    };

    comment!(
        compiler,
        "Compiled expression {expr:?} to symbol {symbol:?}",
    );

    Ok(symbol)
}

fn compile_statement(
    statement: &Statement,
    compiler: &mut Compiler,
    scope: &mut Scope,
    return_result: &Option<&mut Symbol>,
) -> Result<()> {
    maybe_start!(statement.span());
    match &**statement {
        ast::StatementKind::Return(expr) => {
            let symbol = compile_expression(expr, compiler, scope)?;
            compiler.memory.read(
                compiler.instructions,
                symbol.memory_addr,
                symbol.type_.miden_width(),
            );
            compiler.memory.write(
                compiler.instructions,
                return_result
                    .as_ref()
                    .ok_or_else(|| Error::simple("return in a function with no return type"))?
                    .memory_addr,
                &vec![ValueSource::Stack; symbol.type_.miden_width() as usize],
            );
            compiler.instructions.push(encoder::Instruction::Abstract(
                encoder::AbstractInstruction::Return,
            ));
        }
        ast::StatementKind::Break => {
            compiler.instructions.push(encoder::Instruction::Abstract(
                encoder::AbstractInstruction::Break,
            ));
        }
        ast::StatementKind::If(ast::If {
            condition,
            then_statements,
            else_statements,
        }) => {
            let mut scope = scope.deeper();
            let mut condition_instructions = vec![];
            let mut condition_compiler = Compiler::new(
                &mut condition_instructions,
                compiler.memory,
                compiler.root_scope,
            );
            let condition_symbol = compile_expression(condition, &mut condition_compiler, &scope)?;
            // let mut then_cleanup = None;
            let mut then_scope = scope.deeper();
            let condition_symbol = match condition_symbol.type_ {
                Type::PrimitiveType(PrimitiveType::Boolean) => condition_symbol,
                Type::Nullable(ref _t) => {
                    then_scope
                        .non_null_symbol_addrs
                        .push(condition_symbol.memory_addr);

                    nullable::is_not_null(&condition_symbol)
                }
                _ => panic!(
                    "if condition must be a boolean or optional, got {:?}",
                    condition_symbol.type_
                ),
            };
            condition_compiler.memory.read(
                condition_compiler.instructions,
                condition_symbol.memory_addr,
                condition_symbol.type_.miden_width(),
            );

            let mut body_instructions = vec![];
            let mut body_compiler =
                Compiler::new(&mut body_instructions, compiler.memory, compiler.root_scope);
            for statement in then_statements {
                compile_statement(
                    statement,
                    &mut body_compiler,
                    &mut then_scope,
                    return_result,
                )?;
            }
            // then_cleanup.map(|f| f());

            let mut else_body_instructions = vec![];
            let mut else_body_compiler = Compiler::new(
                &mut else_body_instructions,
                compiler.memory,
                compiler.root_scope,
            );
            for statement in else_statements {
                compile_statement(
                    statement,
                    &mut else_body_compiler,
                    &mut scope,
                    return_result,
                )?;
            }

            compiler.instructions.push(encoder::Instruction::If {
                condition: condition_instructions,
                then: body_instructions,
                else_: else_body_instructions,
            })
        }
        ast::StatementKind::While(ast::While {
            condition,
            statements,
        }) => {
            let mut scope = scope.deeper();
            let mut condition_instructions = vec![];
            let mut condition_compiler = Compiler::new(
                &mut condition_instructions,
                compiler.memory,
                compiler.root_scope,
            );
            let condition_symbol = compile_expression(condition, &mut condition_compiler, &scope)?;
            ensure_eq_type!(
                condition_symbol,
                Type::PrimitiveType(PrimitiveType::Boolean)
            );
            condition_compiler.memory.read(
                condition_compiler.instructions,
                condition_symbol.memory_addr,
                condition_symbol.type_.miden_width(),
            );

            let mut body_instructions = vec![];
            let mut body_compiler =
                Compiler::new(&mut body_instructions, compiler.memory, compiler.root_scope);
            for statement in statements {
                compile_statement(statement, &mut body_compiler, &mut scope, return_result)?;
            }

            compiler.instructions.push(encoder::Instruction::While {
                condition: condition_instructions,
                body: body_instructions,
            })
        }
        ast::StatementKind::For(ast::For {
            for_kind,
            statements,
        }) => {
            // There is no `for` instruction, we have to use `while` instead
            let mut scope = scope.deeper();

            let mut initial_instructions = vec![];
            let mut condition_instructions = vec![];
            let mut pre_instructions = vec![];
            let mut post_instructions = vec![];
            match for_kind {
                ast::ForKind::Basic {
                    initial_statement,
                    condition,
                    post_statement,
                } => {
                    let mut initial_compiler = Compiler::new(
                        &mut initial_instructions,
                        compiler.memory,
                        compiler.root_scope,
                    );
                    match initial_statement {
                        ast::ForInitialStatement::Let(l) => {
                            compile_let_statement(l, &mut initial_compiler, &mut scope)
                        }
                        ast::ForInitialStatement::Expression(e) => {
                            compile_expression(e, &mut initial_compiler, &scope).map(|_| ())
                        }
                    }?;

                    let mut condition_compiler = Compiler::new(
                        &mut condition_instructions,
                        compiler.memory,
                        compiler.root_scope,
                    );
                    let condition_symbol =
                        compile_expression(condition, &mut condition_compiler, &scope)?;
                    ensure_eq_type!(
                        condition_symbol,
                        Type::PrimitiveType(PrimitiveType::Boolean)
                    );
                    condition_compiler.memory.read(
                        condition_compiler.instructions,
                        condition_symbol.memory_addr,
                        condition_symbol.type_.miden_width(),
                    );

                    let mut post_compiler =
                        Compiler::new(&mut post_instructions, compiler.memory, compiler.root_scope);
                    compile_expression(post_statement, &mut post_compiler, &scope)?;
                }
                ast::ForKind::ForEach {
                    for_each_type,
                    identifier,
                    iterable,
                } => {
                    let mut initial_compiler = Compiler::new(
                        &mut initial_instructions,
                        compiler.memory,
                        compiler.root_scope,
                    );
                    let foreach_index_identifier = "#internal_foreach_index";
                    let foreach_index_symbol = uint32::new(&mut initial_compiler, 0);
                    scope.add_symbol(
                        foreach_index_identifier.to_string(),
                        foreach_index_symbol.clone(),
                    );

                    let mut condition_compiler = Compiler::new(
                        &mut condition_instructions,
                        compiler.memory,
                        compiler.root_scope,
                    );
                    let iterable_symbol =
                        compile_expression(iterable, &mut condition_compiler, &scope)?;
                    let foreach_len_symbol = match &iterable_symbol.type_ {
                        Type::Array(_) => array::length(&iterable_symbol),
                        Type::Map(_, _) => array::length(&map::keys_arr(&iterable_symbol)?),
                        ty => {
                            return Err(Error::unimplemented(format!(
                                "cannot iterate for-{for_each_type} with type {:?}",
                                ty
                            )));
                        }
                    };

                    let foreach_len_identifier = "#internal_foreach_len";
                    scope.add_symbol(foreach_len_identifier.to_string(), foreach_len_symbol);
                    let condition_symbol = compile_expression(
                        &ast::ExpressionKind::LessThan(
                            Box::new(
                                ast::ExpressionKind::Ident(foreach_index_identifier.to_string())
                                    .into(),
                            ),
                            Box::new(
                                ast::ExpressionKind::Ident(foreach_len_identifier.to_string())
                                    .into(),
                            ),
                        )
                        .into(),
                        &mut condition_compiler,
                        &scope,
                    )?;
                    condition_compiler.memory.read(
                        condition_compiler.instructions,
                        condition_symbol.memory_addr,
                        condition_symbol.type_.miden_width(),
                    );

                    let mut pre_compiler =
                        Compiler::new(&mut pre_instructions, compiler.memory, compiler.root_scope);
                    match (for_each_type, &iterable_symbol.type_) {
                        (ast::ForEachType::In, Type::Array(_)) => {
                            scope.add_symbol(identifier.clone(), foreach_index_symbol.clone());
                        }
                        (ast::ForEachType::In, Type::Map(_, _)) => {
                            // XXX: Optimize?
                            let keys = map::keys_arr(&iterable_symbol)?;
                            let key = array::get(&mut pre_compiler, &keys, &foreach_index_symbol);
                            scope.add_symbol(identifier.clone(), key);
                        }
                        (ast::ForEachType::Of, Type::Array(_)) => {
                            let symbol = array::get(
                                &mut pre_compiler,
                                &iterable_symbol,
                                &foreach_index_symbol,
                            );
                            scope.add_symbol(identifier.clone(), symbol);
                        }
                        (ast::ForEachType::Of, Type::Map(_, _)) => {
                            // XXX: Optimize?
                            let values = map::values_arr(&iterable_symbol)?;
                            let value =
                                array::get(&mut pre_compiler, &values, &foreach_index_symbol);
                            scope.add_symbol(identifier.clone(), value);
                        }
                        ty => {
                            return Err(Error::unimplemented(format!(
                                "cannot iterate for-{for_each_type} with type {:?}",
                                ty
                            )));
                        }
                    }

                    let post_compiler =
                        Compiler::new(&mut post_instructions, compiler.memory, compiler.root_scope);
                    post_compiler.instructions.extend([
                        encoder::Instruction::MemLoad(Some(foreach_index_symbol.memory_addr)),
                        encoder::Instruction::Push(1),
                        encoder::Instruction::U32CheckedAdd,
                        encoder::Instruction::MemStore(Some(foreach_index_symbol.memory_addr)),
                    ]);
                }
            }

            let body = {
                let mut body_instructions = pre_instructions;
                let mut body_compiler =
                    Compiler::new(&mut body_instructions, compiler.memory, compiler.root_scope);
                let mut body_scope = scope.deeper();
                for statement in statements {
                    compile_statement(
                        statement,
                        &mut body_compiler,
                        &mut body_scope,
                        return_result,
                    )?;
                }
                body_instructions.extend(post_instructions);
                body_instructions
            };

            compiler.instructions.extend(initial_instructions);
            compiler.instructions.push(encoder::Instruction::While {
                condition: condition_instructions,
                body,
            });
        }
        ast::StatementKind::Let(let_statement) => {
            compile_let_statement(let_statement, compiler, scope)?
        }
        ast::StatementKind::Expression(expr) => {
            compile_expression(expr, compiler, scope)?;
        }
        ast::StatementKind::Throw(expr) => {
            compile_expression(expr, compiler, scope)?;
        }
    }

    Ok(())
}

fn add_new_symbol(expr: &Expression, compiler: &mut Compiler, scope: &Scope) -> Result<Symbol> {
    let symbol = compile_expression(expr, compiler, scope)?;
    // we need to copy symbol to a new symbol,
    // because Ident expressions return symbols of variables
    let new_symbol = compiler.memory.allocate_symbol(symbol.type_);
    compiler.memory.read(
        compiler.instructions,
        symbol.memory_addr,
        new_symbol.type_.miden_width(),
    );
    compiler.memory.write(
        compiler.instructions,
        new_symbol.memory_addr,
        &vec![ValueSource::Stack; new_symbol.type_.miden_width() as usize],
    );

    Ok(new_symbol)
}

fn compile_let_statement(
    let_statement: &ast::Let,
    compiler: &mut Compiler,
    scope: &mut Scope,
) -> Result<()> {
    let new_symbol = match &*let_statement.expression {
        ast::ExpressionKind::Primitive(ast::Primitive::Number(n, has_decimal)) => {
            match &let_statement.type_ {
                Some(ast::Type::U32) => {
                    ensure!(
                        !*has_decimal,
                        TypeMismatchSnafu {
                            context: "expected integer, not float"
                        }
                    );

                    uint32::new(compiler, *n as u32)
                }
                Some(_) => {
                    return Err(Error::unimplemented(format!(
                        "let statement with type {:?}",
                        let_statement.type_
                    )));
                }
                None => add_new_symbol(&let_statement.expression, compiler, scope)?,
            }
        }
        _ => add_new_symbol(&let_statement.expression, compiler, scope)?,
    };

    scope.add_symbol(let_statement.identifier.to_string(), new_symbol);
    Ok(())
}

fn compile_ast_function_call(
    function: &ast::Function,
    compiler: &mut Compiler,
    args: &[Symbol],
    this: Option<Symbol>,
) -> Result<Option<Symbol>> {
    let mut function_instructions = vec![];
    let mut function_compiler = Compiler::new(
        &mut function_instructions,
        compiler.memory,
        compiler.root_scope,
    );

    let scope = &mut Scope::new();
    scope.parent = Some(compiler.root_scope);

    if let Some(this) = this {
        scope.add_symbol("this".to_string(), this);
    }

    let mut return_result = function.return_type.as_ref().map(|ty| {
        function_compiler
            .memory
            .allocate_symbol(ast_type_to_type(true, ty))
    });
    for (arg, param) in args.iter().zip(function.parameters.iter()) {
        // We need to make a copy of the arg, because Ident expressions return symbols of variables.
        // Modifying them in a function would modify the original variable.
        // TODO: fix this
        let new_arg = function_compiler.memory.allocate_symbol(arg.type_.clone());
        function_compiler.memory.read(
            function_compiler.instructions,
            arg.memory_addr,
            arg.type_.miden_width(),
        );
        function_compiler.memory.write(
            function_compiler.instructions,
            new_arg.memory_addr,
            &vec![ValueSource::Stack; new_arg.type_.miden_width() as usize],
        );

        scope.add_symbol(param.name.clone(), new_arg);
    }

    for statement in &function.statements {
        compile_statement(
            statement,
            &mut function_compiler,
            scope,
            &return_result.as_mut(),
        )?;
    }

    compiler.instructions.push(encoder::Instruction::Abstract(
        encoder::AbstractInstruction::InlinedFunction(function_instructions),
    ));

    Ok(return_result)
}

fn compile_function_call(
    compiler: &mut Compiler,
    function: &Function,
    args: &[Symbol],
    this: Option<Symbol>,
) -> Result<Option<Symbol>> {
    match function {
        Function::Ast(a) => compile_ast_function_call(a, compiler, args, this),
        Function::Builtin(b) => b(compiler, &mut Scope::new(), args).map(Some),
    }
}

fn cast(compiler: &mut Compiler, from: &Symbol, to: &Symbol) {
    match (&from.type_, &to.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::cast_from_uint32(compiler, from, to),
        x => unimplemented!("{:?}", x),
    }
}

fn compile_add(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Result<Symbol> {
    Ok(match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::add(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::add(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::add(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::add(compiler, a, &b_u64)
        }
        (
            Type::PrimitiveType(PrimitiveType::Float32),
            Type::PrimitiveType(PrimitiveType::Float32),
        ) => float32::add(compiler, a, b),
        (Type::String, Type::String) => string::concat(compiler, a, b)?,
        (a, b) => return Err(Error::unimplemented(format!("{a:?} add {b:?}"))),
    })
}

fn compile_sub(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::sub(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::sub(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::sub(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::sub(compiler, a, &b_u64)
        }
        (
            Type::PrimitiveType(PrimitiveType::Float32),
            Type::PrimitiveType(PrimitiveType::Float32),
        ) => float32::sub(compiler, a, b),
        e => unimplemented!("{:?}", e),
    }
}

fn compile_mod(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::modulo(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::modulo(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::modulo(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::modulo(compiler, a, &b_u64)
        }
        e => unimplemented!("{:?}", e),
    }
}

fn compile_div(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::div(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::div(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::div(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::div(compiler, a, &b_u64)
        }
        (
            Type::PrimitiveType(PrimitiveType::Float32),
            Type::PrimitiveType(PrimitiveType::Float32),
        ) => float32::div(compiler, a, b),
        e => unimplemented!("{:?}", e),
    }
}

fn compile_mul(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::mul(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::mul(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::mul(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::mul(compiler, a, &b_u64)
        }
        (
            Type::PrimitiveType(PrimitiveType::Float32),
            Type::PrimitiveType(PrimitiveType::Float32),
        ) => float32::mul(compiler, a, b),
        e => unimplemented!("{:?}", e),
    }
}

fn compile_eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Result<Symbol> {
    Ok(match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::eq(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::eq(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::eq(compiler, a, &b_u64)
        }
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            uint32::eq(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::Float32),
            Type::PrimitiveType(PrimitiveType::Float32),
        ) => float32::eq(compiler, a, b),
        (Type::Hash, Type::Hash) => {
            let result = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

            compiler
                .instructions
                .push(encoder::Instruction::Push(true as _));
            for i in 0..a.type_.miden_width() {
                compiler
                    .memory
                    .read(compiler.instructions, a.memory_addr + i, 1);
                compiler
                    .memory
                    .read(compiler.instructions, b.memory_addr + i, 1);
                compiler.instructions.push(encoder::Instruction::Eq);
                compiler.instructions.push(encoder::Instruction::And);
            }
            compiler.memory.write(
                compiler.instructions,
                result.memory_addr,
                &[ValueSource::Stack],
            );
            result
        }
        (Type::PublicKey, Type::PublicKey) => publickey::eq(compiler, a, b),
        (Type::String, Type::String) => string::eq(compiler, a, b),
        (Type::Nullable(lt), Type::Nullable(rt)) if lt == rt => nullable::eq(compiler, a, b),
        (Type::Nullable(type_from_nullable), not_null_type)
        | (not_null_type, Type::Nullable(type_from_nullable))
            if &**type_from_nullable == not_null_type =>
        {
            // a is the nullable type, b is the not null type
            let (a, b) = if a.type_ == Type::Nullable(type_from_nullable.clone()) {
                (a, b)
            } else {
                (b, a)
            };

            let mut eq_instructions = vec![];
            std::mem::swap(compiler.instructions, &mut eq_instructions);
            let eq_result = compile_eq(compiler, &nullable::value(a.clone()), &b);
            std::mem::swap(compiler.instructions, &mut eq_instructions);

            compiler.instructions.push(encoder::Instruction::If {
                condition: vec![encoder::Instruction::MemLoad(Some(
                    nullable::is_not_null(&a).memory_addr,
                ))],
                then: eq_instructions,
                else_: vec![],
            });

            eq_result?
        }
        e => return Err(Error::unimplemented(format!("eq {:?} {:?}", e.0, e.1))),
    })
}

fn compile_neq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    if a.type_ == Type::PrimitiveType(PrimitiveType::Float32)
        && b.type_ == Type::PrimitiveType(PrimitiveType::Float32)
    {
        return float32::ne(compiler, a, b);
    }

    let eq = compile_eq(compiler, a, b).unwrap();
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
    compiler.memory.read(
        compiler.instructions,
        eq.memory_addr,
        eq.type_.miden_width(),
    );
    compiler.instructions.push(encoder::Instruction::Not);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &vec![ValueSource::Stack; result.type_.miden_width() as _],
    );
    result
}

fn compile_gte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::gte(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::gte(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::gte(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::gte(compiler, a, &b_u64)
        }
        (
            Type::PrimitiveType(PrimitiveType::Float32),
            Type::PrimitiveType(PrimitiveType::Float32),
        ) => float32::gte(compiler, a, b),
        e => unimplemented!("{:?}", e),
    }
}

fn compile_gt(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::gt(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::gt(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::gt(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::gt(compiler, a, &b_u64)
        }
        (
            Type::PrimitiveType(PrimitiveType::Float32),
            Type::PrimitiveType(PrimitiveType::Float32),
        ) => float32::gt(compiler, a, b),
        e => unimplemented!("{:?}", e),
    }
}

fn compile_lte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::lte(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::lte(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::lte(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::lte(compiler, a, &b_u64)
        }
        (
            Type::PrimitiveType(PrimitiveType::Float32),
            Type::PrimitiveType(PrimitiveType::Float32),
        ) => float32::lte(compiler, a, b),
        e => unimplemented!("{:?}", e),
    }
}

fn compile_lt(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::lt(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::lt(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::lt(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::lt(compiler, a, &b_u64)
        }
        (
            Type::PrimitiveType(PrimitiveType::Float32),
            Type::PrimitiveType(PrimitiveType::Float32),
        ) => float32::lt(compiler, a, b),
        e => unimplemented!("{:?}", e),
    }
}

fn compile_shift_left(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::shift_left(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::shift_left(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::shift_left(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::shift_left(compiler, a, &b_u64)
        }
        e => unimplemented!("{:?}", e),
    }
}

fn compile_shift_right(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::shift_right(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::shift_right(compiler, a, b),
        (Type::PrimitiveType(PrimitiveType::Int32), Type::PrimitiveType(PrimitiveType::Int32)) => {
            int32::shift_right(compiler, a, b)
        }
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::shift_right(compiler, a, &b_u64)
        }
        e => unimplemented!("{:?}", e),
    }
}

fn compile_index(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Result<Symbol> {
    match &a.type_ {
        Type::Map(k, _v) => {
            ensure_eq_type!(@k.as_ref(), @&b.type_);

            let (_key, value, _value_ptr, _found) = map::get(compiler, a, b)?;
            Ok(value)
        }
        Type::Array(_) => {
            ensure_eq_type!(
                b,
                Type::PrimitiveType(PrimitiveType::UInt32)
                    // TODO: ideally we should parse it as generic `Number` and instantiate into a real time lately.
                    // so e.g here no need to do a reinterpret back as integer.
                    | Type::PrimitiveType(PrimitiveType::Float32)
            );

            Ok(array::get(compiler, a, b))
        }
        x => TypeMismatchSnafu {
            context: format!("cannot index {x:?}"),
        }
        .fail()
        .map_err(Into::into),
    }
}

fn dynamic_alloc(compiler: &mut Compiler, args: &[Symbol]) -> Result<Symbol> {
    let size = &args[0];
    ensure!(
        matches!(size.type_, Type::PrimitiveType(PrimitiveType::UInt32)),
        TypeMismatchSnafu {
            context: "cannot alloc of size other than UInt32"
        }
    );

    let addr = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    compiler
        .instructions
        .push(encoder::Instruction::MemLoad(Some(3)));
    compiler.instructions.push(encoder::Instruction::Dup(None));
    compiler.memory.write(
        compiler.instructions,
        addr.memory_addr,
        &[ValueSource::Stack],
    );
    compiler.memory.read(
        compiler.instructions,
        size.memory_addr,
        size.type_.miden_width(),
    );
    // old addr + size
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);

    // store new addr
    compiler
        .instructions
        .push(encoder::Instruction::MemStore(Some(3)));

    // return old addr
    Ok(addr)
}

fn log(compiler: &mut Compiler, scope: &mut Scope, args: &[Symbol]) -> Result<Symbol> {
    let mut str_args = vec![];

    for arg in args {
        let message = match &arg.type_ {
            Type::String => arg.clone(),
            Type::PrimitiveType(PrimitiveType::UInt32) => compile_function_call(
                compiler,
                scope.find_function("uint32ToString").unwrap(),
                &[arg.clone()],
                None,
            )?
            .unwrap(),
            Type::PrimitiveType(PrimitiveType::Boolean) => compile_function_call(
                compiler,
                scope.find_function("uint32ToString").unwrap(),
                &[Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::UInt32),
                    ..arg.clone()
                }],
                None,
            )?
            .unwrap(),
            t => {
                return Err(Error::unimplemented(format!(
                    "logging of {t:?} is not supported yet"
                )))
            }
        };

        str_args.push(message);
    }

    for arg in str_args {
        compile_function_call(compiler, &Function::Ast(&LOG_STRING), &[arg], None)?;
    }

    Ok(Symbol {
        type_: Type::PrimitiveType(PrimitiveType::Boolean),
        memory_addr: 0,
    })
}

fn read_advice_contract_reference(compiler: &mut Compiler, contract: String) -> Result<Symbol> {
    let r = compile_function_call(
        compiler,
        BUILTINS_SCOPE
            .find_function("readAdviceContractReference")
            .unwrap(),
        &[],
        None,
    )?
    .unwrap();

    Ok(Symbol {
        type_: Type::ContractReference { contract },
        ..r
    })
}

fn read_advice_public_key(compiler: &mut Compiler) -> Result<Symbol> {
    let result = compiler.memory.allocate_symbol(Type::PublicKey);

    compiler.instructions.push(encoder::Instruction::AdvPush(1));
    compiler.memory.write(
        compiler.instructions,
        publickey::kty(&result).memory_addr,
        &[ValueSource::Stack],
    );

    compiler.instructions.push(encoder::Instruction::AdvPush(1));
    compiler.memory.write(
        compiler.instructions,
        publickey::crv(&result).memory_addr,
        &[ValueSource::Stack],
    );

    compiler.instructions.push(encoder::Instruction::AdvPush(1));
    compiler.memory.write(
        compiler.instructions,
        publickey::alg(&result).memory_addr,
        &[ValueSource::Stack],
    );

    compiler.instructions.push(encoder::Instruction::AdvPush(1));
    compiler.memory.write(
        compiler.instructions,
        publickey::use_(&result).memory_addr,
        &[ValueSource::Stack],
    );

    let n64 = uint32::new(compiler, 64);
    let extra_ptr = dynamic_alloc(compiler, &[n64])?;

    compiler
        .memory
        .read(compiler.instructions, extra_ptr.memory_addr, 1);
    // [extra_ptr]

    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [extra_ptr, extra_ptr]

    compiler.memory.write(
        compiler.instructions,
        publickey::extra_ptr(&result).memory_addr,
        &[ValueSource::Stack],
    );
    // [extra_ptr]

    for _ in 0..64 {
        compiler.instructions.push(encoder::Instruction::AdvPush(1));
        // [byte, extra_ptr]
        compiler
            .instructions
            .push(encoder::Instruction::Dup(Some(1)));
        // [extra_ptr, byte, extra_ptr]
        compiler
            .instructions
            .push(encoder::Instruction::MemStore(None));
        // [extra_ptr]
        compiler.instructions.push(encoder::Instruction::Push(1));
        // [1, extra_ptr]
        compiler
            .instructions
            .push(encoder::Instruction::U32CheckedAdd);
        // [extra_ptr + 1]
    }

    compiler.instructions.push(encoder::Instruction::Drop);
    // []

    Ok(result)
}

fn read_advice_string(compiler: &mut Compiler) -> Result<Symbol> {
    let result = compiler.memory.allocate_symbol(Type::String);

    compiler.instructions.push(encoder::Instruction::AdvPush(1));
    // [str_len]

    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [str_len, str_len]
    let str_len = string::length(&result);
    compiler.memory.write(
        compiler.instructions,
        str_len.memory_addr,
        &[ValueSource::Stack],
    );
    // [str_len]

    let data_ptr = dynamic_alloc(compiler, &[str_len])?;
    compiler.memory.write(
        compiler.instructions,
        string::data_ptr(&result).memory_addr,
        &[ValueSource::Memory(data_ptr.memory_addr)],
    );
    let data_ptr = string::data_ptr(&result);

    compiler.memory.read(
        compiler.instructions,
        data_ptr.memory_addr,
        data_ptr.type_.miden_width(),
    );
    // [data_ptr, str_len]

    compiler.instructions.extend_from_slice(&[
        encoder::Instruction::Swap,
        // [str_len, data_ptr]
        encoder::Instruction::While {
            condition: vec![
                encoder::Instruction::Dup(None),
                // [str_len, str_len, data_ptr]
                encoder::Instruction::Push(0),
                // [0, str_len, str_len, data_ptr]
                encoder::Instruction::U32CheckedGT,
                // [str_len > 0, str_len, data_ptr]
            ],
            body: vec![
                // [str_len, data_ptr]
                encoder::Instruction::Push(1),
                // [1, str_len, data_ptr]
                encoder::Instruction::U32CheckedSub,
                // [str_len - 1, data_ptr]
                encoder::Instruction::Swap,
                // [data_ptr, str_len - 1]
                encoder::Instruction::AdvPush(1),
                // [byte, data_ptr, str_len - 1]
                encoder::Instruction::Dup(Some(1)),
                // [data_ptr, byte, data_ptr, str_len - 1]
                encoder::Instruction::MemStore(None),
                // [data_ptr, str_len - 1]
                encoder::Instruction::Push(1),
                // [1, data_ptr, str_len - 1]
                encoder::Instruction::U32CheckedAdd,
                // [data_ptr + 1, str_len - 1]
                encoder::Instruction::Swap,
                // [str_len - 1, data_ptr + 1]
            ],
        },
        // [0, data_ptr]
        encoder::Instruction::Drop,
        // [data_ptr]
        encoder::Instruction::Drop,
        // []
    ]);

    Ok(result)
}

fn read_advice_array(compiler: &mut Compiler, element_type: &Type) -> Result<Symbol> {
    compiler.instructions.push(encoder::Instruction::AdvPush(1));
    // [array_len]

    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [array_len, array_len]
    let array_len = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler.memory.write(
        compiler.instructions,
        array_len.memory_addr,
        &[ValueSource::Stack],
    );
    // [array_len]

    let capacity = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler.instructions.push(encoder::Instruction::Push(2));
    // [2, array_len]
    // capacity is 2x the length, because reallocating is expensive
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedMul);
    // [capacity = array_len * 2]
    compiler.memory.write(
        compiler.instructions,
        capacity.memory_addr,
        &[ValueSource::Stack],
    );
    // []

    let data_ptr = dynamic_alloc(compiler, &[capacity])?;

    let read_element_advice_insts = {
        let mut insts = vec![];
        std::mem::swap(compiler.instructions, &mut insts);

        let el = read_advice_generic(compiler, element_type)?;
        compiler.memory.read(
            compiler.instructions,
            el.memory_addr,
            element_type.miden_width(),
        );

        std::mem::swap(compiler.instructions, &mut insts);
        insts
    };

    compiler
        .memory
        .read(compiler.instructions, data_ptr.memory_addr, 1);
    // [data_ptr]
    compiler
        .memory
        .read(compiler.instructions, array_len.memory_addr, 1);
    // [array_len, data_ptr]
    compiler.instructions.push(encoder::Instruction::While {
        condition: vec![
            // [array_len, data_ptr]
            encoder::Instruction::Dup(None),
            // [array_len, array_len, data_ptr]
            encoder::Instruction::Push(0),
            // [0, array_len, array_len, data_ptr]
            encoder::Instruction::U32CheckedGT,
            // [array_len > 0, array_len, data_ptr]
        ],
        body: [
            // [array_len, data_ptr]
            encoder::Instruction::Push(1),
            // [1, array_len, data_ptr]
            encoder::Instruction::U32CheckedSub,
            // [array_len - 1, data_ptr]
            encoder::Instruction::Swap,
            // [data_ptr, array_len - 1]
        ]
        .into_iter()
        .chain(read_element_advice_insts)
        .chain({
            // [bytes... (width), data_ptr, array_len - 1]
            let mut v = vec![];

            for i in 0..element_type.miden_width() {
                v.push(encoder::Instruction::Dup(Some(
                    element_type.miden_width() - i,
                )));
                // [data_ptr, bytes..., data_ptr, array_len - 1]
                v.push(encoder::Instruction::Push(i));
                // [i, data_ptr, bytes..., data_ptr, array_len - 1]
                v.push(encoder::Instruction::U32CheckedAdd);
                // [data_ptr + i, bytes..., data_ptr, array_len - 1]
                v.push(encoder::Instruction::MemStore(None));
                // [bytes..., data_ptr, array_len - 1]
            }

            v.into_iter()
        })
        .chain([
            // [data_ptr, array_len - 1]
            encoder::Instruction::Push(element_type.miden_width()),
            // [width, data_ptr, array_len - 1]
            encoder::Instruction::U32CheckedAdd,
            // [data_ptr + width, array_len - 1]
            encoder::Instruction::Swap,
            // [array_len - 1, data_ptr + width]
        ])
        .collect(),
    });

    // [0, end_data_ptr]
    compiler.instructions.push(encoder::Instruction::Drop);
    compiler.instructions.push(encoder::Instruction::Drop);
    // []

    let arr = compiler
        .memory
        .allocate_symbol(Type::Array(Box::new(element_type.clone())));

    compiler.memory.write(
        compiler.instructions,
        array::length(&arr).memory_addr,
        &[ValueSource::Memory(array_len.memory_addr)],
    );

    compiler
        .memory
        .read(compiler.instructions, array::length(&arr).memory_addr, 1);
    // [array_len]
    compiler.instructions.push(encoder::Instruction::Push(2));
    // [2, array_len]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedMul);
    // [capacity = array_len * 2]
    compiler.memory.write(
        compiler.instructions,
        array::capacity(&arr).memory_addr,
        &[ValueSource::Stack],
    );
    // []

    compiler.memory.write(
        compiler.instructions,
        array::data_ptr(&arr).memory_addr,
        &[ValueSource::Memory(data_ptr.memory_addr)],
    );

    Ok(arr)
}

fn read_advice_map(compiler: &mut Compiler, key_type: &Type, value_type: &Type) -> Result<Symbol> {
    // Maps are serialized as [keys_arr..., values_arr...]
    let result = compiler.memory.allocate_symbol(Type::Map(
        Box::new(key_type.clone()),
        Box::new(value_type.clone()),
    ));

    let key_array = read_advice_array(compiler, key_type)?;
    let value_array = read_advice_array(compiler, value_type)?;

    let (keys, values) = map::key_values_arr(&result)?;
    compiler.memory.write(
        compiler.instructions,
        keys.memory_addr,
        &[
            ValueSource::Memory(array::capacity(&key_array).memory_addr),
            ValueSource::Memory(array::length(&key_array).memory_addr),
            ValueSource::Memory(array::data_ptr(&key_array).memory_addr),
        ],
    );

    compiler.memory.write(
        compiler.instructions,
        values.memory_addr,
        &[
            ValueSource::Memory(array::capacity(&value_array).memory_addr),
            ValueSource::Memory(array::length(&value_array).memory_addr),
            ValueSource::Memory(array::data_ptr(&value_array).memory_addr),
        ],
    );

    Ok(result)
}

fn read_advice_nullable(compiler: &mut Compiler, type_: Type) -> Result<Symbol> {
    let value_type = match &type_ {
        Type::Nullable(value_type) => value_type,
        _ => {
            return TypeMismatchSnafu {
                context: format!("read_advice_nullable for non-null type: {type_:?}"),
            }
            .fail()
            .map_err(Into::into)
        }
    };

    let is_not_null = compile_function_call(
        compiler,
        BUILTINS_SCOPE.find_function("readAdviceBoolean").unwrap(),
        &[],
        None,
    )?
    .unwrap();

    let (value, read_value_insts) = {
        let mut insts = vec![];
        std::mem::swap(compiler.instructions, &mut insts);

        let value = read_advice_generic(compiler, value_type)?;
        std::mem::swap(compiler.instructions, &mut insts);

        (value, insts)
    };

    compiler.instructions.push(encoder::Instruction::If {
        condition: vec![encoder::Instruction::MemLoad(Some(is_not_null.memory_addr))],
        then: read_value_insts,
        else_: vec![],
    });

    let s = compiler.memory.allocate_symbol(type_);
    compiler.memory.read(
        compiler.instructions,
        is_not_null.memory_addr,
        is_not_null.type_.miden_width(),
    );
    compiler.memory.write(
        compiler.instructions,
        nullable::is_not_null(&s).memory_addr,
        &vec![ValueSource::Stack; is_not_null.type_.miden_width() as _],
    );
    compiler.memory.read(
        compiler.instructions,
        value.memory_addr,
        value.type_.miden_width(),
    );
    compiler.memory.write(
        compiler.instructions,
        nullable::value(s.clone()).memory_addr,
        &vec![ValueSource::Stack; value.type_.miden_width() as _],
    );

    Ok(s)
}

/// A generic hash function that can hash any symbol by hashing each of it's field elements.
/// Not useful for hashing strings, or any data structure that uses pointers.
fn generic_hash(compiler: &mut Compiler, value: &Symbol) -> Symbol {
    let result = compiler.memory.allocate_symbol(Type::Hash);

    compiler.instructions.extend([
        encoder::Instruction::Push(0),
        encoder::Instruction::Push(0),
        encoder::Instruction::Push(0),
        encoder::Instruction::Push(0),
    ]);
    // [h[3], h[2], h[1], h[0]]
    for i in 0..value.type_.miden_width() {
        compiler
            .memory
            .read(compiler.instructions, value.memory_addr + i, 1);
        compiler.instructions.extend([
            encoder::Instruction::Push(0),
            encoder::Instruction::Push(0),
            encoder::Instruction::Push(0),
        ]);
        // [0, 0, 0, data, h[3], h[2], h[1], h[0]]
        compiler.instructions.push(encoder::Instruction::HMerge);
        // [h[3], h[2], h[1], h[0]]
    }

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[
            ValueSource::Stack,
            ValueSource::Stack,
            ValueSource::Stack,
            ValueSource::Stack,
        ],
    );

    result
}

fn hash(compiler: &mut Compiler, value: Symbol) -> Result<Symbol> {
    let result = match &value.type_ {
        Type::Nullable(_) => {
            let h = compiler.memory.allocate_symbol(Type::Hash);

            let mut hash_value_instructions = vec![];
            std::mem::swap(compiler.instructions, &mut hash_value_instructions);
            let non_null_value_hash = hash(compiler, nullable::value(value.clone()))?;
            std::mem::swap(compiler.instructions, &mut hash_value_instructions);

            compiler.instructions.extend([encoder::Instruction::If {
                condition: vec![encoder::Instruction::MemLoad(Some(
                    nullable::is_not_null(&value).memory_addr,
                ))],
                then: hash_value_instructions
                    .into_iter()
                    .chain({
                        let mut instructions = vec![];
                        compiler.memory.read(
                            &mut instructions,
                            non_null_value_hash.memory_addr,
                            non_null_value_hash.type_.miden_width(),
                        );
                        compiler.memory.write(
                            &mut instructions,
                            h.memory_addr,
                            &vec![
                                ValueSource::Stack;
                                non_null_value_hash.type_.miden_width() as usize
                            ],
                        );
                        instructions
                    })
                    .collect(),
                // leave h at 0 if value is null
                else_: vec![],
            }]);

            h
        }
        Type::PrimitiveType(_) => generic_hash(compiler, &value),
        Type::Hash => generic_hash(compiler, &value),
        Type::Hash8 => generic_hash(compiler, &value),
        Type::String => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashString").unwrap(),
            &[value],
            None,
        )?
        .unwrap(),
        Type::Bytes => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashBytes").unwrap(),
            &[value],
            None,
        )?
        .unwrap(),
        Type::ContractReference { .. } => compile_function_call(
            compiler,
            BUILTINS_SCOPE
                .find_function("hashContractReference")
                .unwrap(),
            &[value],
            None,
        )?
        .unwrap(),
        Type::Array(_) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashArray").unwrap(),
            &[value],
            None,
        )?
        .unwrap(),
        Type::Map(_, _) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashMap").unwrap(),
            &[value],
            None,
        )?
        .unwrap(),
        Type::PublicKey => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashPublicKey").unwrap(),
            &[value],
            None,
        )?
        .unwrap(),
        Type::Struct(s) => {
            let mut offset = 0;
            let struct_hash = compiler.memory.allocate_symbol(Type::Hash);
            for (_, field_type) in &s.fields {
                let width = field_type.miden_width();
                let field = Symbol {
                    type_: field_type.clone(),
                    memory_addr: value.memory_addr + offset,
                };
                offset += width;

                let field_hash = hash(compiler, field)?;

                compiler.memory.read(
                    compiler.instructions,
                    struct_hash.memory_addr,
                    struct_hash.type_.miden_width(),
                );
                compiler.memory.read(
                    compiler.instructions,
                    field_hash.memory_addr,
                    field_hash.type_.miden_width(),
                );

                compiler.instructions.push(encoder::Instruction::HMerge);

                compiler.memory.write(
                    compiler.instructions,
                    struct_hash.memory_addr,
                    &[
                        ValueSource::Stack,
                        ValueSource::Stack,
                        ValueSource::Stack,
                        ValueSource::Stack,
                    ],
                );
            }

            struct_hash
        }
    };

    ensure_eq_type!(result, Type::Hash);

    Ok(result)
}

fn add_salt_to_hash(compiler: &mut Compiler, hash: &Symbol, salt: &Symbol) -> Result<Symbol> {
    ensure_eq_type!(hash, Type::Hash);
    ensure_eq_type!(salt, Type::PrimitiveType(PrimitiveType::UInt32));

    let result = compiler.memory.allocate_symbol(Type::Hash);

    compiler.memory.read(
        compiler.instructions,
        hash.memory_addr,
        hash.type_.miden_width(),
    );
    compiler.memory.read(
        compiler.instructions,
        salt.memory_addr,
        salt.type_.miden_width(),
    );
    for _ in 0..3 {
        compiler.instructions.push(encoder::Instruction::Push(0));
    }
    compiler.instructions.push(encoder::Instruction::HMerge);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[
            ValueSource::Stack,
            ValueSource::Stack,
            ValueSource::Stack,
            ValueSource::Stack,
        ],
    );

    Ok(result)
}

fn hash_record_with_salts(
    compiler: &mut Compiler,
    struct_symbol: &Symbol,
    field_salts: &[Symbol],
) -> Result<Symbol> {
    ensure_eq_type!(struct_symbol, Type::Struct(_));
    let Type::Struct(struct_) = &struct_symbol.type_ else {
        unreachable!()
    };

    let result = compiler.memory.allocate_symbol(Type::Hash);
    for (i, (field_name, _)) in struct_.fields.iter().enumerate() {
        let salt = &field_salts[i];
        let field_symbol = struct_field(compiler, struct_symbol, field_name)?;

        let field_hash = hash(
            compiler,
            Symbol {
                type_: field_symbol.type_.clone(),
                memory_addr: field_symbol.memory_addr,
            },
        )?;
        let field_hash = add_salt_to_hash(compiler, &field_hash, salt)?;

        compiler.memory.read(
            compiler.instructions,
            result.memory_addr,
            result.type_.miden_width(),
        );
        compiler.memory.read(
            compiler.instructions,
            field_hash.memory_addr,
            field_hash.type_.miden_width(),
        );
        compiler.instructions.push(encoder::Instruction::HMerge);
        compiler.memory.write(
            compiler.instructions,
            result.memory_addr,
            &[
                ValueSource::Stack,
                ValueSource::Stack,
                ValueSource::Stack,
                ValueSource::Stack,
            ],
        );
    }

    Ok(result)
}

fn read_advice_generic(compiler: &mut Compiler, type_: &Type) -> Result<Symbol> {
    Ok(match type_ {
        Type::Nullable(_) => read_advice_nullable(compiler, type_.clone())?,
        Type::PrimitiveType(PrimitiveType::Boolean) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceBoolean").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::PrimitiveType(PrimitiveType::UInt32) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceUInt32").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::PrimitiveType(PrimitiveType::UInt64) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceUInt64").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::PrimitiveType(PrimitiveType::Int32) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceInt32").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::PrimitiveType(PrimitiveType::Int64) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceInt64").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::PrimitiveType(PrimitiveType::Float32) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceFloat32").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::PrimitiveType(PrimitiveType::Float64) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceFloat64").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::String => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceString").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::Bytes => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceBytes").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::ContractReference { contract } => {
            read_advice_contract_reference(compiler, contract.clone())?
        }
        Type::Array(t) => read_advice_array(compiler, t)?,
        Type::Struct(s) => {
            let symbol = compiler.memory.allocate_symbol(type_.clone());
            read_struct_from_advice_tape(compiler, &symbol, s, None)?;
            symbol
        }
        Type::PublicKey => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdvicePublicKey").unwrap(),
            &[],
            None,
        )?
        .unwrap(),
        Type::Map(k, v) => read_advice_map(compiler, k, v)?,
        _ => {
            return Err(Error::unimplemented(format!(
                "read_advice_generic {type_:?}"
            )))
        }
    })
}

/// `lazy` is an array of boolean `Symbol`s, as many as there as struct fields.
/// If `lazy[i]` is true, then the field will be read, if false, it will be skipped.
fn read_struct_from_advice_tape(
    compiler: &mut Compiler,
    struct_symbol: &Symbol,
    struct_type: &Struct,
    lazy: Option<&[Symbol]>,
) -> Result<()> {
    for (i, (name, type_)) in struct_type.fields.iter().enumerate() {
        let (_field, field_insts) = {
            let mut insts = vec![];
            std::mem::swap(compiler.instructions, &mut insts);

            let value = read_advice_generic(compiler, type_)?;
            let sf = struct_field(compiler, struct_symbol, name)?;
            compiler.memory.read(
                compiler.instructions,
                value.memory_addr,
                value.type_.miden_width(),
            );
            compiler.memory.write(
                compiler.instructions,
                sf.memory_addr,
                &vec![ValueSource::Stack; value.type_.miden_width() as _],
            );

            std::mem::swap(compiler.instructions, &mut insts);
            (sf, insts)
        };

        match lazy {
            Some(lazy) => {
                compiler.instructions.push(encoder::Instruction::If {
                    condition: vec![encoder::Instruction::MemLoad(Some(lazy[i].memory_addr))],
                    then: [
                        encoder::Instruction::Push(struct_symbol.memory_addr + i as u32),
                        encoder::Instruction::Push(0),
                        encoder::Instruction::Push(0),
                        encoder::Instruction::Push(1),
                        encoder::Instruction::AdvPushMapval,
                    ]
                    .into_iter()
                    .chain(field_insts)
                    .collect(),
                    else_: vec![],
                });
            }
            None => compiler.instructions.extend(field_insts),
        }
    }

    Ok(())
}

/// Returns (Option<(salts, this)>, args)
fn read_contract_inputs(
    compiler: &mut Compiler,
    this_struct: Option<Struct>,
    args: &[Type],
    lazy: Option<&[Symbol]>,
) -> Result<(Option<(Vec<Symbol>, Symbol)>, Vec<Symbol>)> {
    let this = this_struct.map(|ts| compiler.memory.allocate_symbol(Type::Struct(ts)));
    let mut salts = vec![];

    if let Some(this) = this.as_ref() {
        let struct_ty = match &this.type_ {
            Type::Struct(s) => s,
            _ => unreachable!(),
        };

        salts = struct_ty
            .fields
            .iter()
            .map(|_| read_advice_generic(compiler, &Type::PrimitiveType(PrimitiveType::UInt32)))
            .collect::<Result<Vec<_>>>()?;

        read_struct_from_advice_tape(compiler, this, struct_ty, lazy)?;
    }

    let mut args_symbols = Vec::new();
    for arg in args {
        args_symbols.push(read_advice_generic(compiler, arg)?);
    }

    Ok((this.map(|t| (salts, t)), args_symbols))
}

fn prepare_scope(program: &ast::Program) -> Scope {
    let mut scope = Scope::new();

    for (name, type_, func) in USABLE_BUILTINS.iter() {
        match type_ {
            Some(type_) => scope.add_method(type_.clone(), name.clone(), func.clone()),
            None => scope.add_function(name.clone(), func.clone()),
        }
    }

    for node in &program.nodes {
        match node {
            ast::RootNode::Contract(c) => {
                let mut contract = Contract {
                    name: c.name.clone(),
                    functions: vec![],
                    fields: vec![],
                    call_directive: match c.decorators.iter().find(|d| {
                        d.name == "call"
                            || d.name == "public"
                            || d.name == "read"
                            || d.name == "private"
                    }) {
                        Some(d) if d.arguments.len() > 0 => {
                            panic!(
                                "Invalid {name} directive, {name}() takes no arguments",
                                name = &d.name
                            )
                        }
                        Some(d) if d.name == "private" => false,
                        Some(_) => true,
                        // collections are public by default
                        None => true,
                    },
                    read_directive: match c
                        .decorators
                        .iter()
                        .find(|d| d.name == "read" || d.name == "public" || d.name == "private")
                    {
                        Some(d) if d.arguments.len() > 0 => {
                            panic!(
                                "Invalid {name} directive, {name}() takes no arguments",
                                name = &d.name
                            )
                        }
                        Some(d) if d.name == "private" => false,
                        Some(_) => true,
                        // collections are public by default
                        None => true,
                    },
                };

                for item in &c.items {
                    match item {
                        ast::ContractItem::Field(f) => {
                            contract.fields.push(ContractField {
                                name: f.name.clone(),
                                type_: ast_type_to_type(f.required, &f.type_),
                                delegate: f.decorators.iter().any(|d| d.name == "delegate"),
                                read: f.decorators.iter().any(|d| d.name == "read"),
                            });
                        }
                        ast::ContractItem::Function(f) => {
                            contract.functions.push((f.name.clone(), f));
                        }
                        ast::ContractItem::Index(_) => {}
                    }
                }

                scope.add_contract(contract.name.clone(), contract);
            }
            ast::RootNode::Function(function) => scope
                .functions
                .push((function.name.clone(), Function::Ast(function))),
        }
    }

    scope
}

pub fn compile(
    program: ast::Program,
    contract_name: Option<&str>,
    function_name: &str,
) -> Result<(String, Abi)> {
    let mut scope = prepare_scope(&program);
    let contract = contract_name.map(|name| scope.find_contract(name).cloned().unwrap());
    let contract = contract.as_ref();
    let contract_struct = contract.map(|c| Struct::from(c.clone()));

    let (function, param_types) = match function_name {
        ".readAuth" => (None, vec![]),
        _ => {
            let function = contract
                .and_then(|c| {
                    c.functions
                        .iter()
                        .find(|(name, _)| name == function_name)
                        .map(|(_, f)| *f)
                })
                .or_else(|| match scope.find_function(function_name) {
                    Some(Function::Ast(f)) => Some(f),
                    Some(Function::Builtin(_)) => todo!(),
                    None => None,
                })
                .not_found("function", function_name)?;

            let param_types = function
                .parameters
                .iter()
                .map(|p| ast_param_type_to_type(p.required, &p.type_, contract_struct.as_ref()))
                .collect::<Result<Vec<_>>>()?;

            (Some(function), param_types)
        }
    };

    let mut instructions = vec![];
    let mut memory = Memory::new();
    let this_addr;
    let result;
    // A vector of hashmaps for each field, mapping the address of one of the field elements to the count of times it was used
    let mut used_fields_count: Vec<HashMap<u32, usize>>;
    let mut dependent_fields = Vec::<(String, Type)>::new();
    // hashing will generate read instructions
    const USED_FIELD_COUNT_THRESHOLD: usize = 2;

    let ctx_struct = Struct {
        name: "Context".to_string(),
        fields: vec![(
            "publicKey".to_owned(),
            Type::Nullable(Box::new(Type::PublicKey)),
        )],
    };
    let ctx = memory.allocate_symbol(Type::Struct(ctx_struct.clone()));

    scope.add_symbol("ctx".to_string(), ctx.clone());

    let all_possible_record_dependencies = scope
        .contracts
        .iter()
        .map(|c| {
            (
                abi::RecordHashes {
                    contract: c.0.clone(),
                },
                memory.allocate_symbol(Type::Array(Box::new(Type::Hash))),
            )
        })
        .collect::<Vec<_>>();

    {
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        compiler.record_depenencies = all_possible_record_dependencies.clone();

        let fields_in_use = contract_struct
            .as_ref()
            .iter()
            .flat_map(|s| &s.fields)
            .map(|_| {
                let enabled = compiler
                    .memory
                    .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

                enabled
            })
            .collect::<Vec<_>>();

        let expected_hashes = contract_struct
            .as_ref()
            .iter()
            .flat_map(|s| &s.fields)
            .enumerate()
            .map(|(i, _)| {
                let hash = compiler.memory.allocate_symbol(Type::Hash);
                compiler.instructions.extend([encoder::Instruction::If {
                    condition: vec![encoder::Instruction::MemLoad(Some(
                        fields_in_use[i].memory_addr,
                    ))],
                    then: vec![
                        encoder::Instruction::MemStore(Some(hash.memory_addr)),
                        encoder::Instruction::MemStore(Some(hash.memory_addr + 1)),
                        encoder::Instruction::MemStore(Some(hash.memory_addr + 2)),
                        encoder::Instruction::MemStore(Some(hash.memory_addr + 3)),
                    ],
                    else_: vec![],
                }]);
                hash
            })
            .collect::<Vec<_>>();

        for (_, symbol) in &all_possible_record_dependencies {
            let array_length = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
            let full_width = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.instructions.extend([
                // array_len is provided by the host on the stack
                // [array_len]
                encoder::Instruction::Dup(None),
                // [array_len, array_len]
                encoder::Instruction::MemStore(Some(array_length.memory_addr)),
                // [array_len]
                encoder::Instruction::Push(4), // miden width of hash
                // [4, array_len]
                encoder::Instruction::U32CheckedMul,
                // [full_width = array_len * 4]
                encoder::Instruction::Dup(None),
                // [full_width, full_width]
                encoder::Instruction::MemStore(Some(full_width.memory_addr)),
                // [full_width]
            ]);

            let ptr = dynamic_alloc(&mut compiler, &[full_width.clone()])?;

            compiler.instructions.extend([
                encoder::Instruction::While {
                    condition: vec![
                        // [full_width]
                        encoder::Instruction::Dup(None),
                        // [full_width, full_width]
                        encoder::Instruction::Push(0),
                        // [0, full_width, full_width]
                        encoder::Instruction::U32CheckedGT,
                        // [full_width > 0, full_width]
                    ],
                    body: vec![
                        // [full_width]
                        encoder::Instruction::Dup(None),
                        // [full_width, full_width]
                        encoder::Instruction::MemLoad(Some(full_width.memory_addr)),
                        // [original_full_width, full_width, full_width]
                        encoder::Instruction::Swap,
                        encoder::Instruction::U32CheckedSub,
                        // [offset = original_full_width - full_width, full_width]
                        encoder::Instruction::MemLoad(Some(ptr.memory_addr)),
                        // [ptr, offset, full_width]
                        encoder::Instruction::U32CheckedAdd,
                        // [target = ptr + offset, full_width]
                        encoder::Instruction::MovUp(2),
                        // [value, target, full_width]
                        encoder::Instruction::Swap,
                        // [target, value, full_width]
                        encoder::Instruction::MemStore(None),
                        // [full_width]
                        encoder::Instruction::Push(1),
                        // [1, full_width]
                        encoder::Instruction::U32CheckedSub,
                        // [full_width - 1]
                    ],
                },
                encoder::Instruction::Drop,
                // []
            ]);

            compiler.memory.write(
                compiler.instructions,
                array::length(symbol).memory_addr,
                &[ValueSource::Memory(array_length.memory_addr)],
            );

            compiler.memory.write(
                compiler.instructions,
                array::capacity(symbol).memory_addr,
                &[ValueSource::Memory(array_length.memory_addr)],
            );

            compiler.memory.write(
                compiler.instructions,
                array::data_ptr(symbol).memory_addr,
                &[ValueSource::Memory(ptr.memory_addr)],
            );
        }

        read_struct_from_advice_tape(&mut compiler, &ctx, &ctx_struct, None)?;

        let (salts_this_symbol, arg_symbols) = read_contract_inputs(
            &mut compiler,
            contract_struct.clone(),
            &param_types,
            Some(&fields_in_use),
        )?;

        let ctx_pk = struct_field(&mut compiler, &ctx, "publicKey")?;
        if salts_this_symbol.is_some() && function.is_some() {
            let auth_result = compile_call_authorization_proof(
                &mut compiler,
                &ctx_pk,
                &salts_this_symbol.as_ref().unwrap().1,
                contract_name.unwrap(),
                function_name,
            )?;

            let assert_fn = compiler.root_scope.find_function("assert").unwrap();
            let (error_str, _) = string::new(
                &mut compiler,
                "You are not authorized to call this function",
            );
            compile_function_call(&mut compiler, assert_fn, &[auth_result, error_str], None)?;
        }

        this_addr = salts_this_symbol.as_ref().map(|(_, ts)| ts.memory_addr);

        if let Some((salts, this_symbol)) = &salts_this_symbol {
            for (i, field) in contract_struct.as_ref().unwrap().fields.iter().enumerate() {
                let field_used_instructions = {
                    let mut insts = vec![];
                    std::mem::swap(compiler.instructions, &mut insts);

                    let field_symbol = struct_field(&mut compiler, this_symbol, &field.0)?;
                    let expected_field_hash = expected_hashes[i].clone();
                    let actual_field_hash = hash(&mut compiler, field_symbol.clone())?;
                    let actual_field_hash =
                        add_salt_to_hash(&mut compiler, &actual_field_hash, &salts[i])?;

                    let is_eq =
                        compile_eq(&mut compiler, &expected_field_hash, &actual_field_hash)?;
                    let assert_fn = compiler.root_scope.find_function("assert").unwrap();
                    let (error_str, _) = string::new(
                        &mut compiler,
                        &format!("Hash of field {} does not match the expected hash", field.0),
                    );
                    compile_function_call(&mut compiler, assert_fn, &[is_eq, error_str], None)?;

                    std::mem::swap(compiler.instructions, &mut insts);
                    insts
                };
                compiler.instructions.extend([encoder::Instruction::If {
                    condition: vec![encoder::Instruction::MemLoad(Some(
                        fields_in_use[i].memory_addr,
                    ))],
                    then: field_used_instructions,
                    else_: vec![],
                }]);
            }
        }

        result = match function {
            // read auth
            None => {
                let ctx_pk = struct_field(&mut compiler, &ctx, "publicKey")?;

                let read_auth = compile_read_authorization_proof(
                    &mut compiler,
                    &salts_this_symbol.as_ref().unwrap().1,
                    contract.as_ref().unwrap(),
                    &ctx_pk,
                )?;

                compiler.memory.read(
                    compiler.instructions,
                    read_auth.memory_addr,
                    read_auth.type_.miden_width(),
                );

                None
            }
            Some(function) => compile_ast_function_call(
                function,
                &mut compiler,
                &arg_symbols,
                salts_this_symbol.as_ref().map(|(_, ts)| ts).cloned(),
            )?,
        };

        if let Some(result) = &result {
            let result_hash = hash(&mut compiler, result.clone())?;
            compiler.memory.read(
                compiler.instructions,
                result_hash.memory_addr,
                result_hash.type_.miden_width(),
            );
        }

        if let Some((salts, this_symbol)) = &salts_this_symbol {
            for (i, (field_name, _)) in contract_struct.as_ref().unwrap().fields.iter().enumerate()
            {
                let if_in_use_then_insts = {
                    let mut insts = vec![];
                    std::mem::swap(compiler.instructions, &mut insts);

                    let field_symbol = struct_field(&mut compiler, &this_symbol, &field_name)?;
                    let field_hash = hash(&mut compiler, field_symbol.clone())?;
                    let field_hash = add_salt_to_hash(&mut compiler, &field_hash, &salts[i])?;
                    comment!(compiler, "Reading output field `{}` hash", field_name);
                    compiler.memory.read(
                        &mut compiler.instructions,
                        field_hash.memory_addr,
                        field_hash.type_.miden_width(),
                    );

                    std::mem::swap(compiler.instructions, &mut insts);
                    insts
                };

                compiler.instructions.push(encoder::Instruction::If {
                    condition: vec![encoder::Instruction::MemLoad(Some(
                        fields_in_use[i].memory_addr,
                    ))],
                    then: if_in_use_then_insts,
                    else_: vec![],
                });
            }
        }

        comment!(compiler, "Reading selfdestruct flag");
        compiler.memory.read(compiler.instructions, 6, 1);

        assert_eq!(
            compiler.record_depenencies.len(),
            all_possible_record_dependencies.len()
        );

        used_fields_count = vec![HashMap::new(); fields_in_use.len()];
        let field_addr_ranges = contract_struct
            .as_ref()
            .map(|s| {
                s.fields
                    .iter()
                    .map(|(field_name, _)| {
                        let symbol = struct_field(
                            &mut compiler,
                            &salts_this_symbol.as_ref().unwrap().1,
                            &field_name,
                        )?;

                        let start = symbol.memory_addr;
                        let end = symbol.memory_addr + symbol.type_.miden_width();
                        Ok(start..end)
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        let struct_addr_range = salts_this_symbol.map(|(_, this_symbol)| {
            let start = this_symbol.memory_addr;
            let end = this_symbol.memory_addr + this_symbol.type_.miden_width();
            start..end
        });
        if let Some(struct_addr_range) = struct_addr_range.as_ref() {
            encoder::walk(&compiler.instructions, &mut |inst| {
                match inst {
                    encoder::Instruction::MemLoad(Some(addr)) => {
                        // First, check if the address is in the struct
                        if !struct_addr_range.contains(addr) {
                            return;
                        }

                        for (i, field_addr_range) in field_addr_ranges.iter().enumerate() {
                            if field_addr_range.contains(addr) {
                                *used_fields_count[i].entry(*addr).or_default() += 1;
                                return;
                            }
                        }
                    }
                    _ => {}
                }
            });
        }

        for (i, used) in used_fields_count.iter().enumerate() {
            let mut insts = vec![];
            std::mem::swap(compiler.instructions, &mut insts);

            let max_used = used
                .iter()
                .max_by_key(|(_, count)| *count)
                .map(|(_, count)| *count)
                .unwrap_or(0);

            if max_used > USED_FIELD_COUNT_THRESHOLD {
                let field_in_use = &fields_in_use[i];
                compiler.memory.write(
                    compiler.instructions,
                    field_in_use.memory_addr,
                    &[ValueSource::Immediate(1)],
                );

                dependent_fields.push(
                    contract_struct
                        .as_ref()
                        .unwrap()
                        .fields
                        .get(i)
                        .unwrap()
                        .clone(),
                );
            }

            std::mem::swap(compiler.instructions, &mut insts);
            insts.extend(compiler.instructions.drain(..));
            std::mem::swap(compiler.instructions, &mut insts);
        }

        assert_eq!(
            compiler.record_depenencies.len(),
            all_possible_record_dependencies.len()
        );
    }

    let instructions = encoder::unabstract(
        instructions,
        &mut |size| memory.allocate(size),
        &mut None,
        &mut None,
        &mut false,
        false,
    );

    let abi = Abi {
        dependent_fields,
        this_addr,
        this_type: contract_struct.map(Type::Struct),
        result_addr: result.as_ref().map(|r| r.memory_addr),
        result_type: result.map(|r| r.type_),
        param_types,
        other_contract_types: scope
            .contracts
            .iter()
            .map(|c| Type::Struct(Struct::from(c.1.clone())))
            .collect(),
        other_records: all_possible_record_dependencies
            .into_iter()
            .map(|x| x.0)
            .collect(),
        std_version: Some(StdVersion::V0_6_1),
    };

    let mut uses_sha256 = false;
    let mut uses_blake3 = false;
    encoder::walk(&instructions, &mut |inst| match inst {
        encoder::Instruction::Exec(name) if name.starts_with("sha256::") => {
            uses_sha256 = true;
        }
        encoder::Instruction::Exec(name) if name.starts_with("blake3::") => {
            uses_blake3 = true;
        }
        _ => {}
    });

    let mut miden_code = String::new();
    miden_code.push_str(format!("# ABI: {}\n", serde_json::to_string(&abi).unwrap()).as_str());
    miden_code.push_str("use.std::math::u64\n");
    if uses_sha256 {
        miden_code.push_str("use.std::crypto::hashes::sha256\n");
    }
    if uses_blake3 {
        miden_code.push_str("use.std::crypto::hashes::blake3\n");
    }
    miden_code.push_str("begin\n");
    miden_code.push_str("  push.");
    miden_code.push_str(&memory.static_alloc_ptr.to_string());
    miden_code.push_str("\n  mem_store.3\n"); // dynamic allocation pointer
    for instruction in instructions {
        instruction
            .encode(unsafe { miden_code.as_mut_vec() }, 1)
            .context(IoSnafu)?;
        miden_code.push('\n');
    }
    miden_code.push_str("end\n");

    Ok((miden_code, abi))
}

fn compile_read_authorization_proof(
    compiler: &mut Compiler,
    struct_symbol: &Symbol,
    contract: &Contract,
    auth_pk: &Symbol,
) -> Result<Symbol> {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    if contract.read_directive {
        compiler.instructions.push(encoder::Instruction::Push(1));
        compiler
            .instructions
            .push(encoder::Instruction::MemStore(Some(result.memory_addr)));
        return Ok(result);
    }

    for field in contract.fields.iter().filter(|f| f.read) {
        let field_symbol = struct_field(compiler, &struct_symbol, &field.name)?;
        compiler.memory.read(
            &mut compiler.instructions,
            field_symbol.memory_addr,
            field_symbol.type_.miden_width(),
        );

        let passed = compile_check_eq_or_ownership(compiler, field_symbol, auth_pk)?;
        compiler.instructions.push(encoder::Instruction::If {
            condition: vec![encoder::Instruction::MemLoad(Some(passed.memory_addr))],
            then: vec![
                encoder::Instruction::Push(1),
                encoder::Instruction::MemStore(Some(result.memory_addr)),
            ],
            else_: vec![],
        });
    }

    Ok(result)
}

fn compile_call_authorization_proof(
    compiler: &mut Compiler,
    // Symbol of type Type::Nullable(Type::PublicKey)
    auth_pk: &Symbol,
    contract_symbol: &Symbol,
    contract_name: &str,
    function_name: &str,
) -> Result<Symbol> {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    if function_name == "constructor" {
        compiler.instructions.push(encoder::Instruction::Push(1));
        compiler
            .instructions
            .push(encoder::Instruction::MemStore(Some(result.memory_addr)));
        return Ok(result);
    }

    let scope = compiler.root_scope;
    let Some(contract) = scope.find_contract(contract_name) else {
        return Err(Error::simple(format!(
            "Contract not found: {}",
            contract_name
        )));
    };
    // let contract_struct = Struct::from(contract.clone());
    let Some((_, function)) = contract
        .functions
        .iter()
        .find(|(name, _)| name == function_name)
    else {
        panic!("Function not found");
    };

    let mut call_decorators = function
        .decorators
        .iter()
        .filter(|d| d.name == "call")
        .peekable();

    let function_has_call_directive = call_decorators.peek().is_some();
    match (contract.call_directive, function_has_call_directive) {
        // Function call directive overrides the contract call directive.
        (_, true) => {}
        // The contract has a @call directive, but the function does not,
        // anyone can call it.
        (true, false) => {
            compiler.instructions.push(encoder::Instruction::Push(1));
            compiler
                .instructions
                .push(encoder::Instruction::MemStore(Some(result.memory_addr)));
            return Ok(result);
        }
        // Neither the contract nor the function have a @call directive,
        // no calls are allowed.
        (false, false) => return Ok(result),
    }

    let mut call_args = call_decorators
        .flat_map(|d| d.arguments.iter().map(|a| (d.span(), a)))
        .peekable();

    if function_has_call_directive && call_args.peek().is_none() {
        // The call is just `@call` with no fields, so no authorization required.
        compiler.instructions.push(encoder::Instruction::Push(1));
        compiler
            .instructions
            .push(encoder::Instruction::MemStore(Some(result.memory_addr)));
        return Ok(result);
    }

    for (decorator_span, call_arg) in call_args {
        maybe_start!(decorator_span);

        let arg_value = match call_arg {
            ast::DecoratorArgument::Identifier(id) => {
                let mut current_field = contract_symbol.clone();
                for field in &[id] {
                    current_field = struct_field(compiler, &current_field, field)?;
                }

                current_field
            }
            ast::DecoratorArgument::Literal(l) => match l {
                ast::Literal::Eth(pk) => {
                    let key = abi::publickey::Key::from_secp256k1_bytes(&pk).wrap_err()?;
                    publickey::new(compiler, key)
                }
            },
        };

        let passed = compile_check_eq_or_ownership(compiler, arg_value, auth_pk)?;
        compiler.instructions.push(encoder::Instruction::If {
            condition: vec![encoder::Instruction::MemLoad(Some(passed.memory_addr))],
            then: vec![
                encoder::Instruction::Push(1),
                encoder::Instruction::MemStore(Some(result.memory_addr)),
            ],
            else_: vec![],
        });
    }

    Ok(result)
}

fn compile_check_eq_or_ownership(
    compiler: &mut Compiler,
    field: Symbol,
    auth_pk: &Symbol,
) -> Result<Symbol> {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    let is_eq = match &field.type_ {
        Type::PublicKey => compile_eq(compiler, &field, auth_pk)?,
        Type::Nullable(t) if **t == Type::PublicKey => compile_eq(compiler, &field, auth_pk)?,
        Type::ContractReference { contract } => {
            let contract_type = compiler.root_scope.find_contract(&contract).unwrap();
            let contract_record_hashes = compiler.get_record_dependency(contract_type).unwrap();
            let id = struct_field(compiler, &field, "id").unwrap();

            let hash_id = hash(compiler, id.clone())?;
            compiler.memory.read(
                compiler.instructions,
                hash_id.memory_addr,
                hash_id.type_.miden_width(),
            );
            // [...id_hash]
            compiler
                .instructions
                .push(encoder::Instruction::AdvPushMapval);
            // advice = [Nullable(public_record_hash_position), ...record_data]
            compiler.instructions.push(encoder::Instruction::Dropw);
            // []

            let public_hash_position = read_advice_generic(
                compiler,
                &Type::Nullable(Box::new(Type::PrimitiveType(PrimitiveType::UInt32))),
            )?;

            let (not_null_instructions, result) = {
                let mut insts = vec![];
                std::mem::swap(compiler.instructions, &mut insts);

                let public_hash_position = nullable::value(public_hash_position.clone());

                let record_public_hash =
                    array::get(compiler, &contract_record_hashes, &public_hash_position);

                let record = compiler
                    .memory
                    .allocate_symbol(Type::Struct(Struct::from(contract_type.clone())));
                compiler.instructions.push(encoder::Instruction::AdvPush(
                    contract_type.fields.len() as u32,
                ));
                let salts = contract_type
                    .fields
                    .iter()
                    .map(|_| {
                        let salt = compiler
                            .memory
                            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
                        compiler.memory.write(
                            compiler.instructions,
                            salt.memory_addr,
                            &[ValueSource::Stack],
                        );
                        salt
                    })
                    .collect::<Vec<_>>();
                read_struct_from_advice_tape(
                    compiler,
                    &record,
                    &Struct::from(contract_type.clone()),
                    None,
                )?;
                let actual_record_hash = hash_record_with_salts(compiler, &record, &salts)?;
                compiler.memory.read(
                    compiler.instructions,
                    actual_record_hash.memory_addr,
                    actual_record_hash.type_.miden_width(),
                );

                let is_hash_eq = compile_eq(compiler, &record_public_hash, &actual_record_hash)?;
                let assert = compiler.root_scope.find_function("assert").unwrap();
                let (error_str, _) =
                    string::new(compiler, "Record hash does not match the expected hash");
                compile_function_call(compiler, assert, &[is_hash_eq, error_str], None)?;

                let record_id = struct_field(compiler, &record, "id")?;
                let is_id_eq = compile_eq(compiler, &record_id, &id)?;
                let (error_str, _) = string::new(compiler, "Record id does not match");
                compile_function_call(compiler, assert, &[is_id_eq, error_str], None)?;

                let result = compile_check_ownership(compiler, &record, contract_type, auth_pk)?;

                std::mem::swap(compiler.instructions, &mut insts);
                (insts, result)
            };

            compiler.instructions.push(encoder::Instruction::If {
                condition: vec![encoder::Instruction::MemLoad(Some(
                    nullable::is_not_null(&public_hash_position).memory_addr,
                ))],
                then: not_null_instructions,
                else_: vec![],
            });

            result
        }
        Type::Array(_) => {
            // We need to iterate over the array and check if any of the elements match
            let index = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            let (current_array_element, current_array_element_insts) = {
                let mut insts = vec![];
                std::mem::swap(compiler.instructions, &mut insts);

                let result = array::get(compiler, &field, &index);

                std::mem::swap(compiler.instructions, &mut insts);
                (result, insts)
            };

            let (passed, ownership_check_insts) = {
                let mut insts = vec![];
                std::mem::swap(compiler.instructions, &mut insts);

                let result = compile_check_eq_or_ownership(
                    compiler,
                    current_array_element.clone(),
                    auth_pk,
                )?;

                std::mem::swap(compiler.instructions, &mut insts);
                (result, insts)
            };

            compiler.instructions.extend([
                encoder::Instruction::MemLoad(Some(array::length(&field).memory_addr)),
                // [array_len]
                encoder::Instruction::While {
                    condition: vec![
                        encoder::Instruction::Dup(None),
                        // [array_len, array_len]
                        encoder::Instruction::Push(0),
                        encoder::Instruction::U32CheckedGT,
                        // [array_len > 0, array_len]
                        encoder::Instruction::MemLoad(Some(passed.memory_addr)),
                        // [passed, array_len > 0, array_len]
                        encoder::Instruction::Not,
                        // [!passed, array_len > 0, array_len]
                        encoder::Instruction::And,
                        // [array_len > 0 && !passed, array_len]
                    ],
                    body: [
                        // [array_len]
                        encoder::Instruction::Push(1),
                        // [1, array_len]
                        encoder::Instruction::U32CheckedSub,
                        // [array_len - 1]
                        encoder::Instruction::Dup(None),
                        encoder::Instruction::MemStore(Some(index.memory_addr)),
                        // [array_len - 1]
                    ]
                    .into_iter()
                    .chain(current_array_element_insts)
                    .chain(ownership_check_insts)
                    .collect(),
                },
            ]);

            passed
        }
        _ => todo!(),
    };

    compiler.instructions.push(encoder::Instruction::If {
        condition: vec![encoder::Instruction::MemLoad(Some(is_eq.memory_addr))],
        then: vec![
            encoder::Instruction::Push(1),
            encoder::Instruction::MemStore(Some(result.memory_addr)),
        ],
        else_: vec![],
    });

    Ok(result)
}

fn compile_check_ownership(
    compiler: &mut Compiler,
    struct_symbol: &Symbol,
    contract: &Contract,
    auth_pk: &Symbol,
) -> Result<Symbol> {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    for delegate_field in contract.fields.iter().filter(|f| f.delegate) {
        let delegate_symbol = struct_field(compiler, struct_symbol, &delegate_field.name)?;
        let is_eq = compile_check_eq_or_ownership(compiler, delegate_symbol, auth_pk)?;

        compiler.instructions.push(encoder::Instruction::If {
            condition: vec![encoder::Instruction::MemLoad(Some(is_eq.memory_addr))],
            then: vec![
                encoder::Instruction::Push(1),
                encoder::Instruction::MemStore(Some(result.memory_addr)),
            ],
            else_: vec![],
        });
    }

    Ok(result)
}

/// contract_struct is the type used for `record` types
fn ast_param_type_to_type(
    required: bool,
    type_: &ast::ParameterType,
    contract_struct: Option<&Struct>,
) -> Result<Type> {
    let t = match type_ {
        ast::ParameterType::String => Type::String,
        ast::ParameterType::Number => Type::PrimitiveType(PrimitiveType::Float32),
        ast::ParameterType::F32 => Type::PrimitiveType(PrimitiveType::Float32),
        ast::ParameterType::F64 => Type::PrimitiveType(PrimitiveType::Float64),
        ast::ParameterType::U32 => Type::PrimitiveType(PrimitiveType::UInt32),
        ast::ParameterType::U64 => Type::PrimitiveType(PrimitiveType::UInt64),
        ast::ParameterType::I32 => Type::PrimitiveType(PrimitiveType::Int32),
        ast::ParameterType::I64 => Type::PrimitiveType(PrimitiveType::Int64),
        ast::ParameterType::Record => Type::Struct(contract_struct.unwrap().clone()),
        ast::ParameterType::PublicKey => Type::PublicKey,
        ast::ParameterType::Bytes => Type::Bytes,
        ast::ParameterType::ForeignRecord { contract } => Type::ContractReference {
            contract: contract.clone(),
        },
        ast::ParameterType::Array(t) => Type::Array(Box::new(ast_type_to_type(true, t))),
        ast::ParameterType::Boolean => {
            return Err(Error::unimplemented(
                "ast_param_type_to_type for Boolean".into(),
            ))
        }
        ast::ParameterType::Map(k, v) => Type::Map(
            Box::new(ast_type_to_type(true, k)),
            Box::new(ast_type_to_type(true, v)),
        ),
        ast::ParameterType::Object(_) => {
            return Err(Error::unimplemented(
                "ast_param_type_to_type for Object".into(),
            ))
        }
    };

    Ok(if !required {
        Type::Nullable(Box::new(t))
    } else {
        t
    })
}

fn ast_type_to_type(required: bool, type_: &ast::Type) -> Type {
    let t = match type_ {
        ast::Type::String => Type::String,
        ast::Type::Number => Type::PrimitiveType(PrimitiveType::Float32),
        ast::Type::F32 => Type::PrimitiveType(PrimitiveType::Float32),
        ast::Type::F64 => Type::PrimitiveType(PrimitiveType::Float64),
        ast::Type::U32 => Type::PrimitiveType(PrimitiveType::UInt32),
        ast::Type::U64 => Type::PrimitiveType(PrimitiveType::UInt64),
        ast::Type::I32 => Type::PrimitiveType(PrimitiveType::Int32),
        ast::Type::I64 => Type::PrimitiveType(PrimitiveType::Int64),
        ast::Type::PublicKey => Type::PublicKey,
        ast::Type::Bytes => Type::Bytes,
        ast::Type::ForeignRecord { contract } => Type::ContractReference {
            contract: contract.clone(),
        },
        ast::Type::Array(t) => Type::Array(Box::new(ast_type_to_type(true, t))),
        ast::Type::Boolean => Type::PrimitiveType(PrimitiveType::Boolean),
        ast::Type::Map(k, v) => Type::Map(
            Box::new(ast_type_to_type(true, k)),
            Box::new(ast_type_to_type(true, v)),
        ),
        ast::Type::Object(o) => {
            let mut fields = vec![];
            for field in o {
                fields.push((
                    field.name.clone(),
                    ast_type_to_type(field.required, &field.type_),
                ));
            }
            Type::Struct(Struct {
                name: "anonymous".to_owned(),
                fields,
            })
        }
    };

    if !required {
        Type::Nullable(Box::new(t))
    } else {
        t
    }
}

/// A function that takes in a struct type and generates a program that hashes a value of that type and returns the hash on the stack.
pub fn compile_hasher(t: Type, salts: Option<&[u32]>) -> Result<String> {
    let mut instructions = vec![];
    let mut memory = Memory::new();
    let empty_program = ast::Program { nodes: vec![] };
    let scope = prepare_scope(&empty_program);

    {
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);

        let salts = salts.map(|s| {
            s.iter()
                .map(|s| {
                    let salt = compiler
                        .memory
                        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
                    compiler.memory.write(
                        compiler.instructions,
                        salt.memory_addr,
                        &[ValueSource::Immediate(*s)],
                    );
                    salt
                })
                .collect::<Vec<_>>()
        });

        let hash = match t {
            Type::Struct(struct_) => {
                let value = compiler
                    .memory
                    .allocate_symbol(Type::Struct(struct_.clone()));
                read_struct_from_advice_tape(&mut compiler, &value, &struct_, None)?;

                hash_record_with_salts(&mut compiler, &value, salts.as_ref().unwrap())?
            }
            t => {
                let value = read_advice_generic(&mut compiler, &t)?;

                let hash = hash(&mut compiler, value)?;
                if let Some(salts) = salts {
                    add_salt_to_hash(&mut compiler, &hash, &salts[0])?
                } else {
                    hash
                }
            }
        };

        comment!(compiler, "Reading result from memory");
        compiler.memory.read(
            compiler.instructions,
            hash.memory_addr,
            hash.type_.miden_width(),
        );
    }

    let instructions = encoder::unabstract(
        instructions,
        &mut |size| memory.allocate(size),
        &mut None,
        &mut None,
        &mut false,
        false,
    );

    let mut miden_code = String::new();
    miden_code.push_str("use.std::math::u64\n");
    miden_code.push_str("begin\n");
    miden_code.push_str("  push.");
    miden_code.push_str(&memory.static_alloc_ptr.to_string());
    miden_code.push_str("\n  mem_store.3\n"); // dynamic allocation pointer
    for instruction in instructions {
        instruction
            .encode(unsafe { miden_code.as_mut_vec() }, 1)
            .context(IoSnafu)?;
        miden_code.push('\n');
    }
    miden_code.push_str("end\n");

    Ok(miden_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_f64_to_f32() {
        convert_f64_to_f32(0.0).unwrap();
        convert_f64_to_f32(1.0).unwrap();

        assert_eq!(convert_f64_to_f32(std::f64::consts::PI), None);
        assert_eq!(convert_f64_to_f32(-std::f64::consts::PI), None);

        assert_eq!(convert_f64_to_f32(std::f64::MAX), None);
        assert_eq!(convert_f64_to_f32(std::f64::MIN), None);
    }
}
