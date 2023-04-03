pub mod abi;
mod boolean;
mod bytes;
mod encoder;
mod int32;
mod ir;
mod publickey;
mod string;
mod uint32;
mod uint64;

use std::{collections::HashMap, ops::Deref};

use serde::{Deserialize, Serialize};

use crate::{
    ast::{self, Expression, Statement},
    validation::Value,
};

macro_rules! comment {
    ($compiler:expr, $($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $compiler.comment(format!($($arg)*));
    };
}

lazy_static::lazy_static! {
    // TODO: rewrite this in raw instructions for better performance
    static ref READ_ADVICE_INTO_STRING: ast::Function = polylang_parser::parse_function(r#"
        function readAdviceIntoString(length: number, dataPtr: number): number {
            if (length == 0) return 0;
            let i = 0;
            let y = length - 1;
            while (y >= i) {
                writeMemory(dataPtr + i, readAdvice());
                i = i + 1;
            }

            return length;
        }
    "#).unwrap();
    static ref READ_ADVICE_STRING: ast::Function = polylang_parser::parse_function(r#"
        function readAdviceString(): string {
            let length = readAdvice();
            let dataPtr = dynamicAlloc(length);
            readAdviceIntoString(length, dataPtr);
            return unsafeToString(length, dataPtr);
        }
    "#).unwrap();
    static ref READ_ADVICE_BYTES: ast::Function = polylang_parser::parse_function(r#"
        function readAdviceBytes(): bytes {
            let length = readAdvice();
            let dataPtr = dynamicAlloc(length);

            let i = 0;
            while (i < length) {
                writeMemory(dataPtr + i, readAdvice());
                i = i + 1;
            }

            return unsafeToBytes(length, dataPtr);
        }
    "#).unwrap();
    static ref READ_ADVICE_PUBLIC_KEY: ast::Function = polylang_parser::parse_function(r#"
        function readAdvicePublicKey(): PublicKey {
            let kty = readAdvice();
            let crv = readAdvice();
            let alg = readAdvice();
            let use_ = readAdvice();
            let extraPtr = dynamicAlloc(64);

            let i = 0;
            while (i < 64) {
                writeMemory(extraPtr + i, readAdvice());
                i = i + 1;
            }
            
            return unsafeToPublicKey(kty, crv, alg, use_, extraPtr);
        }
    "#).unwrap();
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
            let currentLog = dynamicAlloc(2);
            writeMemory(currentLog, deref(addressOf(message)));
            writeMemory(currentLog + 1, deref(addressOf(message) + 1));

            let newLog = dynamicAlloc(2);
            writeMemory(newLog, deref(4));
            writeMemory(newLog + 1, deref(5));
            writeMemory(4, newLog);
            writeMemory(5, currentLog);
        }
    "#).unwrap();
    static ref PUBLIC_KEY_TO_HEX: ast::Function = polylang_parser::parse_function(r#"
        function publicKeyToHex(publicKey: PublicKey): string {
            let length = 2 + 64 * 2;
            let dataPtr = dynamicAlloc(length);

            // Write the 0x prefix
            writeMemory(dataPtr, 48);
            writeMemory(dataPtr + 1, 120);

            let i = 0;
            let extraPtr = deref(addressOf(publicKey) + 4);
            let startMem = dataPtr + 2;
            while (i < 32) {
                let pos = i * 2;
                let byte = deref(extraPtr + i);
                let firstDigit = byte / 16;
                let secondDigit = byte % 16;
                writeMemory(startMem + pos, firstDigit + 48);
                writeMemory(startMem + pos + 1, secondDigit + 48);
                // TODO: secondDigit values are correct,
                // but logging out the final string outputs:
                // 0x4<><?533>63531517:8;;76>03>6966<=4;3<208919>0:<<1558873:34773;4<5>:19>35397806824<00=0<:=7:=315>11:3:51:;2=1;1>:2;?467?:3;1<:35
                // The bug might be unrelated to this function.
                i = i + 1;
            }

            return unsafeToString(length, dataPtr);
        }
    "#).unwrap();
    static ref BUILTINS_SCOPE: &'static Scope<'static, 'static> = {
        let mut scope = Scope::new();

        for function in HIDDEN_BUILTINS.iter() {
            scope.add_function(function.0.clone(), function.1.clone());
        }

        for function in USABLE_BUILTINS.iter() {
            scope.add_function(function.0.clone(), function.1.clone());
        }

        Box::leak(Box::new(scope))
    };
    static ref HIDDEN_BUILTINS: &'static [(String, Function<'static>)] = {
        let mut builtins = Vec::new();

        builtins.push((
            "dynamicAlloc".to_string(),
            Function::Builtin(Box::new(&|compiler, scope, args| dynamic_alloc(compiler, args))),
        ));

        builtins.push((
            "writeMemory".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                assert_eq!(args.len(), 2);
                let address = args.get(0).unwrap();
                let value = args.get(1).unwrap();

                assert_eq!(address.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                assert_eq!(value.type_, Type::PrimitiveType(PrimitiveType::UInt32));

                compiler.memory.read(
                    &mut compiler.instructions,
                    value.memory_addr,
                    value.type_.miden_width(),
                );
                compiler.memory.read(
                    &mut compiler.instructions,
                    address.memory_addr,
                    address.type_.miden_width(),
                );
                compiler
                    .instructions
                    .push(encoder::Instruction::MemStore(None));
                compiler
                    .instructions
                    .push(encoder::Instruction::Drop);

                Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::UInt32),
                    memory_addr: 0,
                }
            })),
        ));

        builtins.push((
            "readAdvice".to_string(),
            Function::Builtin(Box::new(&|compiler, _, _| {
                let symbol = compiler
                    .memory
                    .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

                compiler.instructions.push(encoder::Instruction::AdvPush(1));
                compiler.memory.write(
                    &mut compiler.instructions,
                    symbol.memory_addr,
                    &vec![ValueSource::Stack],
                );

                symbol
            })),
        ));

        builtins.push((
            "readAdviceIntoString".to_string(),
            Function::AST(&READ_ADVICE_INTO_STRING),
        ));

        builtins.push((
            "unsafeToString".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let length = args.get(0).unwrap();
                let address_ptr = args.get(1).unwrap();

                assert_eq!(length.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                assert_eq!(address_ptr.type_, Type::PrimitiveType(PrimitiveType::UInt32));

                let two = uint32::new(compiler, 2);
                let mut s = dynamic_alloc(compiler, &[two]);
                s.type_ = Type::String;

                compiler.memory.read(
                    &mut compiler.instructions,
                    length.memory_addr,
                    length.type_.miden_width(),
                );
                compiler.memory.write(
                    &mut compiler.instructions,
                    string::length(&s).memory_addr,
                    &vec![ValueSource::Stack; length.type_.miden_width() as _],
                );

                compiler.memory.read(
                    &mut compiler.instructions,
                    address_ptr.memory_addr,
                    address_ptr.type_.miden_width(),
                );
                compiler.memory.write(
                    &mut compiler.instructions,
                    string::data_ptr(&s).memory_addr,
                    &vec![ValueSource::Stack; address_ptr.type_.miden_width() as _],
                );

                s
            })),
        ));

        builtins.push((
            "unsafeToBytes".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let length = args.get(0).unwrap();
                let address_ptr = args.get(1).unwrap();

                assert_eq!(length.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                assert_eq!(address_ptr.type_, Type::PrimitiveType(PrimitiveType::UInt32));

                let two = uint32::new(compiler, 2);
                let mut s = dynamic_alloc(compiler, &[two]);
                s.type_ = Type::Bytes;

                compiler.memory.read(
                    &mut compiler.instructions,
                    length.memory_addr,
                    length.type_.miden_width(),
                );
                compiler.memory.write(
                    &mut compiler.instructions,
                    string::length(&s).memory_addr,
                    &vec![ValueSource::Stack; length.type_.miden_width() as _],
                );

                compiler.memory.read(
                    &mut compiler.instructions,
                    address_ptr.memory_addr,
                    address_ptr.type_.miden_width(),
                );
                compiler.memory.write(
                    &mut compiler.instructions,
                    string::data_ptr(&s).memory_addr,
                    &vec![ValueSource::Stack; address_ptr.type_.miden_width() as _],
                );

                s
            })),
        ));

        builtins.push((
            "unsafeToPublicKey".to_string(),
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
                    &mut compiler.instructions,
                    kty.memory_addr,
                    kty.type_.miden_width(),
                );

                compiler.memory.write(
                    &mut compiler.instructions,
                    publickey::kty(&pk).memory_addr,
                    &vec![ValueSource::Stack; kty.type_.miden_width() as _],
                );

                compiler.memory.read(
                    &mut compiler.instructions,
                    crv.memory_addr,
                    crv.type_.miden_width(),
                );

                compiler.memory.write(
                    &mut compiler.instructions,
                    publickey::crv(&pk).memory_addr,
                    &vec![ValueSource::Stack; crv.type_.miden_width() as _],
                );

                compiler.memory.read(
                    &mut compiler.instructions,
                    alg.memory_addr,
                    alg.type_.miden_width(),
                );

                compiler.memory.write(
                    &mut compiler.instructions,
                    publickey::alg(&pk).memory_addr,
                    &vec![ValueSource::Stack; alg.type_.miden_width() as _],
                );

                compiler.memory.read(
                    &mut compiler.instructions,
                    use_.memory_addr,
                    use_.type_.miden_width(),
                );

                compiler.memory.write(
                    &mut compiler.instructions,
                    publickey::use_(&pk).memory_addr,
                    &vec![ValueSource::Stack; use_.type_.miden_width() as _],
                );

                compiler.memory.read(
                    &mut compiler.instructions,
                    extra_ptr.memory_addr,
                    extra_ptr.type_.miden_width(),
                );

                compiler.memory.write(
                    &mut compiler.instructions,
                    publickey::extra_ptr(&pk).memory_addr,
                    &vec![ValueSource::Stack; extra_ptr.type_.miden_width() as _],
                );

                pk
            })),
        ));

        builtins.push(("deref".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            let address = args.get(0).unwrap();

            assert_eq!(address.type_, Type::PrimitiveType(PrimitiveType::UInt32));

            let result = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                &mut compiler.instructions,
                address.memory_addr,
                address.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::MemLoad(None));
            compiler.memory.write(
                &mut compiler.instructions,
                result.memory_addr,
                &[ValueSource::Stack],
            );

            result
         }))));

         builtins.push(("addressOf".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            let a = args.get(0).unwrap();

            let result = uint32::new(compiler, a.memory_addr);

            result
         }))));


        let hash_string = Function::Builtin(Box::new(&|compiler, _, args| {
            let string = args.get(0).unwrap();
            assert!(matches!(string.type_, Type::String | Type::Bytes | Type::CollectionReference { .. }));

            let result = compiler
                .memory
                .allocate_symbol(Type::Hash);

            compiler.instructions.extend([
                encoder::Instruction::Push(0),
                encoder::Instruction::Push(0),
                encoder::Instruction::Push(0),
                encoder::Instruction::Push(0),
            ]);
            // [h[3], h[2], h[1], h[0]]
            compiler.memory.read(
                &mut compiler.instructions,
                string::data_ptr(string).memory_addr,
                string::data_ptr(string).type_.miden_width(),
            );
            // [data_ptr, h[3], h[2], h[1], h[0]]
            compiler.memory.read(
                &mut compiler.instructions,
                string::length(string).memory_addr,
                string::length(string).type_.miden_width(),
            );
            // [len, data_ptr, h[3], h[2], h[1], h[0]]

            compiler.instructions.push(encoder::Instruction::While {
                // len > 0
                condition: vec![
                    encoder::Instruction::Dup(None),
                    // [len, len, data_ptr, h[3], h[2], h[1], h[0]]
                    encoder::Instruction::Push(0),
                    // [0, len, len, data_ptr, h[3], h[2], h[1], h[0]]
                    encoder::Instruction::U32CheckedGT,
                    // [len > 0, len, data_ptr, h[3], h[2], h[1], h[0]]
                ],
                body: vec![
                    // [len, data_ptr, h[3], h[2], h[1], h[0]]
                    encoder::Instruction::Push(1),
                    // [1, len, data_ptr, h[3], h[2], h[1], h[0]]
                    encoder::Instruction::U32CheckedSub,
                    // [len - 1, data_ptr, h[3], h[2], h[1], h[0]]
                    encoder::Instruction::MovDown(5),
                    // [data_ptr, h[3], h[2], h[1], h[0], len - 1]
                    encoder::Instruction::Dup(None),
                    // [data_ptr, data_ptr, h[3], h[2], h[1], h[0], len - 1]
                    encoder::Instruction::MovDown(6),
                    // [data_ptr, h[3], h[2], h[1], h[0], len - 1, data_ptr]
                    encoder::Instruction::MemLoad(None),
                    // [byte, h[3], h[2], h[1], h[0], len - 1, data_ptr]
                    encoder::Instruction::Push(0),
                    encoder::Instruction::Push(0),
                    encoder::Instruction::Push(0),
                    // [0, 0, 0, byte, h[3], h[2], h[1], h[0], len - 1, data_ptr]
                    encoder::Instruction::HMerge,
                    // [h[3], h[2], h[1], h[0], len - 1, data_ptr]
                    encoder::Instruction::MovUp(5),
                    // [data_ptr, h[3], h[2], h[1], h[0], len - 1]
                    encoder::Instruction::Push(1),
                    // [1, data_ptr, h[3], h[2], h[1], h[0], len - 1]
                    encoder::Instruction::U32CheckedAdd,
                    // [data_ptr + 1, h[3], h[2], h[1], h[0], len - 1]
                    encoder::Instruction::MovUp(5),
                    // [len - 1, data_ptr + 1, h[3], h[2], h[1], h[0]]
                ],
            });

            // [len, data_ptr, h[3], h[2], h[1], h[0]]
            compiler.instructions.push(encoder::Instruction::Drop);
            // [data_ptr, h[3], h[2], h[1], h[0]]
            compiler.instructions.push(encoder::Instruction::Drop);
            // [h[3], h[2], h[1], h[0]]

            compiler.memory.write(
                &mut compiler.instructions,
                result.memory_addr,
                &[ValueSource::Stack, ValueSource::Stack, ValueSource::Stack, ValueSource::Stack],
            );

            result
         }));

         builtins.push(("hashString".to_string(), hash_string.clone()));

         // bytes and collection reference have the same layout as strings,
         // so we can reuse the hashing function
         builtins.push(("hashBytes".to_owned(), hash_string.clone()));
         builtins.push(("hashCollectionReference".to_owned(), hash_string.clone()));

         builtins.push(("hashPublicKey".to_owned(), Function::Builtin(Box::new(&|compiler, _, args| {
            let public_key = args.get(0).unwrap();
            assert_eq!(public_key.type_, Type::PublicKey);

            let result = compiler
                .memory
                .allocate_symbol(Type::Hash);

            compiler.instructions.extend([
                encoder::Instruction::Push(0),
                encoder::Instruction::Push(0),
                encoder::Instruction::Push(0),
                encoder::Instruction::Push(0),
            ]);
            // [h[3], h[2], h[1], h[0]]
            compiler.memory.read(
                &mut compiler.instructions,
                public_key.memory_addr,
                4,
            );

            compiler.instructions.push(encoder::Instruction::HMerge);

            // We hashed kty, crv, alg, use. Now we need to hash the x and y coordinates.
            let extra_ptr = publickey::extra_ptr(public_key);
            // x
            for i in (0..32).step_by(4) {
                // [h[3], h[2], h[1], h[0]]
                compiler.memory.read(
                    &mut compiler.instructions,
                    extra_ptr.memory_addr + i,
                    4,
                );
                compiler.instructions.push(encoder::Instruction::HMerge);
            }

            // y
            for i in (32..64).step_by(4) {
                // [h[3], h[2], h[1], h[0]]
                compiler.memory.read(
                    &mut compiler.instructions,
                    extra_ptr.memory_addr + i,
                    4,
                );
                compiler.instructions.push(encoder::Instruction::HMerge);
            }

            compiler.memory.write(
                &mut compiler.instructions,
                result.memory_addr,
                &[ValueSource::Stack, ValueSource::Stack, ValueSource::Stack, ValueSource::Stack],
            );

            result
        }))));

        Box::leak(Box::new(builtins))
    };
    static ref USABLE_BUILTINS: &'static [(String, Function<'static>)] = {
        let mut builtins = Vec::new();

        builtins.push((
            "assert".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let condition = args.get(0).unwrap();
                let message = args.get(1).unwrap();

                assert_eq!(condition.type_, Type::PrimitiveType(PrimitiveType::Boolean));
                assert_eq!(message.type_, Type::String);

                let mut failure_branch = vec![];
                let mut failure_compiler = Compiler::new(&mut failure_branch, compiler.memory, compiler.root_scope);

                let str_len = string::length(message);
                let str_data_ptr = string::data_ptr(message);

                failure_compiler.memory.write(
                    &mut failure_compiler.instructions,
                    1,
                    &vec![
                        ValueSource::Memory(str_len.memory_addr),
                        ValueSource::Memory(str_data_ptr.memory_addr),
                    ],
                );

                failure_compiler
                    .instructions
                    .push(encoder::Instruction::Push(0));
                failure_compiler
                    .instructions
                    .push(encoder::Instruction::Assert);

                compiler.instructions.push(encoder::Instruction::If {
                    condition: vec![encoder::Instruction::MemLoad(Some(condition.memory_addr))],
                    then: vec![],
                    // fail on purpose with assert(0)
                    else_: failure_branch,
                });

                Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::Boolean),
                    memory_addr: 0,
                }
            })),
        ));

        builtins.push((
            "log".to_string(),
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
            Function::Builtin(Box::new(&|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let result = compile_ast_function_call(&READ_ADVICE_STRING, compiler, args, None);
                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        builtins.push((
            "readAdviceBytes".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let result = compile_ast_function_call(&READ_ADVICE_BYTES, compiler, args, None);
                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        builtins.push((
            "readAdviceCollectionReference".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;

                let result = compile_ast_function_call(&READ_ADVICE_BYTES, compiler, args, None);
                compiler.root_scope = old_root_scope;

                let result = Symbol {
                    type_: Type::CollectionReference { collection: "".to_owned() },
                    ..result
                };

                result
            })),
        ));

        builtins.push((
            "readAdvicePublicKey".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let result = compile_ast_function_call(&READ_ADVICE_PUBLIC_KEY, compiler, args, None);
                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        builtins.push(("readAdviceUInt32".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a u32
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
            compiler.memory.write(&mut compiler.instructions, symbol.memory_addr, &vec![ValueSource::Stack]);
            symbol
        }))));

        builtins.push(("readAdviceBoolean".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a boolean
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
            compiler.memory.write(&mut compiler.instructions, symbol.memory_addr, &vec![ValueSource::Stack]);
            symbol
        }))));

        builtins.push(("uint32ToString".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            let old_root_scope = compiler.root_scope;
            compiler.root_scope = &BUILTINS_SCOPE;
            let result = compile_ast_function_call(&UINT32_TO_STRING, compiler, args, None);
            compiler.root_scope = old_root_scope;
            result
        }))));

        builtins.push(("uint32WrappingAdd".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            let b = &args[1];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));
            assert_eq!(b.type_, Type::PrimitiveType(PrimitiveType::UInt32));

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                &mut compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.read(
                &mut compiler.instructions,
                b.memory_addr,
                b.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::U32WrappingAdd);
            compiler.memory.write(&mut compiler.instructions, result.memory_addr, &vec![ValueSource::Stack]);
            result
        }))));

        builtins.push(("uint32WrappingSub".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            let b = &args[1];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));
            assert_eq!(b.type_, Type::PrimitiveType(PrimitiveType::UInt32));

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                &mut compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.read(
                &mut compiler.instructions,
                b.memory_addr,
                b.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::U32WrappingSub);
            compiler.memory.write(&mut compiler.instructions, result.memory_addr, &vec![ValueSource::Stack]);
            result
        }))));

        builtins.push(("uint32WrappingMul".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            let b = &args[1];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));
            assert_eq!(b.type_, Type::PrimitiveType(PrimitiveType::UInt32));

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                &mut compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.read(
                &mut compiler.instructions,
                b.memory_addr,
                b.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::U32WrappingMul);
            compiler.memory.write(&mut compiler.instructions, result.memory_addr, &vec![ValueSource::Stack]);
            result
        }))));

        builtins.push(("uint32CheckedXor".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            let b = &args[1];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));
            assert_eq!(b.type_, Type::PrimitiveType(PrimitiveType::UInt32));

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

            compiler.memory.read(
                &mut compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.read(
                &mut compiler.instructions,
                b.memory_addr,
                b.type_.miden_width(),
            );
            compiler.instructions.push(encoder::Instruction::U32CheckedXOR);
            compiler.memory.write(&mut compiler.instructions, result.memory_addr, &vec![ValueSource::Stack]);
            result
        }))));

        // TODO: remove this when we add proper comments
        builtins.push(("comment".to_string(), Function::Builtin(Box::new(&|_, _, _| {
            Symbol {
                type_: Type::PrimitiveType(PrimitiveType::Boolean),
                memory_addr: 0,
            }
        }))));

        builtins.push(("int32".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            let a = &args[0];
            assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::UInt32));

            let result = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

            compiler.memory.read(
                &mut compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );
            compiler.memory.write(&mut compiler.instructions, result.memory_addr, &vec![ValueSource::Stack]);

            result
        }))));

        builtins.push((
            "publicKeyToHex".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let result = compile_ast_function_call(&PUBLIC_KEY_TO_HEX, compiler, args, None);
                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        Box::leak(Box::new(builtins))
    };
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveType {
    Boolean,
    UInt32,
    UInt64,
    Int32,
}

impl PrimitiveType {
    fn miden_width(&self) -> u32 {
        match self {
            PrimitiveType::Boolean => boolean::WIDTH,
            PrimitiveType::UInt32 => uint32::WIDTH,
            PrimitiveType::UInt64 => uint64::WIDTH,
            PrimitiveType::Int32 => int32::WIDTH,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<(String, Type)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Type {
    PrimitiveType(PrimitiveType),
    String,
    Bytes,
    CollectionReference {
        collection: String,
    },
    /// A type that can contain a 4-field wide hash, such as one returned by `hmerge`
    Hash,
    PublicKey,
    Struct(Struct),
}

impl Type {
    fn miden_width(&self) -> u32 {
        match self {
            Type::PrimitiveType(pt) => pt.miden_width(),
            Type::String => string::WIDTH,
            Type::Bytes => bytes::WIDTH,
            Type::CollectionReference { .. } => bytes::WIDTH,
            Type::Hash => 4,
            Type::PublicKey => publickey::WIDTH,
            Type::Struct(struct_) => struct_.fields.iter().map(|(_, t)| t.miden_width()).sum(),
        }
    }
}

fn new_struct(compiler: &mut Compiler, struct_: Struct) -> Symbol {
    let symbol = compiler.memory.allocate_symbol(Type::Struct(struct_));

    symbol
}

fn struct_field(struct_symbol: &Symbol, field_name: &str) -> Option<Symbol> {
    let struct_ = match &struct_symbol.type_ {
        Type::Struct(struct_) => struct_,
        t => panic!("expected struct, got: {:?}", t),
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

#[derive(Debug, Clone)]
pub(crate) struct Symbol {
    type_: Type,
    memory_addr: u32,
}

#[derive(Debug, Clone)]
struct Collection<'ast> {
    name: String,
    fields: Vec<(String, Type)>,
    functions: Vec<(String, &'ast ast::Function)>,
}

#[derive(Clone)]
enum Function<'ast> {
    AST(&'ast ast::Function),
    Builtin(Box<&'static (dyn Fn(&mut Compiler, &mut Scope, &[Symbol]) -> Symbol + Sync)>),
}

impl std::fmt::Debug for Function<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Function::AST(ast) => write!(f, "Function::AST({:?})", ast),
            Function::Builtin(_) => write!(f, "Function::Builtin"),
        }
    }
}

#[derive(Debug, Clone)]
struct Scope<'ast, 'b> {
    parent: Option<&'b Scope<'ast, 'b>>,
    symbols: Vec<(String, Symbol)>,
    functions: Vec<(String, Function<'ast>)>,
    collections: Vec<(String, Collection<'ast>)>,
}

impl<'ast> Scope<'ast, '_> {
    fn new() -> Self {
        Scope {
            parent: None,
            symbols: vec![],
            functions: vec![],
            collections: vec![],
        }
    }

    fn deeper<'b>(&'b self) -> Scope<'ast, 'b> {
        let scope = Scope {
            parent: Some(self),
            symbols: vec![],
            functions: vec![],
            collections: vec![],
        };

        scope
    }

    fn add_symbol(&mut self, name: String, symbol: Symbol) {
        self.symbols.push((name, symbol));
    }

    fn find_symbol(&self, name: &str) -> Option<&Symbol> {
        if let Some(symbol) = self
            .symbols
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s)
        {
            return Some(symbol);
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
            static_alloc_ptr: 6,
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
        }
    }

    fn comment(&mut self, comment: String) {
        self.instructions
            .push(encoder::Instruction::Comment(comment));
    }
}

fn compile_expression(expr: &Expression, compiler: &mut Compiler, scope: &Scope) -> Symbol {
    comment!(compiler, "Compiling expression {expr:?}");

    let symbol = match expr {
        Expression::Ident(id) => scope.find_symbol(id).unwrap().clone(),
        Expression::Primitive(ast::Primitive::Number(n)) => uint32::new(compiler, *n as u32),
        Expression::Primitive(ast::Primitive::String(s)) => string::new(compiler, s),
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
        Expression::Call(func, args) => {
            let func_name = match func.deref() {
                Expression::Ident(id) => id,
                _ => panic!("expected function name"),
            };
            let func = scope
                .find_function(func_name)
                .expect(format!("function {} not found", func_name).as_str());
            let mut args_symbols = vec![];
            for arg in args {
                args_symbols.push(compile_expression(arg, compiler, scope));
            }

            compile_function_call(compiler, func, &args_symbols, None)
        }
        Expression::Assign(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            assert_eq!(a.type_, b.type_);

            compiler
                .memory
                .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
            compiler.memory.write(
                compiler.instructions,
                a.memory_addr,
                &vec![ValueSource::Stack; b.type_.miden_width() as usize],
            );

            a
        }
        Expression::Dot(a, b) => {
            let a = compile_expression(a, compiler, scope);

            struct_field(&a, b).unwrap()
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
        e => unimplemented!("{:?}", e),
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
                &mut compiler.instructions,
                symbol.memory_addr,
                symbol.type_.miden_width(),
            );
            compiler.memory.write(
                &mut compiler.instructions,
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
            assert_eq!(
                condition_symbol.type_,
                Type::PrimitiveType(PrimitiveType::Boolean)
            );
            condition_compiler.memory.read(
                &mut condition_compiler.instructions,
                condition_symbol.memory_addr,
                condition_symbol.type_.miden_width(),
            );

            let mut body_instructions = vec![];
            let mut body_compiler =
                Compiler::new(&mut body_instructions, compiler.memory, compiler.root_scope);
            for statement in then_statements {
                compile_statement(statement, &mut body_compiler, &mut scope, return_result);
            }

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
                &mut condition_compiler.instructions,
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
                    compile_expression(e, &mut initial_compiler, &mut scope);
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
                &mut condition_compiler.instructions,
                condition_symbol.memory_addr,
                condition_symbol.type_.miden_width(),
            );

            let mut post_instructions = vec![];
            let mut post_compiler =
                Compiler::new(&mut post_instructions, compiler.memory, compiler.root_scope);
            compile_expression(post_statement, &mut post_compiler, &mut scope);

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
        st => unimplemented!("{:?}", st),
    }
}

fn compile_let_statement(let_statement: &ast::Let, compiler: &mut Compiler, scope: &mut Scope) {
    let symbol = compile_expression(&let_statement.expression, compiler, scope);
    // we need to copy symbol to a new symbol,
    // because Ident expressions return symbols of variables
    let new_symbol = compiler.memory.allocate_symbol(symbol.type_);
    compiler.memory.read(
        &mut compiler.instructions,
        symbol.memory_addr,
        new_symbol.type_.miden_width(),
    );
    compiler.memory.write(
        &mut compiler.instructions,
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
            Some(ast::Type::Number) => Type::PrimitiveType(PrimitiveType::UInt32),
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
            &mut function_compiler.instructions,
            arg.memory_addr,
            arg.type_.miden_width(),
        );
        function_compiler.memory.write(
            &mut function_compiler.instructions,
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
        Function::AST(a) => compile_ast_function_call(a, compiler, args, this),
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
        e => unimplemented!("{:?}", e),
    }
}

fn compile_neq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
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
        &mut compiler.instructions,
        addr.memory_addr,
        &vec![ValueSource::Stack],
    );
    compiler.memory.read(
        &mut compiler.instructions,
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
        compile_function_call(compiler, &Function::AST(&LOG_STRING), &[arg], None);
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
            .read(&mut compiler.instructions, value.memory_addr + i, 1);
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
        &mut compiler.instructions,
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
                    &mut compiler.instructions,
                    struct_hash.memory_addr,
                    struct_hash.type_.miden_width(),
                );
                compiler.memory.read(
                    &mut compiler.instructions,
                    field_hash.memory_addr,
                    field_hash.type_.miden_width(),
                );

                compiler.instructions.push(encoder::Instruction::HMerge);

                compiler.memory.write(
                    &mut compiler.instructions,
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

fn read_struct_from_advice_tape(
    compiler: &mut Compiler,
    struct_symbol: &Symbol,
    struct_type: &Struct,
) {
    for (name, type_) in &struct_type.fields {
        let symbol = match type_ {
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
            Type::PrimitiveType(PrimitiveType::UInt64) => todo!(),
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
            Type::PublicKey => compile_function_call(
                compiler,
                BUILTINS_SCOPE.find_function("readAdvicePublicKey").unwrap(),
                &[],
                None,
            ),
            _ => unimplemented!("{:?}", type_),
        };

        let sf = struct_field(struct_symbol, name).unwrap();
        compiler.memory.read(
            &mut compiler.instructions,
            symbol.memory_addr,
            symbol.type_.miden_width(),
        );
        compiler.memory.write(
            &mut compiler.instructions,
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
    let this_struct = this.as_ref().map(|ref t| {
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
        let symbol = match arg {
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
            Type::PrimitiveType(PrimitiveType::UInt64) => todo!(),
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
            Type::PublicKey => compile_function_call(
                compiler,
                BUILTINS_SCOPE.find_function("readAdvicePublicKey").unwrap(),
                &[],
                None,
            ),
            Type::Struct(struct_) => {
                let symbol = compiler.memory.allocate_symbol(arg.clone());
                read_struct_from_advice_tape(compiler, &symbol, struct_);
                symbol
            }
            x => unimplemented!("{:?}", x),
        };

        args_symbols.push(symbol);
    }

    (this, args_symbols)
}

fn prepare_scope(program: &ast::Program) -> Scope {
    let mut scope = Scope::new();

    for func in USABLE_BUILTINS.iter() {
        scope.add_function(func.0.clone(), func.1.clone());
    }

    for node in &program.nodes {
        match node {
            ast::RootNode::Collection(c) => {
                let mut collection = Collection {
                    name: c.name.clone(),
                    functions: vec![],
                    fields: vec![],
                };

                for item in &c.items {
                    match item {
                        ast::CollectionItem::Field(f) => {
                            collection.fields.push((
                                f.name.clone(),
                                match &f.type_ {
                                    ast::Type::String => Type::String,
                                    ast::Type::Number => Type::PrimitiveType(PrimitiveType::UInt32),
                                    ast::Type::Boolean => {
                                        Type::PrimitiveType(PrimitiveType::Boolean)
                                    }
                                    ast::Type::Bytes => Type::Bytes,
                                    ast::Type::PublicKey => Type::PublicKey,
                                    ast::Type::ForeignRecord { collection } => {
                                        Type::CollectionReference {
                                            collection: collection.clone(),
                                        }
                                    }
                                    ast::Type::Array(_) => {
                                        todo!("Array fields are not implemented")
                                    }
                                    ast::Type::Map(_, _) => todo!("Map fields are not implemented"),
                                    ast::Type::Object(_) => todo!(),
                                },
                            ));
                        }
                        ast::CollectionItem::Function(f) => {
                            collection.functions.push((f.name.clone(), &f));
                        }
                        ast::CollectionItem::Index(_) => todo!(),
                    }
                }

                scope.add_collection(collection.name.clone(), collection);
            }
            ast::RootNode::Function(function) => scope
                .functions
                .push((function.name.clone(), Function::AST(function))),
        }
    }

    scope
}

pub enum CompileTimeArg {
    U32(u32),
    Record(HashMap<String, u32>),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Abi {
    pub out_this_addr: Option<u32>,
    pub out_this_type: Option<Type>,
}

pub fn compile(
    program: ast::Program,
    collection_name: Option<&str>,
    function_name: &str,
) -> (String, Abi) {
    let scope = prepare_scope(&program);
    let collection = collection_name.map(|name| scope.find_collection(name).unwrap());
    let collection_struct = collection.map(|collection| Struct {
        name: collection.name.clone(),
        fields: collection
            .fields
            .iter()
            .map(|(name, field)| (name.clone(), field.clone()))
            .collect(),
    });
    let function = collection
        .and_then(|c| {
            c.functions
                .iter()
                .find(|(name, _)| name == function_name)
                .map(|(_, f)| *f)
        })
        .or_else(|| match scope.find_function(function_name) {
            Some(Function::AST(f)) => Some(f),
            Some(Function::Builtin(_)) => todo!(),
            None => None,
        })
        .unwrap();

    let mut instructions = vec![];
    let mut memory = Memory::new();
    let this_addr;

    {
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);

        let expected_hash = collection_struct.as_ref().map(|_| {
            let hash = compiler.memory.allocate_symbol(Type::Hash);
            compiler.memory.write(
                &mut compiler.instructions,
                hash.memory_addr,
                &vec![ValueSource::Stack; hash.type_.miden_width() as _],
            );
            hash
        });

        let (this_symbol, arg_symbols) = read_collection_inputs(
            &mut compiler,
            collection_struct.clone(),
            &function
                .parameters
                .iter()
                .map(|p| match &p.type_ {
                    ast::ParameterType::String => Type::String,
                    ast::ParameterType::Number => Type::PrimitiveType(PrimitiveType::UInt32),
                    ast::ParameterType::Record => Type::Struct(collection_struct.clone().unwrap()),
                    ast::ParameterType::PublicKey => Type::PublicKey,
                    ast::ParameterType::Bytes => Type::Bytes,
                    ast::ParameterType::ForeignRecord { collection } => Type::CollectionReference {
                        collection: collection.clone(),
                    },
                    ast::ParameterType::Boolean => todo!(),
                    ast::ParameterType::Array(_) => todo!(),
                    ast::ParameterType::Map(_, _) => todo!(),
                    ast::ParameterType::Object(_) => todo!(),
                })
                .collect::<Vec<_>>(),
        );

        this_addr = this_symbol.as_ref().map(|ts| ts.memory_addr);

        if let Some(this_symbol) = &this_symbol {
            // let this_hash = hash(&mut compiler, this_symbol.clone());
            // compiler.memory.read(
            //     &mut compiler.instructions,
            //     this_hash.memory_addr,
            //     this_hash.type_.miden_width(),
            // );
            // let is_eq = compile_eq(&mut compiler, &this_hash, expected_hash.as_ref().unwrap());
            // let assert_fn = compiler.root_scope.find_function("assert").unwrap();
            // let error_str = string::new(
            //     &mut compiler,
            //     "Hash of this does not match the expected hash",
            // );
            // compile_function_call(&mut compiler, assert_fn, &[is_eq, error_str], None);
        }

        let result =
            compile_ast_function_call(function, &mut compiler, &arg_symbols, this_symbol.clone());

        comment!(compiler, "Reading result from memory");
        compiler.memory.read(
            &mut compiler.instructions,
            result.memory_addr,
            result.type_.miden_width(),
        );

        if let Some(this_symbol) = this_symbol {
            let this_hash = hash(&mut compiler, this_symbol);
            compiler.memory.read(
                &mut compiler.instructions,
                this_hash.memory_addr,
                this_hash.type_.miden_width(),
            );
        }
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
        miden_code.push_str("\n");
    }
    miden_code.push_str("end\n");

    (
        miden_code,
        Abi {
            out_this_addr: this_addr,
            out_this_type: collection_struct.map(|s| Type::Struct(s)),
        },
    )
}

/// A function that takes in a struct type and generates a program that hashes a value of that type and returns the hash on the stack.
pub fn compile_struct_hasher(struct_: Struct) -> String {
    let mut instructions = vec![];
    let mut memory = Memory::new();
    let empty_program = ast::Program { nodes: vec![] };
    let scope = prepare_scope(&empty_program);

    {
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);

        let (this_symbol, _) = read_collection_inputs(&mut compiler, Some(struct_), &[]);

        let hash = hash(&mut compiler, this_symbol.unwrap());

        comment!(compiler, "Reading result from memory");
        compiler.memory.read(
            &mut compiler.instructions,
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
        miden_code.push_str("\n");
    }
    miden_code.push_str("end\n");

    miden_code
}
