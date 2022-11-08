use super::*;

pub(crate) const WIDTH: u32 = 2;

pub(crate) fn cast_from_uint32(compiler: &mut Compiler, from: &Symbol, dest: &Symbol) {
    assert_eq!(from.type_, Type::PrimitiveType(PrimitiveType::UInt32));
    assert_eq!(dest.type_, Type::PrimitiveType(PrimitiveType::UInt64));

    compiler.memory.read(
        &mut compiler.instructions,
        from.memory_addr,
        from.type_.miden_width(),
    );
    compiler.memory.write(
        &mut compiler.instructions,
        dest.memory_addr,
        &[ValueSource::Immediate(0), ValueSource::Stack],
    );
}

pub(crate) fn add(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

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
        .push(encoder::Instruction::Exec("u64::checked_add"));
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack, ValueSource::Stack],
    );

    result
}

pub(crate) fn sub(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

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
        .push(encoder::Instruction::Exec("u64::checked_sub"));
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack, ValueSource::Stack],
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
        .push(encoder::Instruction::Exec("u64::checked_eq"));
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
        .push(encoder::Instruction::Exec("u64::checked_gte"));
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
        .push(encoder::Instruction::Exec("u64::checked_lte"));
    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}
