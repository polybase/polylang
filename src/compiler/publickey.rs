use super::{encoder::Instruction, *};
use publickey::Key;

// Layout: [key, crv, alg, use, extra_ptr]
// `extra_ptr` in secp256k1 is pointer to 64 bytes of data,
// the x and y coordinates of the public key.
#[allow(unused)]
pub(crate) fn new(compiler: &mut Compiler, key: Key) -> Symbol {
    let symbol = compiler.memory.allocate_symbol(Type::PublicKey);
    let ptr_xy = compiler.memory.allocate(64);

    compiler.memory.write(
        compiler.instructions,
        symbol.memory_addr,
        &[
            ValueSource::Immediate(u8::from(key.kty) as u32),
            ValueSource::Immediate(u8::from(key.crv) as u32),
            ValueSource::Immediate(u8::from(key.alg) as u32),
            ValueSource::Immediate(u8::from(key.use_) as u32),
            ValueSource::Immediate(ptr_xy),
        ],
    );

    compiler.memory.write(
        compiler.instructions,
        ptr_xy,
        &key.x
            .iter()
            .chain(key.y.iter())
            .map(|c| ValueSource::Immediate(*c as u32))
            .collect::<Vec<_>>(),
    );

    symbol
}

pub(crate) fn kty(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
    }
}

pub(crate) fn crv(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 1,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
    }
}

pub(crate) fn alg(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 2,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
    }
}

pub(crate) fn use_(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 3,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
    }
}

pub(crate) fn extra_ptr(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 4,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
    }
}

pub(crate) fn eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let symbol = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    compiler.instructions.push(Instruction::Push(1));
    for i in 0..4 {
        compiler
            .memory
            .read(compiler.instructions, a.memory_addr + i, 1);

        compiler
            .memory
            .read(compiler.instructions, b.memory_addr + i, 1);

        compiler.instructions.push(Instruction::Eq);
        compiler.instructions.push(Instruction::And);
    }

    for i in 0..64 {
        compiler
            .memory
            .read(compiler.instructions, a.memory_addr + 4, 1);
        compiler.instructions.push(Instruction::Push(i as u32));
        compiler.instructions.push(Instruction::Add);
        compiler.instructions.push(Instruction::MemLoad(None));

        compiler
            .memory
            .read(compiler.instructions, b.memory_addr + 4, 1);
        compiler.instructions.push(Instruction::Push(i as u32));
        compiler.instructions.push(Instruction::Add);
        compiler.instructions.push(Instruction::MemLoad(None));

        compiler.instructions.push(Instruction::Eq);
        compiler.instructions.push(Instruction::And);
    }

    compiler.memory.write(
        compiler.instructions,
        symbol.memory_addr,
        &[ValueSource::Stack],
    );

    symbol
}

