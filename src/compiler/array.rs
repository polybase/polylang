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

pub(crate) fn element_type(type_: &Type) -> &Type {
    match type_ {
        Type::Array(inner_type) => inner_type,
        _ => unreachable!(),
    }
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

pub(crate) fn hash(compiler: &mut Compiler, _scope: &mut Scope, args: &[Symbol]) -> Result<Symbol> {
    ensure!(
        args.len() == 1,
        ArgumentsCountSnafu {
            found: args.len(),
            expected: 1usize
        }
    );
    let arr = &args[0];
    ensure_eq_type!(arr, Type::Array(_));

    let Type::Array(inner_type) = &arr.type_ else {
        unreachable!()
    };

    let (inner_hashing_input, inner_hashing_insts, inner_hashing_output) = {
        let mut insts = Vec::new();

        std::mem::swap(compiler.instructions, &mut insts);
        let input = compiler.memory.allocate_symbol(*inner_type.clone());
        let output = super::hash(compiler, input.clone())?;
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
                Instruction::MemLoad(Some(length(arr).memory_addr)),
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
                Instruction::MemLoad(Some(data_ptr(arr).memory_addr)),
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

    Ok(result)
}

pub(crate) fn get(compiler: &mut Compiler, arr: &Symbol, index: &Symbol) -> Symbol {
    assert!(matches!(arr.type_, Type::Array(_)));
    assert!(matches!(
        index.type_,
        Type::PrimitiveType(PrimitiveType::UInt32)
    ));

    let result = compiler
        .memory
        .allocate_symbol(element_type(&arr.type_).clone());

    compiler.instructions.extend([
        Instruction::MemLoad(Some(data_ptr(&arr).memory_addr)),
        // [data_ptr]
        Instruction::MemLoad(Some(index.memory_addr)),
        // [index, data_ptr]
        Instruction::Push(element_type(&arr.type_).miden_width()),
        // [element_width, index, data_ptr]
        Instruction::U32CheckedMul,
        // [offset = index * element_width, data_ptr]
        Instruction::U32CheckedAdd,
        // [ptr = data_ptr + offset]
    ]);

    for i in 0..element_type(&arr.type_).miden_width() {
        compiler.instructions.extend([
            Instruction::Dup(None),
            // [ptr, ptr]
            Instruction::Push(i),
            // [i, ptr, ptr]
            Instruction::U32CheckedAdd,
            // [ptr + i, ptr]
            Instruction::MemLoad(None),
            // [value, ptr]
            Instruction::MemStore(Some(result.memory_addr + i)),
            // [ptr]
        ]);
    }

    compiler.instructions.push(Instruction::Drop);

    result
}

pub(crate) fn find_index(compiler: &mut Compiler, arr: &Symbol, el: &Symbol) -> Result<Symbol> {
    ensure_eq_type!(arr, Type::Array(_));
    let element_type = element_type(&arr.type_);
    ensure_eq_type!(el, @element_type);

    let result = int32::new(compiler, -1);

    let current_arr_element = compiler.memory.allocate_symbol(element_type.clone());
    let (eq_insts, eq_result) = {
        let mut insts = Vec::new();

        std::mem::swap(compiler.instructions, &mut insts);
        let result = super::compile_eq(compiler, el, &current_arr_element);
        std::mem::swap(compiler.instructions, &mut insts);

        (insts, result)
    };

    let current_index = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    let finished = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    iterate_array_elements(
        compiler,
        arr,
        &current_index,
        &current_arr_element,
        &finished,
        eq_insts
            .into_iter()
            .chain([Instruction::If {
                condition: vec![Instruction::MemLoad(Some(eq_result.memory_addr))],
                then: vec![
                    Instruction::MemLoad(Some(current_index.memory_addr)),
                    Instruction::MemStore(Some(result.memory_addr)),
                    Instruction::Push(1),
                    Instruction::MemStore(Some(finished.memory_addr)),
                ],
                else_: vec![],
            }])
            .collect(),
    )?;

    Ok(result)
}

pub(crate) fn push(compiler: &mut Compiler, _scope: &Scope, args: &[Symbol]) -> Result<Symbol> {
    ensure!(
        args.len() == 2,
        ArgumentsCountSnafu {
            found: args.len(),
            expected: 2usize
        }
    );
    let arr = args.get(0).unwrap();
    let element = args.get(1).unwrap();
    ensure_eq_type!(
        @arr.type_.clone(),
        @Type::Array(Box::new(element.type_.clone()))
    );

    compiler
        .memory
        .read(compiler.instructions, array::length(arr).memory_addr, 1);
    // [len]
    compiler.instructions.push(encoder::Instruction::Push(1));
    // [1, len]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);
    // [len + 1]
    compiler.memory.write(
        compiler.instructions,
        array::length(arr).memory_addr,
        &[ValueSource::Stack],
    );
    // []

    grow(compiler, arr, &array::length(arr))?;

    compiler
        .memory
        .read(compiler.instructions, array::capacity(arr).memory_addr, 1);
    // [capacity]
    compiler
        .memory
        .read(compiler.instructions, array::length(arr).memory_addr, 1);
    // [len + 1, capacity]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedGTE);
    // [len + 1 >= capacity]

    // TODO: if false, reallocate and copy
    compiler.instructions.push(encoder::Instruction::Assert);
    // []

    compiler
        .memory
        .read(compiler.instructions, array::data_ptr(arr).memory_addr, 1);
    // [data_ptr]
    compiler
        .memory
        .read(compiler.instructions, array::length(arr).memory_addr, 1);
    compiler.instructions.push(encoder::Instruction::Push(1));
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSub);
    // [len, data_ptr]
    compiler
        .instructions
        .push(encoder::Instruction::Push(element.type_.miden_width()));
    // [element_width, len, data_ptr]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedMul);
    // [len * element_width, data_ptr]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);
    // [data_ptr + len * element_width]
    compiler.memory.read(
        compiler.instructions,
        element.memory_addr,
        element.type_.miden_width(),
    );
    // [element, data_ptr + len * element_width]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [data_ptr + len * element_width, element]
    compiler
        .instructions
        .push(encoder::Instruction::MemStore(None));
    // []

    // Return the element, same as push does in JS
    Ok(element.clone())
}

fn iterate_array_elements<'a>(
    compiler: &mut Compiler<'a, '_, '_>,
    arr: &Symbol,
    current_element_index: &Symbol,
    current_element: &Symbol,
    finished: &Symbol,
    body: Vec<Instruction<'a>>,
) -> Result<()> {
    ensure_eq_type!(arr, Type::Array(_));
    let element_type = element_type(&arr.type_);
    ensure_eq_type!(current_element, @element_type);

    compiler.instructions.extend([
        Instruction::Push(0),
        // [i = 0]
        Instruction::While {
            condition: vec![
                Instruction::Dup(None),
                // [i, i]
                Instruction::MemLoad(Some(length(arr).memory_addr)),
                // [len, i, i]
                Instruction::U32CheckedLT,
                // [i < len, i]
                Instruction::MemLoad(Some(finished.memory_addr)),
                // [finished, i < len, i]
                Instruction::Not,
                // [!finished, i < len, i]
                Instruction::And,
                // [i < len && !finished]
            ],
            body: vec![
                Instruction::Dup(None),
                // [i, i]
                Instruction::MemStore(Some(current_element_index.memory_addr)),
                // [i]
                Instruction::Dup(None),
                // [i, i]
                Instruction::Push(current_element.type_.miden_width()),
                // [inner_width, i, i]
                Instruction::U32CheckedMul,
                // [offset = i * inner_width, i]
                Instruction::MemLoad(Some(data_ptr(arr).memory_addr)),
                // [data_ptr, offset, i]
                Instruction::U32CheckedAdd,
                // [ptr = data_ptr + offset, i]
            ]
            .into_iter()
            .chain((0..current_element.type_.miden_width()).flat_map(|y| {
                vec![
                    Instruction::Dup(None),
                    // [ptr, ptr, i]
                    Instruction::Push(y),
                    // [y, ptr, ptr, i]
                    Instruction::U32CheckedAdd,
                    // [ptr + y, ptr, i]
                    Instruction::MemLoad(None),
                    // [value, ptr, i]
                    Instruction::MemStore(Some(current_element.memory_addr + y)),
                    // [ptr, i]
                ]
            }))
            .chain([Instruction::Drop])
            .chain(body)
            .chain([
                Instruction::Push(1),
                // [1, i]
                Instruction::U32CheckedAdd,
                // [i = i + 1]
            ])
            .collect(),
        },
        // [i]
        Instruction::Drop,
    ]);

    Ok(())
}

