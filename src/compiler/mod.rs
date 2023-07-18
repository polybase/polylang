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

use crate::ast::{self, Expression, Statement};
use abi::{self, publickey::Key, Abi, PrimitiveType, StdVersion, Struct, Type};
use std::ops::Deref;

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
                Some(type_) => scope.add_method(type_.clone(), name.clone(), func.clone()),
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
            Function::Builtin(Box::new(&|_, _, _| {
                panic!("this function should never be called");
            })),
        ));

        builtins.push((
            "dynamicAlloc".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _scope, args| dynamic_alloc(compiler, args))),
        ));

        builtins.push((
            "writeMemory".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, args| {
                assert_eq!(args.len(), 2);
                let address = args.get(0).unwrap();
                let value = args.get(1).unwrap();

                assert_eq!(address.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                assert_eq!(value.type_, Type::PrimitiveType(PrimitiveType::UInt32));

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

                Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::UInt32),
                    memory_addr: 0,

                }
            })),
        ));

        builtins.push((
            "readAdvice".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, _| {
                let symbol = compiler
                    .memory
                    .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

                compiler.instructions.push(encoder::Instruction::AdvPush(1));
                compiler.memory.write(
                    compiler.instructions,
                    symbol.memory_addr,
                    &[ValueSource::Stack],
                );

                symbol
            })),
        ));

        builtins.push((
            "unsafeToString".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, args| {
                let length = args.get(0).unwrap();
                let address_ptr = args.get(1).unwrap();

                assert_eq!(length.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                assert_eq!(address_ptr.type_, Type::PrimitiveType(PrimitiveType::UInt32));

                let two = uint32::new(compiler, 2);
                let mut s = dynamic_alloc(compiler, &[two]);
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

                s
            })),
        ));

        builtins.push((
            "unsafeToBytes".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, args| {
                let length = args.get(0).unwrap();
                let address_ptr = args.get(1).unwrap();

                assert_eq!(length.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                assert_eq!(address_ptr.type_, Type::PrimitiveType(PrimitiveType::UInt32));

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

                s
            })),
        ));

        builtins.push((
            "unsafeToPublicKey".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, args| {
                let kty = args.get(0).unwrap();
                assert_eq!(kty.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                let crv = args.get(1).unwrap();
                assert_eq!(crv.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                let alg = args.get(2).unwrap();
                assert_eq!(alg.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                let use_ = args.get(3).unwrap();
                assert_eq!(use_.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                let extra_ptr = args.get(4).unwrap();
                assert_eq!(extra_ptr.type_, Type::PrimitiveType(PrimitiveType::UInt32));

                assert!(args.get(5).is_none());

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

                pk
            })),
        ));

        builtins.push(("deref".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            let address = args.get(0).unwrap();

            assert_eq!(address.type_, Type::PrimitiveType(PrimitiveType::UInt32));

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

            result
         }))));

         builtins.push(("addressOf".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            let a = args.get(0).unwrap();



            uint32::new(compiler, a.memory_addr)
         }))));


         builtins.push(("hashString".to_string(), None, Function::Builtin(Box::new(&|compiler, scope, args| string::hash(compiler, scope, args)))));

         // bytes and collection reference have the same layout as strings,
         // so we can reuse the hashing function
         builtins.push(("hashBytes".to_owned(), None, Function::Builtin(Box::new(&|compiler, scope, args| string::hash(compiler, scope, args)))));
         builtins.push(("hashCollectionReference".to_owned(), None, Function::Builtin(Box::new(&|compiler, scope, args| string::hash(compiler, scope, args)))));

         builtins.push(("hashArray".to_owned(), None, Function::Builtin(Box::new(&|compiler, scope, args| array::hash(compiler, scope, args)))));

        builtins.push(("hashMap".to_owned(), None, Function::Builtin(Box::new(&|compiler, _scope, args| {
            let map = args.get(0).unwrap();

            let keys = map::keys_arr(map);
            let values = map::values_arr(map);

            let (_, _, hash_array_fn) = HIDDEN_BUILTINS.iter().find(|(name, _, _)| name == "hashArray").unwrap();

            let keys_hash = compile_function_call(compiler, hash_array_fn, &[keys], None);
            let values_hash = compile_function_call(compiler, hash_array_fn, &[values], None);

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

            result
        }))));

         builtins.push(("hashPublicKey".to_owned(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            publickey::hash(compiler, args)
        }))));

        builtins.push((
            "uintToFloat".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _scope, args| {
                float32::from_uint32(compiler, &args[0])
            }))
        ));

        builtins.push((
            "intToFloat".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _scope, args| {
                float32::from_int32(compiler, &args[0])
            }))
        ));

        Box::leak(Box::new(builtins))
    };
    static ref USABLE_BUILTINS: &'static [(String, Option<Type>, Function<'static>)] = {
        let mut builtins = Vec::new();

        builtins.push((
            "assert".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, args| {
                let condition = args.get(0).unwrap();
                let message = args.get(1).unwrap();

                assert_eq!(condition.type_, Type::PrimitiveType(PrimitiveType::Boolean));
                assert_eq!(message.type_, Type::String);

                let mut failure_branch = vec![];
                let mut failure_compiler = Compiler::new(&mut failure_branch, compiler.memory, compiler.root_scope);

                let error_fn = &USABLE_BUILTINS
                    .iter()
                    .find(|(name, _, _)| name == "error")
                    .unwrap()
                    .2;
                compile_function_call(&mut failure_compiler, error_fn, &[message.clone()], None);

                compiler.instructions.push(encoder::Instruction::If {
                    condition: vec![encoder::Instruction::MemLoad(Some(condition.memory_addr))],
                    then: vec![],
                    else_: failure_branch,
                });

                Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::Boolean),
                    memory_addr: 0,
                }
            })),
        ));

        builtins.push((
            "error".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, args| {
                let message = args.get(0).unwrap();
                assert_eq!(message.type_, Type::String);

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

                Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::Boolean),
                    memory_addr: 0,
                }
            })),
        ));

        builtins.push((
            "log".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let mut scope = compiler.root_scope.deeper();
                let result = log(compiler, &mut scope, args);
                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        builtins.push((
            "readAdviceString".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, args| {
                assert_eq!(args.len(), 0);

                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;

                let result = read_advice_string(compiler);
                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        builtins.push((
            "readAdviceBytes".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, _args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;

                let result_str = read_advice_string(compiler);
                let result = Symbol {
                    type_: Type::Bytes,
                    ..result_str
                };

                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        builtins.push((
            "readAdviceCollectionReference".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, _args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;

                let result_str = read_advice_string(compiler);
                let result = Symbol {
                    type_: Type::Bytes,
                    ..result_str
                };

                compiler.root_scope = old_root_scope;

                Symbol {
                    type_: Type::CollectionReference { collection: "".to_owned() },
                    ..result
                }
            })),
        ));

        builtins.push((
            "readAdvicePublicKey".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _, _args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;

                let result = read_advice_public_key(compiler);

                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        builtins.push(("readAdviceUInt32".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a u32
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            symbol
        }))));

        builtins.push(("readAdviceUInt64".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

            // high
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);

            // low
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr + 1, &[ValueSource::Stack]);

            result
        }))));

        builtins.push(("readAdviceInt32".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a u32
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            symbol
        }))));

        builtins.push(("readAdviceInt64".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Int64));

            // high
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);

            // low
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr + 1, &[ValueSource::Stack]);

            result
        }))));

        builtins.push(("readAdviceFloat32".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a u32
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Float32));
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            symbol
        }))));


        builtins.push(("readAdviceFloat64".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Float64));

            // high
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);

            // low
            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            compiler.memory.write(compiler.instructions, result.memory_addr + 1, &[ValueSource::Stack]);

            result
        }))));

        builtins.push(("readAdviceBoolean".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a boolean
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
            compiler.memory.write(compiler.instructions, symbol.memory_addr, &[ValueSource::Stack]);
            symbol
        }))));

        builtins.push(("uint32ToString".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            let old_root_scope = compiler.root_scope;
            compiler.root_scope = &BUILTINS_SCOPE;
            let result = compile_ast_function_call(&UINT32_TO_STRING, compiler, args, None);
            compiler.root_scope = old_root_scope;
            result
        }))));

        builtins.push(("uint32WrappingAdd".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            let b = &args[1];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));
            assert_eq!(b.type_, Type::PrimitiveType(PrimitiveType::UInt32));

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
            compiler.instructions.push(encoder::Instruction::U32WrappingAdd);
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);
            result
        }))));

        builtins.push(("uint32WrappingSub".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            let b = &args[1];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));
            assert_eq!(b.type_, Type::PrimitiveType(PrimitiveType::UInt32));

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
            compiler.instructions.push(encoder::Instruction::U32WrappingSub);
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);
            result
        }))));

        builtins.push(("uint32WrappingMul".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            let b = &args[1];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));
            assert_eq!(b.type_, Type::PrimitiveType(PrimitiveType::UInt32));

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
            compiler.instructions.push(encoder::Instruction::U32WrappingMul);
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);
            result
        }))));

        builtins.push(("uint32CheckedXor".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            let b = &args[1];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));
            assert_eq!(b.type_, Type::PrimitiveType(PrimitiveType::UInt32));

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
            result
        }))));

        builtins.push(("int32".to_string(), None, Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

            compiler.memory.read(
                compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.write(compiler.instructions, result.memory_addr, &[ValueSource::Stack]);

            result
        }))));

        builtins.push((
            "toHex".to_string(),
            Some(Type::PublicKey),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let result = publickey::to_hex(compiler, args);
                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        builtins.push((
            "arrayPush".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, scope, args| {
                array_push(compiler, scope, args)
            })),
        ));

        builtins.push((
            "mapLength".to_string(),
            None,
            Function::Builtin(Box::new(&|_compiler, _scope, args| {
                let m = &args[0];
                assert!(matches!(m.type_, Type::Map(_, _)));

                array::length(&map::keys_arr(m))
            }))
        ));

        builtins.push((
            "selfdestruct".to_string(),
            None,
            Function::Builtin(Box::new(&|compiler, _scope, _args| {
                compiler.memory.write(
                    compiler.instructions,
                    6,
                    &[ValueSource::Immediate(1)],
                );

                Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::Boolean),
                    memory_addr: 0,
                }
            })),
        ));

        Box::leak(Box::new(builtins))
    };
}

