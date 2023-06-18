use crate::compiler::encoder::Instruction;

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

pub(crate) fn hash(compiler: &mut Compiler, _scope: &mut Scope, args: &[Symbol]) -> Symbol {
    assert_eq!(args.len(), 1);
    assert!(matches!(args[0].type_, Type::Array(_)));
    let arr = &args[0];

    let inner_type = match args[0].type_ {
        Type::Array(ref inner_type) => inner_type,
        _ => unreachable!(),
    };

    let (inner_hashing_input, inner_hashing_insts, inner_hashing_output) = {
        let mut insts = Vec::new();

        std::mem::swap(compiler.instructions, &mut insts);
        let input = compiler.memory.allocate_symbol(*inner_type.clone());
        let output = super::hash(compiler, input.clone());
        std::mem::swap(compiler.instructions, &mut insts);

        (input, insts, output)
    };

    let result = compiler.memory.allocate_symbol(Type::Hash);

    compiler.instructions.extend([
        Instruction::Push(0),
        Instruction::Push(0),
        Instruction::Push(0),
        Instruction::Push(0),
        // [h[3], h[2], h[1], h[0]]
        Instruction::Push(0),
        // [i = 0]
        Instruction::While {
            condition: vec![
                Instruction::Dup(None),
                // [i, i]
                Instruction::MemLoad(Some(length(&arr).memory_addr)),
                // [len, i, i]
                Instruction::U32CheckedLT,
                // [i < len, i]
            ],
            body: vec![
                Instruction::Dup(None),
                // [i, i]
                Instruction::Push(inner_type.miden_width()),
                // [inner_width, i, i]
                Instruction::U32CheckedMul,
                // [offset = i * inner_width, i]
                Instruction::MemLoad(Some(data_ptr(&arr).memory_addr)),
                // [data_ptr, offset, i]
                Instruction::U32CheckedAdd,
                // [ptr = data_ptr + offset, i]
            ]
            .into_iter()
            .chain({
                let mut insts = vec![];

                for y in 0..inner_type.miden_width() {
                    insts.extend([
                        Instruction::Dup(None),
                        // [ptr, ptr, i]
                        Instruction::Push(y),
                        // [y, ptr, ptr, i]
                        Instruction::U32CheckedAdd,
                        // [ptr + y, ptr, i]
                        Instruction::MemLoad(None),
                        // [value, ptr, i]
                        Instruction::MemStore(Some(inner_hashing_input.memory_addr + y)),
                        // [ptr, i]
                    ]);
                }

                insts.push(Instruction::Drop);
                // [i]

                insts.into_iter()
            })
            .chain(inner_hashing_insts)
            .chain([
                Instruction::MemLoad(Some(inner_hashing_output.memory_addr)),
                Instruction::MemLoad(Some(inner_hashing_output.memory_addr + 1)),
                Instruction::MemLoad(Some(inner_hashing_output.memory_addr + 2)),
                Instruction::MemLoad(Some(inner_hashing_output.memory_addr + 3)),
                // [h[3], h[2], h[1], h[0], i, h[3], h[2], h[1], h[0]]
                Instruction::MovUp(4),
                // [i...]
                Instruction::MovDown(8),
                // [..., i]
                Instruction::HMerge,
                // [h[3], h[2], h[1], h[0], i]
                Instruction::MovUp(4),
                // [i]
                Instruction::Push(1),
                // [1, i]
                Instruction::U32CheckedAdd,
                // [i = i + 1]
            ])
            .collect(),
        },
        // [i, h[3], h[2], h[1], h[0]]
        Instruction::Drop,
        // [h[3], h[2], h[1], h[0]]
        Instruction::MemStore(Some(result.memory_addr)),
        Instruction::MemStore(Some(result.memory_addr + 1)),
        Instruction::MemStore(Some(result.memory_addr + 2)),
        Instruction::MemStore(Some(result.memory_addr + 3)),
    ]);

    result
}
