use super::*;

// Layout: [high, low]
pub(crate) fn cast_from_uint32(compiler: &mut Compiler, from: &Symbol, dest: &Symbol) {
    assert_eq!(from.type_, Type::PrimitiveType(PrimitiveType::UInt32));
    assert_eq!(dest.type_, Type::PrimitiveType(PrimitiveType::UInt64));

    compiler.memory.read(
        compiler.instructions,
        from.memory_addr,
        from.type_.miden_width(),
    );
    compiler.memory.write(
        compiler.instructions,
        dest.memory_addr,
        &[ValueSource::Immediate(0), ValueSource::Stack],
    );
}

pub(crate) fn add(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::Exec("u64::checked_add"));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack, ValueSource::Stack],
    );

    result
}

pub(crate) fn sub(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::Exec("u64::checked_sub"));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack, ValueSource::Stack],
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
        .push(encoder::Instruction::Exec("u64::checked_eq"));
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
        .push(encoder::Instruction::Exec("u64::checked_gte"));
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
        .push(encoder::Instruction::Exec("u64::checked_gt"));
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
        .push(encoder::Instruction::Exec("u64::checked_lte"));
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
        .push(encoder::Instruction::Exec("u64::checked_lt"));
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
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::Exec("u64::checked_mod"));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack, ValueSource::Stack],
    );

    result
}

pub(crate) fn div(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::Exec("u64::checked_div"));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack, ValueSource::Stack],
    );

    result
}

pub(crate) fn mul(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::Exec("u64::checked_mul"));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack, ValueSource::Stack],
    );

    result
}

pub(crate) fn shift_left(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::Exec("u64::checked_shl"));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack, ValueSource::Stack],
    );

    result
}

pub(crate) fn shift_right(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));

    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler
        .instructions
        .push(encoder::Instruction::Exec("u64::checked_shr"));
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack, ValueSource::Stack],
    );

    result
}