fn struct_field(
    _compiler: &mut Compiler,
    struct_symbol: &Symbol,
    field_name: &str,
) -> Option<Symbol> {
    let struct_ = match &struct_symbol.type_ {
        Type::Struct(struct_) => struct_,
        Type::CollectionReference { collection: _ } if field_name == "id" => {
            return Some(Symbol {
                type_: Type::String,
                memory_addr: struct_symbol.memory_addr,
            });
        }
        t => panic!("cannot access field {field_name} on type {t:?}"),
    };

    let mut offset = 0;
    for (name, field_type) in &struct_.fields {
        if name == field_name {
            return Some(Symbol {
                type_: field_type.clone(),
                memory_addr: struct_symbol.memory_addr + offset,
            });
        }

        offset += field_type.miden_width();
    }

    None
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Symbol {
    type_: Type,
    memory_addr: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct CollectionField {
    name: String,
    type_: Type,
    delegate: bool,
    read: bool,
}

#[derive(Debug, Clone)]
struct Collection<'ast> {
    name: String,
    fields: Vec<CollectionField>,
    functions: Vec<(String, &'ast ast::Function)>,
    call_directive: bool,
    read_directive: bool,
}

impl From<Collection<'_>> for Struct {
    fn from(collection: Collection<'_>) -> Self {
        let mut fields = Vec::new();
        for field in collection.fields {
            fields.push((field.name, field.type_));
        }

        Struct {
            name: collection.name,
            fields,
        }
    }
}

type BuiltinFn<'a> = Box<&'a (dyn Fn(&mut Compiler, &mut Scope, &[Symbol]) -> Symbol + Sync)>;

#[derive(Clone)]
enum Function<'ast> {
    Ast(&'ast ast::Function),
    Builtin(BuiltinFn<'ast>),
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
    methods: Vec<(Type, String, Function<'ast>)>,
    collections: Vec<(String, Collection<'ast>)>,
}

impl<'ast> Scope<'ast, '_> {
    fn new() -> Self {
        Scope {
            parent: None,
            symbols: vec![],
            non_null_symbol_addrs: vec![],
            functions: vec![],
            methods: vec![],
            collections: vec![],
        }
    }

    fn deeper<'b>(&'b self) -> Scope<'ast, 'b> {
        Scope {
            parent: Some(self),
            symbols: vec![],
            non_null_symbol_addrs: vec![],
            functions: vec![],
            methods: vec![],
            collections: vec![],
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

    fn add_method(&mut self, type_: Type, name: String, function: Function<'ast>) {
        self.methods.push((type_, name, function));
    }

    fn find_method(&self, type_: &Type, name: &str) -> Option<&Function<'ast>> {
        if let Some(func) = self
            .methods
            .iter()
            .rev()
            .find(|(t, n, _)| t == type_ && n == name)
            .map(|(_, _, f)| f)
        {
            return Some(func);
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.find_method(type_, name);
        }

        None
    }

    fn add_collection(&mut self, name: String, collection: Collection<'ast>) {
        if self.find_collection(&name).is_some() {
            panic!("Collection {} already exists", name);
        }

        self.collections.push((name, collection));
    }

    fn find_collection(&self, name: &str) -> Option<&Collection<'ast>> {
        if let Some(collection) = self
            .collections
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c)
        {
            return Some(collection);
        }

        self.parent.and_then(|p| p.find_collection(name))
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

    fn get_record_dependency(&mut self, col: &Collection) -> Option<Symbol> {
        self.record_depenencies
            .iter()
            .find(|(hashes, _)| hashes.collection == col.name)
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

fn compile_expression(expr: &Expression, compiler: &mut Compiler, scope: &Scope) -> Symbol {
    comment!(compiler, "Compiling expression {expr:?}");

    let symbol = match expr {
        Expression::Ident(id) => scope
            .find_symbol(id)
            .expect(&format!("symbol {} not found", id)),
        Expression::Primitive(ast::Primitive::Number(n, _has_decimal_point)) => {
            let n = convert_f64_to_f32(*n).expect("silent truncation");

            float32::new(compiler, n)
        }
        Expression::Primitive(ast::Primitive::String(s)) => string::new(compiler, s).0,
        Expression::Boolean(b) => boolean::new(compiler, *b),
        Expression::Add(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_add(compiler, &a, &b)
        }
        Expression::Subtract(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_sub(compiler, &a, &b)
        }
        Expression::Modulo(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_mod(compiler, &a, &b)
        }
        Expression::Divide(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_div(compiler, &a, &b)
        }
        Expression::Multiply(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_mul(compiler, &a, &b)
        }
        Expression::Equal(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_eq(compiler, &a, &b)
        }
        Expression::NotEqual(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_neq(compiler, &a, &b)
        }
        Expression::Not(x) => {
            let x = compile_expression(x, compiler, scope);
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
        Expression::Call(func, args) => {
            let is_in_hidden_builtin = scope.find_function("hiddenNoopMarker").is_some();
            let (func, args_symbols) = match func.deref() {
                Expression::Ident(id) if id == "u32_" && is_in_hidden_builtin => {
                    assert_eq!(args.len(), 1);

                    match args[0] {
                        Expression::Primitive(ast::Primitive::Number(n, _has_decimal_point)) => {
                            return uint32::new(compiler, n as u32)
                        }
                        _ => panic!("expected number"),
                    }
                }
                Expression::Ident(id) => (
                    scope
                        .find_function(id)
                        .expect(&format!("function {} not found", id)),
                    {
                        let mut args_symbols = vec![];
                        for arg in args {
                            args_symbols.push(compile_expression(arg, compiler, scope));
                        }
                        args_symbols
                    },
                ),
                Expression::Dot(obj_expr, func_name) => {
                    let obj = compile_expression(obj_expr, compiler, scope);

                    let func = scope.find_method(&obj.type_, &func_name).expect(&format!(
                        "method {} not found on type {:?}",
                        func_name, &obj.type_
                    ));

                    (func, {
                        let mut args_symbols = vec![obj];
                        for arg in args {
                            args_symbols.push(compile_expression(arg, compiler, scope));
                        }
                        args_symbols
                    })
                }
                _ => panic!("expected function name to be an identifier"),
            };

            compile_function_call(compiler, func, &args_symbols, None)
        }
        Expression::Assign(a, b) => {
            if let (Expression::Index(a, index), b) = (&**a, b) {
                let a = compile_expression(a, compiler, scope);
                let b = compile_expression(b, compiler, scope);
                let index = compile_expression(index, compiler, scope);

                let (_key, _value, value_ptr, did_find) = map::get(compiler, &a, &index);

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

                    array_push(compiler, scope, &[map::keys_arr(&a), index]);
                    array_push(compiler, scope, &[map::values_arr(&a), b.clone()]);

                    std::mem::swap(compiler.instructions, &mut if_not_found);
                }

                compiler.instructions.extend([encoder::Instruction::If {
                    condition: vec![encoder::Instruction::MemLoad(Some(did_find.memory_addr))],
                    then: if_found_instructions,
                    else_: if_not_found,
                }]);

                return b;
            }

            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            match (&a.type_, &b.type_) {
                (Type::Struct(a_struct), Type::Struct(_b_struct)) => {
                    for (field, ty) in &a_struct.fields {
                        let a_field = struct_field(compiler, &a, field).unwrap();
                        let b_field = struct_field(compiler, &b, field)
                            .unwrap_or_else(|| panic!("field {} not found", field));

                        assert_eq!(ty, &b_field.type_);

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
                    assert_eq!(a_inner_type.as_ref(), b_type);

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
                    assert!(
                        a_type == b_type,
                        "type mismatch at assign: {:?} != {:?}",
                        a_type,
                        b_type
                    );

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
        Expression::Dot(a, b) => {
            let a = compile_expression(a, compiler, scope);

            struct_field(compiler, &a, b).unwrap()
        }
        Expression::GreaterThanOrEqual(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_gte(compiler, &a, &b)
        }
        Expression::GreaterThan(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_gt(compiler, &a, &b)
        }
        Expression::LessThanOrEqual(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_lte(compiler, &a, &b)
        }
        Expression::LessThan(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_lt(compiler, &a, &b)
        }
        Expression::ShiftLeft(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_shift_left(compiler, &a, &b)
        }
        Expression::ShiftRight(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_shift_right(compiler, &a, &b)
        }
        Expression::And(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            boolean::compile_and(compiler, &a, &b)
        }
        Expression::Or(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            boolean::compile_or(compiler, &a, &b)
        }
        Expression::Array(exprs) => {
            let mut symbols = vec![];
            for expr in exprs {
                symbols.push(compile_expression(expr, compiler, scope));
            }

            assert!(
                symbols.iter().all(|s| s.type_ == symbols[0].type_),
                "all array elements must be of the same type"
            );

            if symbols.is_empty() {
                array::new(
                    compiler,
                    0,
                    // TODO: We need to infer what the type of the array is,
                    // for example, if the user does `this.array = []` we need
                    // the type to be the same as this.array
                    Type::PrimitiveType(PrimitiveType::UInt32),
                )
                .0
            } else {
                let type_ = symbols[0].type_.clone();
                let (array, data_ptr) = array::new(compiler, symbols.len() as u32, type_);

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
        Expression::Object(obj) => {
            let mut types = Vec::new();
            let mut values = Vec::new();
            for (field, expr) in &obj.fields {
                let symbol = compile_expression(expr, compiler, scope);
                types.push((field.clone(), symbol.type_.clone()));
                values.push((field, symbol));
            }

            let struct_type = Type::Struct(Struct {
                name: "anonymous".to_owned(),
                fields: types,
            });

            let symbol = compiler.memory.allocate_symbol(struct_type);
            for (field, expr_symbol) in values {
                let field = struct_field(compiler, &symbol, field).unwrap();
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
        Expression::Index(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_index(compiler, &a, &b)
        }
        e => unimplemented!("{:?}", e),
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

    symbol
}

fn compile_statement(
    statement: &Statement,
    compiler: &mut Compiler,
    scope: &mut Scope,
    return_result: &mut Symbol,
) {
    match statement {
        Statement::Return(expr) => {
            let symbol = compile_expression(expr, compiler, scope);
            compiler.memory.read(
                compiler.instructions,
                symbol.memory_addr,
                symbol.type_.miden_width(),
            );
            compiler.memory.write(
                compiler.instructions,
                return_result.memory_addr,
                &vec![ValueSource::Stack; symbol.type_.miden_width() as usize],
            );
            compiler.instructions.push(encoder::Instruction::Abstract(
                encoder::AbstractInstruction::Return,
            ));
        }
        Statement::Break => {
            compiler.instructions.push(encoder::Instruction::Abstract(
                encoder::AbstractInstruction::Break,
            ));
        }
        Statement::If(ast::If {
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
            let condition_symbol = compile_expression(condition, &mut condition_compiler, &scope);
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
                );
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
                );
            }

            compiler.instructions.push(encoder::Instruction::If {
                condition: condition_instructions,
                then: body_instructions,
                else_: else_body_instructions,
            })
        }
        Statement::While(ast::While {
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
            let condition_symbol = compile_expression(condition, &mut condition_compiler, &scope);
            assert_eq!(
                condition_symbol.type_,
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
                compile_statement(statement, &mut body_compiler, &mut scope, return_result);
            }

            compiler.instructions.push(encoder::Instruction::While {
                condition: condition_instructions,
                body: body_instructions,
            })
        }
        Statement::For(ast::For {
            initial_statement,
            condition,
            post_statement,
            statements,
        }) => {
            // There is no `for` instruction, we have to use `while` instead
            let mut scope = scope.deeper();

            let mut initial_instructions = vec![];
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
                    compile_expression(e, &mut initial_compiler, &scope);
                }
            };

            let mut condition_instructions = vec![];
            let mut condition_compiler = Compiler::new(
                &mut condition_instructions,
                compiler.memory,
                compiler.root_scope,
            );
            let condition_symbol = compile_expression(condition, &mut condition_compiler, &scope);
            assert_eq!(
                condition_symbol.type_,
                Type::PrimitiveType(PrimitiveType::Boolean)
            );
            condition_compiler.memory.read(
                condition_compiler.instructions,
                condition_symbol.memory_addr,
                condition_symbol.type_.miden_width(),
            );

            let mut post_instructions = vec![];
            let mut post_compiler =
                Compiler::new(&mut post_instructions, compiler.memory, compiler.root_scope);
            compile_expression(post_statement, &mut post_compiler, &scope);

            let mut body_instructions = vec![];
            let mut body_compiler =
                Compiler::new(&mut body_instructions, compiler.memory, compiler.root_scope);
            let mut body_scope = scope.deeper();
            for statement in statements {
                compile_statement(
                    statement,
                    &mut body_compiler,
                    &mut body_scope,
                    return_result,
                );
            }

            compiler.instructions.extend(initial_instructions);
            compiler.instructions.push(encoder::Instruction::While {
                condition: condition_instructions,
                body: {
                    body_instructions.extend(post_instructions);
                    body_instructions
                },
            });
        }
        Statement::Let(let_statement) => compile_let_statement(let_statement, compiler, scope),
        Statement::Expression(expr) => {
            compile_expression(expr, compiler, scope);
        }
        Statement::Throw(expr) => {
            compile_expression(expr, compiler, scope);
        }
    }
}

fn compile_let_statement(let_statement: &ast::Let, compiler: &mut Compiler, scope: &mut Scope) {
    let symbol = compile_expression(&let_statement.expression, compiler, scope);
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

    scope.add_symbol(let_statement.identifier.to_string(), new_symbol);
}

fn compile_ast_function_call(
    function: &ast::Function,
    compiler: &mut Compiler,
    args: &[Symbol],
    this: Option<Symbol>,
) -> Symbol {
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

    let mut return_result = function_compiler
        .memory
        .allocate_symbol(match &function.return_type {
            None => Type::PrimitiveType(PrimitiveType::Boolean),
            Some(ast::Type::Number) => Type::PrimitiveType(PrimitiveType::Float32),
            Some(ast::Type::F32) => Type::PrimitiveType(PrimitiveType::Float32),
            Some(ast::Type::F64) => Type::PrimitiveType(PrimitiveType::Float64),
            Some(ast::Type::U32) => Type::PrimitiveType(PrimitiveType::UInt32),
            Some(ast::Type::U64) => Type::PrimitiveType(PrimitiveType::UInt64),
            Some(ast::Type::I32) => Type::PrimitiveType(PrimitiveType::Int32),
            Some(ast::Type::I64) => Type::PrimitiveType(PrimitiveType::Int64),
            Some(ast::Type::String) => Type::String,
            Some(ast::Type::PublicKey) => Type::PublicKey,
            Some(ast::Type::Bytes) => Type::Bytes,
            Some(ast::Type::ForeignRecord { collection }) => Type::CollectionReference {
                collection: collection.clone(),
            },
            Some(ast::Type::Boolean) => todo!(),
            Some(ast::Type::Array(_)) => todo!(),
            Some(ast::Type::Map(_, _)) => todo!(),
            Some(ast::Type::Object(_)) => todo!(),
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
        compile_statement(statement, &mut function_compiler, scope, &mut return_result);
    }

    compiler.instructions.push(encoder::Instruction::Abstract(
        encoder::AbstractInstruction::InlinedFunction(function_instructions),
    ));

    return_result
}

fn compile_function_call(
    compiler: &mut Compiler,
    function: &Function,
    args: &[Symbol],
    this: Option<Symbol>,
) -> Symbol {
    match function {
        Function::Ast(a) => compile_ast_function_call(a, compiler, args, this),
        Function::Builtin(b) => b(compiler, &mut Scope::new(), args),
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

fn compile_add(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
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
        (Type::String, Type::String) => string::concat(compiler, a, b),
        e => unimplemented!("{:?}", e),
    }
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

fn compile_eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
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

            eq_result
        }
        e => unimplemented!("{:?}", e),
    }
}

fn compile_neq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    if a.type_ == Type::PrimitiveType(PrimitiveType::Float32)
        && b.type_ == Type::PrimitiveType(PrimitiveType::Float32)
    {
        return float32::ne(compiler, a, b);
    }

    let eq = compile_eq(compiler, a, b);
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

fn compile_index(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match &a.type_ {
        Type::Map(k, _v) => {
            assert_eq!(k.as_ref(), &b.type_);

            let (_key, value, _value_ptr, _found) = map::get(compiler, a, b);
            value
        }
        x => todo!("can't index into {x:?}"),
    }
}

fn dynamic_alloc(compiler: &mut Compiler, args: &[Symbol]) -> Symbol {
    let size = &args[0];
    assert_eq!(size.type_, Type::PrimitiveType(PrimitiveType::UInt32));

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
    addr
}

fn log(compiler: &mut Compiler, scope: &mut Scope, args: &[Symbol]) -> Symbol {
    let mut str_args = vec![];

    for arg in args {
        let message = match &arg.type_ {
            Type::String => arg.clone(),
            Type::PrimitiveType(PrimitiveType::UInt32) => compile_function_call(
                compiler,
                scope.find_function("uint32ToString").unwrap(),
                &[arg.clone()],
                None,
            ),
            Type::PrimitiveType(PrimitiveType::Boolean) => compile_function_call(
                compiler,
                scope.find_function("uint32ToString").unwrap(),
                &[Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::UInt32),
                    ..arg.clone()
                }],
                None,
            ),
            t => unimplemented!("You can't log a {:?} yet", t),
        };

        str_args.push(message);
    }

    for arg in str_args {
        compile_function_call(compiler, &Function::Ast(&LOG_STRING), &[arg], None);
    }

    Symbol {
        type_: Type::PrimitiveType(PrimitiveType::Boolean),
        memory_addr: 0,
    }
}

fn read_advice_collection_reference(compiler: &mut Compiler, collection: String) -> Symbol {
    let r = compile_function_call(
        compiler,
        BUILTINS_SCOPE
            .find_function("readAdviceCollectionReference")
            .unwrap(),
        &[],
        None,
    );

    Symbol {
        type_: Type::CollectionReference { collection },
        ..r
    }
}

fn read_advice_public_key(compiler: &mut Compiler) -> Symbol {
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
    let extra_ptr = dynamic_alloc(compiler, &[n64]);

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

    result
}

fn read_advice_string(compiler: &mut Compiler) -> Symbol {
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

    let data_ptr = dynamic_alloc(compiler, &[str_len]);
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

    result
}

fn read_advice_array(compiler: &mut Compiler, element_type: &Type) -> Symbol {
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

    let data_ptr = dynamic_alloc(compiler, &[capacity]);

    let read_element_advice_insts = {
        let mut insts = vec![];
        std::mem::swap(compiler.instructions, &mut insts);

        let el = read_advice_generic(compiler, element_type);
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

    arr
}

fn read_advice_map(compiler: &mut Compiler, key_type: &Type, value_type: &Type) -> Symbol {
    // Maps are serialized as [keys_arr..., values_arr...]
    let result = compiler.memory.allocate_symbol(Type::Map(
        Box::new(key_type.clone()),
        Box::new(value_type.clone()),
    ));

    let key_array = read_advice_array(compiler, key_type);
    let value_array = read_advice_array(compiler, value_type);

    compiler.memory.write(
        compiler.instructions,
        map::keys_arr(&result).memory_addr,
        &[
            ValueSource::Memory(array::capacity(&key_array).memory_addr),
            ValueSource::Memory(array::length(&key_array).memory_addr),
            ValueSource::Memory(array::data_ptr(&key_array).memory_addr),
        ],
    );

    compiler.memory.write(
        compiler.instructions,
        map::values_arr(&result).memory_addr,
        &[
            ValueSource::Memory(array::capacity(&value_array).memory_addr),
            ValueSource::Memory(array::length(&value_array).memory_addr),
            ValueSource::Memory(array::data_ptr(&value_array).memory_addr),
        ],
    );

    result
}

fn read_advice_nullable(compiler: &mut Compiler, type_: Type) -> Symbol {
    assert!(matches!(type_, Type::Nullable(_)));

    let value_type = match &type_ {
        Type::Nullable(value_type) => value_type,
        _ => unreachable!(),
    };

    let is_not_null = compile_function_call(
        compiler,
        BUILTINS_SCOPE.find_function("readAdviceBoolean").unwrap(),
        &[],
        None,
    );

    let (value, read_value_insts) = {
        let mut insts = vec![];
        std::mem::swap(compiler.instructions, &mut insts);

        let value = read_advice_generic(compiler, value_type);
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

    s
}

fn array_push(compiler: &mut Compiler, _scope: &Scope, args: &[Symbol]) -> Symbol {
    let arr = args.get(0).unwrap();
    let element = args.get(1).unwrap();
    assert_eq!(
        arr.type_.clone(),
        Type::Array(Box::new(element.type_.clone()))
    );

    compiler
        .memory
        .read(compiler.instructions, array::length(arr).memory_addr, 1);
    // [len]
    compiler.instructions.push(encoder::Instruction::Push(1));
    // [1, len]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);
    // [len + 1]
    compiler.memory.write(
        compiler.instructions,
        array::length(arr).memory_addr,
        &[ValueSource::Stack],
    );
    // []

    compiler
        .memory
        .read(compiler.instructions, array::capacity(arr).memory_addr, 1);
    // [capacity]
    compiler
        .memory
        .read(compiler.instructions, array::length(arr).memory_addr, 1);
    // [len + 1, capacity]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedGTE);
    // [len + 1 >= capacity]

    // TODO: if false, reallocate and copy
    compiler.instructions.push(encoder::Instruction::Assert);
    // []

    compiler
        .memory
        .read(compiler.instructions, array::data_ptr(arr).memory_addr, 1);
    // [data_ptr]
    compiler
        .memory
        .read(compiler.instructions, array::length(arr).memory_addr, 1);
    compiler.instructions.push(encoder::Instruction::Push(1));
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSub);
    // [len, data_ptr]
    compiler
        .instructions
        .push(encoder::Instruction::Push(element.type_.miden_width()));
    // [element_width, len, data_ptr]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedMul);
    // [len * element_width, data_ptr]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);
    // [data_ptr + len * element_width]
    compiler.memory.read(
        compiler.instructions,
        element.memory_addr,
        element.type_.miden_width(),
    );
    // [element, data_ptr + len * element_width]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [data_ptr + len * element_width, element]
    compiler
        .instructions
        .push(encoder::Instruction::MemStore(None));
    // []

    // Return the element, same as push does in JS
    element.clone()
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

fn hash(compiler: &mut Compiler, value: Symbol) -> Symbol {
    let result = match &value.type_ {
        Type::Nullable(_) => {
            let h = compiler.memory.allocate_symbol(Type::Hash);

            let mut hash_value_instructions = vec![];
            std::mem::swap(compiler.instructions, &mut hash_value_instructions);
            let non_null_value_hash = hash(compiler, nullable::value(value.clone()));
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
        Type::String => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashString").unwrap(),
            &[value],
            None,
        ),
        Type::Bytes => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashBytes").unwrap(),
            &[value],
            None,
        ),
        Type::CollectionReference { .. } => compile_function_call(
            compiler,
            BUILTINS_SCOPE
                .find_function("hashCollectionReference")
                .unwrap(),
            &[value],
            None,
        ),
        Type::Array(_) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashArray").unwrap(),
            &[value],
            None,
        ),
        Type::Map(_, _) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashMap").unwrap(),
            &[value],
            None,
        ),
        Type::PublicKey => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("hashPublicKey").unwrap(),
            &[value],
            None,
        ),
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

                let field_hash = hash(compiler, field);

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

    assert_eq!(result.type_, Type::Hash);

    result
}

fn read_advice_generic(compiler: &mut Compiler, type_: &Type) -> Symbol {
    match type_ {
        Type::Nullable(_) => read_advice_nullable(compiler, type_.clone()),
        Type::PrimitiveType(PrimitiveType::Boolean) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceBoolean").unwrap(),
            &[],
            None,
        ),
        Type::PrimitiveType(PrimitiveType::UInt32) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceUInt32").unwrap(),
            &[],
            None,
        ),
        Type::PrimitiveType(PrimitiveType::UInt64) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceUInt64").unwrap(),
            &[],
            None,
        ),
        Type::PrimitiveType(PrimitiveType::Int32) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceInt32").unwrap(),
            &[],
            None,
        ),
        Type::PrimitiveType(PrimitiveType::Int64) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceInt64").unwrap(),
            &[],
            None,
        ),
        Type::PrimitiveType(PrimitiveType::Float32) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceFloat32").unwrap(),
            &[],
            None,
        ),
        Type::PrimitiveType(PrimitiveType::Float64) => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceFloat64").unwrap(),
            &[],
            None,
        ),
        Type::String => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceString").unwrap(),
            &[],
            None,
        ),
        Type::Bytes => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdviceBytes").unwrap(),
            &[],
            None,
        ),
        Type::CollectionReference { collection } => {
            read_advice_collection_reference(compiler, collection.clone())
        }
        Type::Array(t) => read_advice_array(compiler, t),
        Type::Struct(s) => {
            let symbol = compiler.memory.allocate_symbol(type_.clone());
            read_struct_from_advice_tape(compiler, &symbol, s);
            symbol
        }
        Type::PublicKey => compile_function_call(
            compiler,
            BUILTINS_SCOPE.find_function("readAdvicePublicKey").unwrap(),
            &[],
            None,
        ),
        Type::Map(k, v) => read_advice_map(compiler, k, v),
        _ => unimplemented!("{:?}", type_),
    }
}

