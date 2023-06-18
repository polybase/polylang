use super::*;

pub(crate) const WIDTH: u32 = 1;

pub(crate) fn new(compiler: &mut Compiler, value: u32) -> Symbol {
    let symbol = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    // memory is zero-initialized, so we don't need to write for 0
    if value > 0 {
        compiler.memory.write(
            compiler.instructions,
            symbol.memory_addr,
            &[ValueSource::Immediate(value)],
        );
    }

    symbol
}

pub(crate) fn add(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn sub(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSub);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn gte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedGTE);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn gt(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedGT);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn lte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedLTE);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn lt(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedLT);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn modulo(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedMod(None));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn div(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedDiv(None));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn mul(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedMul);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn shift_left(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    // TODO: SHL with Some is an order of magnitude faster, optimize this for constants
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHL(None));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn shift_right(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    // TODO: SHR with Some is an order of magnitude faster, optimize this for constants
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHR(None));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

/// Finds the most significant bit and returns it's position.
/// Stack transition:
/// [number] => [msb]
pub(crate) fn find_msb(instructions: &mut Vec<encoder::Instruction>) {
    instructions.push(encoder::Instruction::Push(0));
    instructions.push(encoder::Instruction::Swap);
    // [number, position = 0]

    for i in [16, 8, 4, 2, 1] {
        instructions.push(encoder::Instruction::If {
            condition: vec![
                // [number, position]
                encoder::Instruction::Dup(None),
                // [number, number, position]
                encoder::Instruction::Push(1 << i),
                // [1 << i, number, number, position]
                encoder::Instruction::U32CheckedGTE,
                // [number >= 1 << i, number, position]
            ],
            then: if i > 1 {
                vec![
                    // [number, position]
                    encoder::Instruction::U32CheckedSHR(Some(i)),
                    // [number >> i, position]
                ]
            } else {
                vec![]
            }
            .into_iter()
            .chain([
                encoder::Instruction::Swap,
                // [position, number]
                encoder::Instruction::Push(i),
                encoder::Instruction::U32CheckedAdd,
                // [position + i, number]
                encoder::Instruction::Swap,
            ])
            .collect(),
            else_: vec![],
        });
    }

    // [number, position]
    instructions.push(encoder::Instruction::Drop);
    // [position]
}
