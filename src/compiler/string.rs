use super::*;

pub(crate) const WIDTH: u32 = 2;

pub(crate) fn new(compiler: &mut Compiler, value: &str) -> Symbol {
    let symbol = compiler.memory.allocate_symbol(Type::String);

    if value != "" {
        let string_addr = compiler.memory.allocate(value.len() as u32);

        compiler.memory.write(
            &mut compiler.instructions,
            symbol.memory_addr,
            &[
                ValueSource::Immediate(value.len() as u32),
                ValueSource::Immediate(string_addr),
            ],
        );

        compiler.memory.write(
            &mut compiler.instructions,
            string_addr,
            &value
                .bytes()
                .map(|c| ValueSource::Immediate(c as u32))
                .collect::<Vec<_>>(),
        );
    }

    symbol
}

pub(crate) fn length(string: &Symbol) -> Symbol {
    Symbol {
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
        memory_addr: string.memory_addr,
    }
}

pub(crate) fn data_ptr(string: &Symbol) -> Symbol {
    Symbol {
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
        memory_addr: string.memory_addr + 1,
    }
}