fn grow(compiler: &mut Compiler, arr: &Symbol, needed_len: &Symbol) -> Result<Symbol> {
    ensure_eq_type!(arr, Type::Array(_));
    ensure_eq_type!(needed_len, Type::PrimitiveType(PrimitiveType::UInt32));

    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    let then_instructions = {
        let mut insts = Vec::new();
        std::mem::swap(compiler.instructions, &mut insts);

        let new_capacity = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
        compiler
            .memory
            .read(compiler.instructions, array::length(arr).memory_addr, 1);
        // [len]
        compiler.instructions.push(Instruction::Push(2));
        compiler
            .instructions
            .push(encoder::Instruction::U32CheckedMul);
        // [len * 2]
        compiler.instructions.push(Instruction::Push(16));
        compiler
            .instructions
            .push(encoder::Instruction::U32CheckedAdd);
        // [len * 2 + 15]
        compiler.memory.write(
            compiler.instructions,
            new_capacity.memory_addr,
            &[ValueSource::Stack],
        );
        // []

        let new_data_ptr = super::dynamic_alloc(compiler, &[new_capacity.clone()])?;
        copy(
            compiler,
            &data_ptr(arr),
            &length(arr),
            &new_data_ptr,
            &new_capacity,
            1,
        )?;

        compiler.memory.write(
            compiler.instructions,
            array::data_ptr(arr).memory_addr,
            &[ValueSource::Memory(new_data_ptr.memory_addr)],
        );
        compiler.memory.write(
            compiler.instructions,
            array::capacity(arr).memory_addr,
            &[ValueSource::Memory(new_capacity.memory_addr)],
        );

        std::mem::swap(compiler.instructions, &mut insts);
        insts
    };

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            Instruction::MemLoad(Some(array::capacity(arr).memory_addr)),
            // [capacity]
            Instruction::MemLoad(Some(needed_len.memory_addr)),
            // [needed_len, capacity]
            Instruction::U32CheckedGTE,
            // [capacity >= needed_len]
        ],
        then: then_instructions,
        else_: vec![],
    }]);

    Ok(result)
}