pub(crate) fn to_hex(compiler: &mut Compiler, args: &[Symbol]) -> Symbol {
    let mut initial_result_str = String::new();
    initial_result_str.push_str("0x");
    for _ in 0..64 {
        initial_result_str.push_str("00");
    }

    let (result, result_data) = string::new(compiler, &initial_result_str);
    let result_data = result_data.unwrap();

    let [pk] = args else {
        panic!("invalid args: {args:?}, expected [publicKey]");
    };

    assert_eq!(pk.type_, Type::PublicKey);

    compiler
        .memory
        .read(compiler.instructions, extra_ptr(pk).memory_addr, 1);
    // [extra_ptr]
    for i in 0..64 {
        let pos = 2 + i * 2;

        compiler.instructions.extend([
            Instruction::Push(i),
            // [i, extra_ptr]
            Instruction::Dup(Some(1)),
            // [extra_ptr, i, extra_ptr]
            Instruction::U32CheckedAdd,
            // [i + extra_ptr, extra_ptr]
            Instruction::MemLoad(None),
            // [extra_ptr[i], extra_ptr]

            // Do the first hex character
            Instruction::Dup(None),
            // [extra_ptr[i], extra_ptr[i], extra_ptr]
            Instruction::U32CheckedDiv(Some(16)),
            // [first_digit = extra_ptr[i] / 16, extra_ptr[i], extra_ptr]
            Instruction::Push(b'a' as u32 - 10),
            Instruction::Push(48),
            // [48, 87, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Dup(Some(2)),
            // [extra_ptr[i], 48, 87, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Push(10),
            Instruction::U32CheckedLT,
            // [extra_ptr[i] < 10, 48, 87, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Cswap,
            // [87 if true, 48 if true, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Drop,
            // [48 if true else 87, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Swap,
            // [first_digit, 48 if true else 87, extra_ptr[i], extra_ptr]
            Instruction::U32CheckedAdd,
            // [first_digit + delta, extra_ptr[i], extra_ptr]
            Instruction::MemStore(Some(result_data + pos)),
            // [extra_ptr[i], extra_ptr]

            // Second hex character
            Instruction::Dup(None),
            // [extra_ptr[i], extra_ptr[i], extra_ptr]
            Instruction::U32CheckedMod(Some(16)),
            // [second_digit = extra_ptr[i] % 16, extra_ptr[i], extra_ptr]
            Instruction::Push(b'a' as u32 - 10),
            Instruction::Push(48),
            // [48, 87, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Dup(Some(2)),
            // [extra_ptr[i], 48, 87, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Push(10),
            Instruction::U32CheckedLT,
            // [extra_ptr[i] < 10, 48, 87, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Cswap,
            // [87 if true, 48 if true, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Drop,
            // [48 if true else 87, first_digit, extra_ptr[i], extra_ptr]
            Instruction::Swap,
            // [first_digit, 48 if true else 87, extra_ptr[i], extra_ptr]
            Instruction::U32CheckedAdd,
            // [first_digit + delta, extra_ptr[i], extra_ptr]
            Instruction::MemStore(Some(result_data + pos + 1)),
            // [extra_ptr[i], extra_ptr]

            // Done
            Instruction::Drop,
        ]);
    }

    result
}

pub(crate) fn hash(compiler: &mut Compiler, args: &[Symbol]) -> Symbol {
    let public_key = args.get(0).unwrap();
    assert_eq!(public_key.type_, Type::PublicKey);

    let result = compiler.memory.allocate_symbol(Type::Hash);

    compiler.instructions.extend([
        encoder::Instruction::Push(0),
        encoder::Instruction::Push(0),
        encoder::Instruction::Push(0),
        encoder::Instruction::Push(0),
    ]);
    // [h[3], h[2], h[1], h[0]]

    compiler
        .memory
        .read(compiler.instructions, kty(public_key).memory_addr, 1);
    compiler
        .memory
        .read(compiler.instructions, crv(public_key).memory_addr, 1);
    compiler
        .memory
        .read(compiler.instructions, alg(public_key).memory_addr, 1);
    compiler
        .memory
        .read(compiler.instructions, use_(public_key).memory_addr, 1);

    // [use, alg, crv, kty, h[3], h[2], h[1], h[0]
    compiler.instructions.push(encoder::Instruction::HMerge);

    // We hashed kty, crv, alg, use. Now we need to hash the x and y coordinates.
    let extra_ptr = publickey::extra_ptr(public_key);
    // x
    for i in (0..32).step_by(4) {
        // [h[3], h[2], h[1], h[0]]
        for y in 0..4 {
            compiler
                .memory
                .read(compiler.instructions, extra_ptr.memory_addr, 1);
            compiler
                .instructions
                .push(encoder::Instruction::Push(i + y));
            compiler
                .instructions
                .push(encoder::Instruction::U32CheckedAdd);
            compiler
                .instructions
                .push(encoder::Instruction::MemLoad(None));
        }
        compiler.instructions.push(encoder::Instruction::HMerge);
    }

    // y
    for i in (32..64).step_by(4) {
        // [h[3], h[2], h[1], h[0]]
        for y in 0..4 {
            compiler
                .memory
                .read(compiler.instructions, extra_ptr.memory_addr, 1);
            compiler
                .instructions
                .push(encoder::Instruction::Push(i + y));
            compiler
                .instructions
                .push(encoder::Instruction::U32CheckedAdd);
            compiler
                .instructions
                .push(encoder::Instruction::MemLoad(None));
        }
        compiler.instructions.push(encoder::Instruction::HMerge);
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
