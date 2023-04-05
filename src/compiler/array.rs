use super::*;

/// [capacity, length, data_ptr]
pub(crate) const WIDTH: u32 = 3;

pub(crate) fn new(compiler: &mut Compiler, bytes: &[u8]) -> Symbol {
    let symbol = Symbol {
        memory_addr: compiler.memory.allocate(WIDTH),
        type_: Type::Bytes,
    };

    let symbol_capacity = capacity(&symbol);
    let symbol_length = length(&symbol);
    let symbol_data_ptr = data_ptr(&symbol);

    compiler.memory.write(
        &mut compiler.instructions,
        symbol_capacity.memory_addr,
        &[ValueSource::Immediate(bytes.len() as u32)],
    );

    compiler.memory.write(
        &mut compiler.instructions,
        symbol_length.memory_addr,
        &[ValueSource::Immediate(bytes.len() as u32)],
    );

    let allocated_ptr = dynamic_alloc(compiler, &[symbol_length]);

    compiler.memory.write(
        &mut compiler.instructions,
        symbol_data_ptr.memory_addr,
        &[ValueSource::Memory(allocated_ptr.memory_addr)],
    );

    compiler.memory.read(
        &mut compiler.instructions,
        symbol_data_ptr.memory_addr,
        symbol_data_ptr.type_.miden_width(),
    );
    // [symbol_data_ptr]

    compiler.memory.write(
        &mut compiler.instructions,
        allocated_ptr.memory_addr,
        &bytes
            .iter()
            .map(|b| ValueSource::Immediate(*b as u32))
            .collect::<Vec<_>>(),
    );

    symbol
}

pub(crate) fn capacity(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
    }
}

pub(crate) fn length(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 1,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
    }
}

pub(crate) fn data_ptr(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 2,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
    }
}
