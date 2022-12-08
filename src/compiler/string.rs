use crate::validation::Value;

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

/// Expects the stack to be: [len, src_ptr, dest_ptr]
fn copy_str_stack(compiler: &mut Compiler) {
    // [len, src_ptr, dest_ptr]
    compiler.instructions.push(encoder::Instruction::While {
        // len > 0
        condition: vec![
            encoder::Instruction::Dup(None),
            // [len, len, src_ptr, dest_ptr]
            encoder::Instruction::Push(0),
            // [0, len, len, src_ptr, dest_ptr]
            encoder::Instruction::U32CheckedGT,
            // [len > 0, len, src_ptr, dest_ptr]
        ],
        // len--; *dest_ptr = *src_ptr; src_ptr++; dest_ptr++;
        body: vec![
            // [len, src_ptr, dest_ptr]
            encoder::Instruction::Push(1),
            // [1, len, src_ptr, dest_ptr]
            encoder::Instruction::U32CheckedSub,
            // [len - 1, src_ptr, dest_ptr]
            encoder::Instruction::MovDown(2),
            // [src_ptr, dest_ptr, len - 1]
            encoder::Instruction::Dup(None),
            // [src_ptr, src_ptr, dest_ptr, len - 1]
            encoder::Instruction::MemLoad(None),
            // [*src_ptr, src_ptr, dest_ptr, len - 1]
            encoder::Instruction::Dup(Some(2)),
            // [dest_ptr, *src_ptr, src_ptr, dest_ptr, len - 1]
            encoder::Instruction::MemStore(None),
            // [src_ptr, dest_ptr, len - 1]
            encoder::Instruction::Push(1),
            // [1, src_ptr, dest_ptr, len - 1]
            encoder::Instruction::U32CheckedAdd,
            // [src_ptr + 1, dest_ptr, len - 1]
            encoder::Instruction::MovDown(2),
            // [dest_ptr, len - 1, src_ptr + 1]
            encoder::Instruction::Push(1),
            // [1, dest_ptr, len - 1, src_ptr + 1]
            encoder::Instruction::U32CheckedAdd,
            // [dest_ptr + 1, len - 1, src_ptr + 1]
            encoder::Instruction::MovDown(2),
            // [len - 1, src_ptr + 1, dest_ptr + 1]
        ],
    });

    // [len, src_ptr, dest_ptr]
    compiler.instructions.push(encoder::Instruction::Drop);
    // [src_ptr, dest_ptr]
    compiler.instructions.push(encoder::Instruction::Drop);
    // [dest_ptr]
    compiler.instructions.push(encoder::Instruction::Drop);
    // []
}

pub(crate) fn concat(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = new(compiler, "");
    let result_data_ptr = data_ptr(&result);
    let result_len = length(&result);

    let a_len = length(a);
    let a_data_ptr = data_ptr(a);

    let b_len = length(b);
    let b_data_ptr = data_ptr(b);

    // Set the length of the result
    compiler.memory.read(
        &mut compiler.instructions,
        a_len.memory_addr,
        a_len.type_.miden_width(),
    );
    // [a_len]

    compiler.memory.read(
        &mut compiler.instructions,
        b_len.memory_addr,
        b_len.type_.miden_width(),
    );
    // [b_len, a_len]

    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);
    // [a_len + b_len]

    compiler.memory.write(
        &mut compiler.instructions,
        result_len.memory_addr,
        &[ValueSource::Stack],
    );

    let allocated_ptr = dynamic_alloc(compiler, &[result_len]);

    compiler.memory.write(
        &mut compiler.instructions,
        result_data_ptr.memory_addr,
        &[ValueSource::Memory(allocated_ptr.memory_addr)],
    );

    compiler.memory.read(
        &mut compiler.instructions,
        result_data_ptr.memory_addr,
        result_data_ptr.type_.miden_width(),
    );
    // [result_data_ptr]

    compiler.memory.read(
        &mut compiler.instructions,
        a_data_ptr.memory_addr,
        a_data_ptr.type_.miden_width(),
    );
    // [a_data_ptr, result_data_ptr]

    compiler.memory.read(
        &mut compiler.instructions,
        a_len.memory_addr,
        a_len.type_.miden_width(),
    );
    // [a_len, a_data_ptr, result_data_ptr]

    copy_str_stack(compiler);
    // []

    compiler.memory.read(
        &mut compiler.instructions,
        result_data_ptr.memory_addr,
        result_data_ptr.type_.miden_width(),
    );
    // [result_data_ptr]

    compiler.memory.read(
        &mut compiler.instructions,
        a_len.memory_addr,
        a_len.type_.miden_width(),
    );
    // [a_len, result_data_ptr]

    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);
    // [result_data_ptr + a_len]

    compiler.memory.read(
        &mut compiler.instructions,
        b_data_ptr.memory_addr,
        b_data_ptr.type_.miden_width(),
    );
    // [b_data_ptr, result_data_ptr + a_len]

    compiler.memory.read(
        &mut compiler.instructions,
        b_len.memory_addr,
        b_len.type_.miden_width(),
    );
    // [b_len, b_data_ptr, result_data_ptr + a_len]

    copy_str_stack(compiler);
    // []

    result
}
