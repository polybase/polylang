use super::*;

pub(crate) const WIDTH: u32 = 1;

pub(crate) fn new(compiler: &mut Compiler, value: bool) -> Symbol {
    let symbol = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    // memory is zero-initialized, so we don't need to write for false
    if value {
        compiler.memory.write(
            compiler.instructions,
            symbol.memory_addr,
            &[ValueSource::Immediate(1)],
        );
    }

    symbol
}

pub(crate) fn compile_and(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    assert_eq!(a.type_, b.type_);
    assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::Boolean));

    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler.instructions.push(encoder::Instruction::And);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn compile_or(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    assert_eq!(a.type_, b.type_);
    assert_eq!(a.type_, Type::PrimitiveType(PrimitiveType::Boolean));

    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    compiler.instructions.push(encoder::Instruction::Or);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}
