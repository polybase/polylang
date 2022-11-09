use super::*;

pub(crate) const WIDTH: u32 = 1;

pub(crate) fn new(compiler: &mut Compiler, value: u32) -> Symbol {
    let symbol = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    // memory is zero-initialized, so we don't need to write for 0
    if value > 0 {
        compiler.memory.write(
            &mut compiler.instructions,
            symbol.memory_addr,
            &[ValueSource::Immediate(value as u32)],
        );
    }

    symbol
}

pub(crate) fn add(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
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
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn sub(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
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
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSub);
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
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
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn gte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
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
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedGTE);
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn lte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
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
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedLTE);
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn lt(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
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
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedLT);
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn modulo(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
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
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedMod);
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn div(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
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
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedDiv);
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}
