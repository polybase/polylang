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
        .push(encoder::Instruction::U32CheckedMod);
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
        .push(encoder::Instruction::U32CheckedDiv);
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