fn copy(
    compiler: &mut Compiler,
    source_data_ptr: &Symbol,
    source_len: &Symbol,
    target_data_ptr: &Symbol,
    target_capacity: &Symbol,
    element_width: u32,
) -> Result<()> {
    // Ensure that the target array has enough capacity to hold the source array's contents
    compiler.instructions.extend([
        Instruction::MemLoad(Some(source_len.memory_addr)),
        Instruction::Push(element_width),
        Instruction::U32CheckedMul,
        Instruction::MemLoad(Some(target_capacity.memory_addr)),
        Instruction::U32CheckedLTE,
        Instruction::Assert,
    ]);

    // Calculate total length (source_len * element_width) and push it to the stack
    compiler.instructions.extend([
        Instruction::MemLoad(Some(source_len.memory_addr)),
        Instruction::Push(element_width),
        Instruction::U32CheckedMul,
    ]);
    // [total_length]

    compiler.instructions.extend([
        Instruction::Push(0),
        // [offset = 0, total_length]
        Instruction::While {
            condition: vec![
                Instruction::Dup(None),
                // [offset, offset, total_length]
                Instruction::Dup(Some(2)),
                // [total_length, offset, offset, total_length]
                Instruction::U32CheckedLT,
                // [offset < total_length, offset, total_length]
            ],
            body: vec![
                // [offset, total_length]
                Instruction::Dup(None),
                // [offset, offset, total_length]
                Instruction::MemLoad(Some(source_data_ptr.memory_addr)),
                // [source_data_ptr, offset, offset, total_length]
                Instruction::U32CheckedAdd,
                // [source_data_ptr + offset, offset, total_length]
                Instruction::MemLoad(None),
                // [value, offset, total_length]
                Instruction::MemLoad(Some(target_data_ptr.memory_addr)),
                // [target_data_ptr, value, offset, total_length]
                Instruction::Dup(Some(2)),
                // [offset, target_data_ptr, value, offset, total_length]
                Instruction::U32CheckedAdd,
                // [target_data_ptr + offset, value, offset, total_length]
                Instruction::MemStore(None),
                // [offset, total_length]
                Instruction::Push(1),
                // [1, offset, total_length]
                Instruction::U32CheckedAdd,
                // [offset = offset + 1, total_length]
            ],
        },
        Instruction::Drop, // Drop the loop counter
        Instruction::Drop, // Drop the total length
    ]);

    Ok(())
}
