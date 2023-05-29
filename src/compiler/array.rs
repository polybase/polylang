use super::*;

/// [capacity, length, data_ptr]
pub(crate) const WIDTH: u32 = 3;

/// Returns (array_symbol, data_ptr), because data_ptr is known statically
pub(crate) fn new(compiler: &mut Compiler, len: u32, element_type: Type) -> (Symbol, u32) {
    let symbol = Symbol {
        memory_addr: compiler.memory.allocate(WIDTH),
        type_: Type::Array(Box::new(element_type)),
        
    };

    let symbol_capacity = capacity(&symbol);
    let symbol_length = length(&symbol);
    let symbol_data_ptr = data_ptr(&symbol);

    let capacity = len * 2;
    compiler.memory.write(
        compiler.instructions,
        symbol_capacity.memory_addr,
        &[ValueSource::Immediate(capacity)],
    );

    compiler.memory.write(
        compiler.instructions,
        symbol_length.memory_addr,
        &[ValueSource::Immediate(len)],
    );

    let allocated_ptr = compiler.memory.allocate(len * 2);

    compiler.memory.write(
        compiler.instructions,
        symbol_data_ptr.memory_addr,
        &[ValueSource::Immediate(allocated_ptr)],
    );

    (symbol, allocated_ptr)
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
