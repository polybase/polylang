use super::*;

pub(crate) const WIDTH: u32 = 1;

pub(crate) fn new(compiler: &mut Compiler, value: bool) -> Symbol {
    let symbol = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    // memory is zero-initialized, so we don't need to write for false
    if value {
        compiler.memory.write(
            &mut compiler.instructions,
            symbol.memory_addr,
            &[ValueSource::Immediate(1)],
        );
    }

    symbol
}