fn read_struct_from_advice_tape(
    compiler: &mut Compiler,
    struct_symbol: &Symbol,
    struct_type: &Struct,
) {
    for (name, type_) in &struct_type.fields {
        let symbol = read_advice_generic(compiler, type_);

        let sf = struct_field(compiler, struct_symbol, name).unwrap();
        compiler.memory.read(
            compiler.instructions,
            symbol.memory_addr,
            symbol.type_.miden_width(),
        );
        compiler.memory.write(
            compiler.instructions,
            sf.memory_addr,
            &vec![ValueSource::Stack; symbol.type_.miden_width() as _],
        );
    }
}

fn read_collection_inputs(
    compiler: &mut Compiler,
    this_struct: Option<Struct>,
    args: &[Type],
) -> (Option<Symbol>, Vec<Symbol>) {
    let this = this_struct.map(|ts| compiler.memory.allocate_symbol(Type::Struct(ts)));
    let this_struct = this.as_ref().map(|t| {
        if let Type::Struct(s) = &t.type_ {
            s
        } else {
            unreachable!();
        }
    });

    if let Some(this_struct) = this_struct {
        read_struct_from_advice_tape(compiler, this.as_ref().unwrap(), this_struct);
    }

    let mut args_symbols = Vec::new();
    for arg in args {
        args_symbols.push(read_advice_generic(compiler, arg));
    }

    (this, args_symbols)
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
            ast::RootNode::Collection(c) => {
                let mut collection = Collection {
                    name: c.name.clone(),
                    functions: vec![],
                    fields: vec![],
                    call_directive: match c
                        .decorators
                        .iter()
                        .find(|d| d.name == "call" || d.name == "public")
                    {
                        Some(d) if d.arguments.len() == 0 => true,
                        Some(d) => {
                            panic!("Invalid {} directive, call() takes no arguments", &d.name)
                        }
                        None => false,
                    },
                    read_directive: match c
                        .decorators
                        .iter()
                        .find(|d| d.name == "read" || d.name == "public")
                    {
                        Some(d) if d.arguments.len() == 0 => true,
                        Some(d) => {
                            panic!("Invalid {} directive, read() takes no arguments", &d.name)
                        }
                        None => false,
                    },
                };

                for item in &c.items {
                    match item {
                        ast::CollectionItem::Field(f) => {
                            collection.fields.push(CollectionField {
                                name: f.name.clone(),
                                type_: ast_type_to_type(f.required, &f.type_),
                                delegate: f.decorators.iter().any(|d| d.name == "delegate"),
                                read: f.decorators.iter().any(|d| d.name == "read"),
                            });
                        }
                        ast::CollectionItem::Function(f) => {
                            collection.functions.push((f.name.clone(), f));
                        }
                        ast::CollectionItem::Index(_) => {}
                    }
                }

                scope.add_collection(collection.name.clone(), collection);
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
    collection_name: Option<&str>,
    function_name: &str,
) -> (String, Abi) {
    let mut scope = prepare_scope(&program);
    let collection = collection_name.map(|name| scope.find_collection(name).cloned().unwrap());
    let collection = collection.as_ref();
    let collection_struct = collection.map(|c| Struct::from(c.clone()));

    let (function, param_types) = match function_name {
        ".readAuth" => (None, vec![]),
        _ => {
            let function = collection
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
                .unwrap();

            let param_types = function
                .parameters
                .iter()
                .map(|p| ast_param_type_to_type(p.required, &p.type_, collection_struct.as_ref()))
                .collect::<Vec<_>>();

            (Some(function), param_types)
        }
    };

    let mut instructions = vec![];
    let mut memory = Memory::new();
    let this_addr;

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
        .collections
        .iter()
        .map(|c| {
            (
                abi::RecordHashes {
                    collection: c.0.clone(),
                },
                memory.allocate_symbol(Type::Array(Box::new(Type::Hash))),
            )
        })
        .collect::<Vec<_>>();

    {
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        compiler.record_depenencies = all_possible_record_dependencies.clone();

        let expected_hash = collection_struct.as_ref().map(|_| {
            let hash = compiler.memory.allocate_symbol(Type::Hash);
            compiler.memory.write(
                compiler.instructions,
                hash.memory_addr,
                &vec![ValueSource::Stack; hash.type_.miden_width() as _],
            );
            hash
        });

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

            let ptr = dynamic_alloc(&mut compiler, &[full_width.clone()]);

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

        read_struct_from_advice_tape(&mut compiler, &ctx, &ctx_struct);

        let (this_symbol, arg_symbols) =
            read_collection_inputs(&mut compiler, collection_struct.clone(), &param_types);

        let ctx_pk = struct_field(&mut compiler, &ctx, "publicKey").unwrap();
        if function.is_some() {
            let auth_result = compile_call_authorization_proof(
                &mut compiler,
                &ctx_pk,
                this_symbol.as_ref().unwrap(),
                collection_name.unwrap(),
                function_name,
            );

            let assert_fn = compiler.root_scope.find_function("assert").unwrap();
            let (error_str, _) = string::new(
                &mut compiler,
                "You are not authorized to call this function",
            );
            compile_function_call(&mut compiler, assert_fn, &[auth_result, error_str], None);
        }

        this_addr = this_symbol.as_ref().map(|ts| ts.memory_addr);

        if let Some(this_symbol) = &this_symbol {
            let this_hash = hash(&mut compiler, this_symbol.clone());
            // compiler.memory.read(
            //     &mut compiler.instructions,
            //     this_hash.memory_addr,
            //     this_hash.type_.miden_width(),
            // );
            let is_eq = compile_eq(&mut compiler, &this_hash, expected_hash.as_ref().unwrap());
            let assert_fn = compiler.root_scope.find_function("assert").unwrap();
            let (error_str, _) = string::new(
                &mut compiler,
                "Hash of this does not match the expected hash",
            );
            compile_function_call(&mut compiler, assert_fn, &[is_eq, error_str], None);
        }

        let result = match function {
            // read auth
            None => compile_read_authorization_proof(
                &mut compiler,
                this_symbol.as_ref().unwrap(),
                collection.as_ref().unwrap(),
                &ctx_pk,
            ),
            Some(function) => compile_ast_function_call(
                function,
                &mut compiler,
                &arg_symbols,
                this_symbol.clone(),
            ),
        };

        comment!(compiler, "Reading result from memory");
        compiler.memory.read(
            compiler.instructions,
            result.memory_addr,
            result.type_.miden_width(),
        );

        if let Some(this_symbol) = this_symbol {
            comment!(compiler, "Reading selfdestruct flag");
            compiler.memory.read(compiler.instructions, 6, 1);

            let this_hash = hash(&mut compiler, this_symbol);
            comment!(compiler, "Reading output `this` hash");
            compiler.memory.read(
                compiler.instructions,
                this_hash.memory_addr,
                this_hash.type_.miden_width(),
            );
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

    let mut miden_code = String::new();
    miden_code.push_str("use.std::math::u64\n");
    miden_code.push_str("begin\n");
    miden_code.push_str("  push.");
    miden_code.push_str(&memory.static_alloc_ptr.to_string());
    miden_code.push_str("\n  mem_store.3\n"); // dynamic allocation pointer
    for instruction in instructions {
        instruction
            .encode(unsafe { miden_code.as_mut_vec() }, 1)
            .unwrap();
        miden_code.push('\n');
    }
    miden_code.push_str("end\n");

    (
        miden_code,
        Abi {
            this_addr,
            this_type: collection_struct.map(Type::Struct),
            param_types,
            other_collection_types: scope
                .collections
                .iter()
                .map(|c| Type::Struct(Struct::from(c.1.clone())))
                .collect(),
            other_records: all_possible_record_dependencies
                .into_iter()
                .map(|x| x.0)
                .collect(),
            std_version: Some(StdVersion::V0_6_1),
        },
    )
}

fn compile_read_authorization_proof(
    compiler: &mut Compiler,
    struct_symbol: &Symbol,
    collection: &Collection,
    auth_pk: &Symbol,
) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    if collection.read_directive {
        compiler.instructions.push(encoder::Instruction::Push(1));
        compiler
            .instructions
            .push(encoder::Instruction::MemStore(Some(result.memory_addr)));
        return result;
    }

    for field in collection.fields.iter().filter(|f| f.read) {
        let field_symbol = struct_field(compiler, &struct_symbol, &field.name).unwrap();
        compiler.memory.read(
            &mut compiler.instructions,
            field_symbol.memory_addr,
            field_symbol.type_.miden_width(),
        );

        let passed = compile_check_eq_or_ownership(compiler, field_symbol, auth_pk);
        compiler.instructions.push(encoder::Instruction::If {
            condition: vec![encoder::Instruction::MemLoad(Some(passed.memory_addr))],
            then: vec![
                encoder::Instruction::Push(1),
                encoder::Instruction::MemStore(Some(result.memory_addr)),
            ],
            else_: vec![],
        });
    }

    result
}

fn compile_call_authorization_proof(
    compiler: &mut Compiler,
    // Symbol of type Type::Nullable(Type::PublicKey)
    auth_pk: &Symbol,
    collection_symbol: &Symbol,
    collection_name: &str,
    function_name: &str,
) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    if function_name == "constructor" {
        compiler.instructions.push(encoder::Instruction::Push(1));
        compiler
            .instructions
            .push(encoder::Instruction::MemStore(Some(result.memory_addr)));
        return result;
    }

    let scope = compiler.root_scope;
    let collection = scope.find_collection(collection_name).unwrap();
    // let collection_struct = Struct::from(collection.clone());
    let Some((_, function)) = collection
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
    match (collection.call_directive, function_has_call_directive) {
        // Function call directive overrides the collection call directive.
        (_, true) => {}
        // The collection has a @call directive, but the function does not,
        // anyone can call it.
        (true, false) => {
            compiler.instructions.push(encoder::Instruction::Push(1));
            compiler
                .instructions
                .push(encoder::Instruction::MemStore(Some(result.memory_addr)));
            return result;
        }
        // Neither the collection nor the function have a @call directive,
        // no calls are allowed.
        (false, false) => return result,
    }

    let mut call_fields = call_decorators
        .flat_map(|d| &d.arguments)
        .map(|a| a.split('.').collect::<Vec<_>>())
        .peekable();

    if function_has_call_directive && call_fields.peek().is_none() {
        // The call is just `@call` with no fields, so no authorization required.
        compiler.instructions.push(encoder::Instruction::Push(1));
        compiler
            .instructions
            .push(encoder::Instruction::MemStore(Some(result.memory_addr)));
        return result;
    }

    for call_field_path in call_fields {
        let mut current_field = collection_symbol.clone();
        for field in call_field_path {
            current_field = match struct_field(compiler, &current_field, field) {
                Some(s) => s,
                None => panic!("Field {field} not found in {current_field:?}"),
            };
        }

        let passed = compile_check_eq_or_ownership(compiler, current_field, auth_pk);
        compiler.instructions.push(encoder::Instruction::If {
            condition: vec![encoder::Instruction::MemLoad(Some(passed.memory_addr))],
            then: vec![
                encoder::Instruction::Push(1),
                encoder::Instruction::MemStore(Some(result.memory_addr)),
            ],
            else_: vec![],
        });
    }

    result
}

fn compile_check_eq_or_ownership(
    compiler: &mut Compiler,
    field: Symbol,
    auth_pk: &Symbol,
) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    let is_eq = match &field.type_ {
        Type::PublicKey => compile_eq(compiler, &field, auth_pk),
        Type::Nullable(t) if **t == Type::PublicKey => compile_eq(compiler, &field, auth_pk),
        Type::CollectionReference { collection } => {
            let collection_type = compiler.root_scope.find_collection(&collection).unwrap();
            let collection_record_hashes = compiler.get_record_dependency(collection_type).unwrap();
            let id = struct_field(compiler, &field, "id").unwrap();

            let hash_id = hash(compiler, id.clone());
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
            );

            let (not_null_instructions, result) = {
                let mut insts = vec![];
                std::mem::swap(compiler.instructions, &mut insts);

                let public_hash_position = nullable::value(public_hash_position.clone());

                let record_public_hash =
                    array::get(compiler, &collection_record_hashes, &public_hash_position);

                let record = compiler
                    .memory
                    .allocate_symbol(Type::Struct(Struct::from(collection_type.clone())));
                read_struct_from_advice_tape(
                    compiler,
                    &record,
                    &Struct::from(collection_type.clone()),
                );
                let actual_record_hash = hash(compiler, record.clone());

                let is_hash_eq = compile_eq(
                    compiler,
                    dbg!(&record_public_hash), // addr 63
                    dbg!(&actual_record_hash), // addr 77
                );
                let assert = compiler.root_scope.find_function("assert").unwrap();
                let (error_str, _) =
                    string::new(compiler, "Record hash does not match the expected hash");
                compile_function_call(compiler, assert, &[is_hash_eq, error_str], None);

                let record_id = struct_field(compiler, &record, "id").unwrap();
                let is_id_eq = compile_eq(compiler, &record_id, &id);
                let (error_str, _) = string::new(compiler, "Record id does not match");
                compile_function_call(compiler, assert, &[is_id_eq, error_str], None);

                let result = compile_check_ownership(compiler, &record, collection_type, auth_pk);

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
        Type::Array(t) => {
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

                let result =
                    compile_check_eq_or_ownership(compiler, current_array_element.clone(), auth_pk);

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

    result
}

fn compile_check_ownership(
    compiler: &mut Compiler,
    struct_symbol: &Symbol,
    collection: &Collection,
    auth_pk: &Symbol,
) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    let collection_struct = Struct::from(collection.clone());

    for delegate_field in collection.fields.iter().filter(|f| f.delegate) {
        let delegate_symbol = struct_field(compiler, struct_symbol, &delegate_field.name).unwrap();

        let is_eq = match &delegate_symbol.type_ {
            Type::PublicKey => compile_eq(compiler, &delegate_symbol, auth_pk),
            Type::Nullable(t) if **t == Type::PublicKey => {
                compile_eq(compiler, &delegate_symbol, auth_pk)
            }
            Type::CollectionReference { collection } => {
                todo!()
            }
            Type::Array(_) => todo!(),
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
    }

    result
}

/// collection_struct is the type used for `record` types
fn ast_param_type_to_type(
    required: bool,
    type_: &ast::ParameterType,
    collection_struct: Option<&Struct>,
) -> Type {
    let t = match type_ {
        ast::ParameterType::String => Type::String,
        ast::ParameterType::Number => Type::PrimitiveType(PrimitiveType::Float32),
        ast::ParameterType::F32 => Type::PrimitiveType(PrimitiveType::Float32),
        ast::ParameterType::F64 => Type::PrimitiveType(PrimitiveType::Float64),
        ast::ParameterType::U32 => Type::PrimitiveType(PrimitiveType::UInt32),
        ast::ParameterType::U64 => Type::PrimitiveType(PrimitiveType::UInt64),
        ast::ParameterType::I32 => Type::PrimitiveType(PrimitiveType::Int32),
        ast::ParameterType::I64 => Type::PrimitiveType(PrimitiveType::Int64),
        ast::ParameterType::Record => Type::Struct(collection_struct.unwrap().clone()),
        ast::ParameterType::PublicKey => Type::PublicKey,
        ast::ParameterType::Bytes => Type::Bytes,
        ast::ParameterType::ForeignRecord { collection } => Type::CollectionReference {
            collection: collection.clone(),
        },
        ast::ParameterType::Array(t) => Type::Array(Box::new(ast_type_to_type(true, t))),
        ast::ParameterType::Boolean => todo!(),
        ast::ParameterType::Map(k, v) => Type::Map(
            Box::new(ast_type_to_type(true, k)),
            Box::new(ast_type_to_type(true, v)),
        ),
        ast::ParameterType::Object(_) => todo!(),
    };

    if !required {
        Type::Nullable(Box::new(t))
    } else {
        t
    }
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
        ast::Type::ForeignRecord { collection } => Type::CollectionReference {
            collection: collection.clone(),
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
pub fn compile_hasher(t: Type) -> String {
    let mut instructions = vec![];
    let mut memory = Memory::new();
    let empty_program = ast::Program { nodes: vec![] };
    let scope = prepare_scope(&empty_program);

    {
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);

        let this_symbol = match t {
            Type::Struct(struct_) => read_collection_inputs(&mut compiler, Some(struct_), &[]).0,
            t => Some(read_advice_generic(&mut compiler, &t)),
        };

        let hash = hash(&mut compiler, this_symbol.unwrap());

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
            .unwrap();
        miden_code.push('\n');
    }
    miden_code.push_str("end\n");

    miden_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_f64_to_f32() {
        convert_f64_to_f32(0.0).unwrap();
        convert_f64_to_f32(1.0).unwrap();

        assert_eq!(convert_f64_to_f32(3.14159265359), None);
        assert_eq!(convert_f64_to_f32(-3.14159265359), None);

        assert_eq!(convert_f64_to_f32(std::f64::MAX), None);
        assert_eq!(convert_f64_to_f32(std::f64::MIN), None);
    }
}
