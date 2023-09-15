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

fn dynamic_new(compiler: &mut Compiler, element_type: Type, needed_len: Symbol) -> Result<Symbol> {
    let array = compiler
        .memory
        .allocate_symbol(Type::Array(Box::new(element_type)));

    let cap = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler
        .memory
        .read(compiler.instructions, cap.memory_addr, 1);
    // [needed_len]
    compiler.instructions.extend([
        Instruction::Push(2),
        Instruction::U32CheckedMul,
        // [needed_len * 2]
        Instruction::Push(16),
        Instruction::U32CheckedAdd,
        // [cap = needed_len * 2 + 16]
    ]);
    compiler.memory.write(
        compiler.instructions,
        cap.memory_addr,
        &[ValueSource::Stack],
    );
    // []

    let array_data_ptr = dynamic_alloc(compiler, &[needed_len])?;

    compiler.memory.write(
        compiler.instructions,
        capacity(&array).memory_addr,
        &[ValueSource::Memory(cap.memory_addr)],
    );
    compiler.memory.write(
        compiler.instructions,
        data_ptr(&array).memory_addr,
        &[ValueSource::Memory(array_data_ptr.memory_addr)],
    );

    Ok(array)
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

pub(crate) enum HashFn {
    Sha256,
    Blake3,
}

pub(crate) fn hash_sha256_blake3(
    compiler: &mut Compiler,
    arr: &Symbol,
    hash: HashFn,
) -> Result<Symbol> {
    let result = compiler.memory.allocate_symbol(Type::Hash8);

    ensure_eq_type!(arr, Type::Array(_));
    let element_type = element_type(&arr.type_);
    assert_eq!(element_type.miden_width(), 1);

    let hash_fn_name = match hash {
        HashFn::Sha256 => "sha256::hash_2to1",
        HashFn::Blake3 => "blake3::hash_2to1",
    };

    let len_div_8 = compiler
        .memory
        .allocate_symbol(Type::Array(Box::new(Type::PrimitiveType(
            PrimitiveType::UInt32,
        ))));
    compiler.memory.read(
        compiler.instructions,
        length(arr).memory_addr,
        length(arr).type_.miden_width(),
    );
    // [len]
    compiler
        .instructions
        .push(Instruction::U32CheckedDiv(Some(8)));
    compiler.memory.write(
        compiler.instructions,
        len_div_8.memory_addr,
        &[ValueSource::Stack],
    );
    let len_mod_8 = compiler
        .memory
        .allocate_symbol(Type::Array(Box::new(Type::PrimitiveType(
            PrimitiveType::UInt32,
        ))));
    compiler.memory.read(
        compiler.instructions,
        length(arr).memory_addr,
        length(arr).type_.miden_width(),
    );
    // [len]
    compiler
        .instructions
        .push(Instruction::U32CheckedMod(Some(8)));
    compiler.memory.write(
        compiler.instructions,
        len_mod_8.memory_addr,
        &[ValueSource::Stack],
    );

    let index = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    compiler.memory.read(
        compiler.instructions,
        result.memory_addr,
        result.type_.miden_width(),
    );
    // [...hash]

    compiler.instructions.extend([Instruction::While {
        condition: vec![
            Instruction::MemLoad(Some(index.memory_addr)),
            // [index]
            Instruction::MemLoad(Some(len_div_8.memory_addr)),
            // [len_div_8, index]
            Instruction::U32CheckedLT,
            // [index < len_div_8]
        ],
        body: vec![
            Instruction::MemLoad(Some(data_ptr(arr).memory_addr)),
            // [data_ptr]
            Instruction::MemLoad(Some(index.memory_addr)),
            // [index, data_ptr]
            Instruction::Push(8),
            // [8, index, data_ptr]
            Instruction::U32CheckedMul,
            // [offset = index * 8, data_ptr]
            Instruction::U32CheckedAdd,
            // [target_ptr = data_ptr + offset]
            Instruction::Dup(None),
            // [target_ptr, target_ptr]
            Instruction::MemLoad(None),
            // [value, target_ptr]
        ]
        .into_iter()
        .chain((1..8).flat_map(|i| {
            [
                Instruction::Dup(Some(i)),
                // [target_ptr, ...values, target_ptr]
                Instruction::Push(1),
                Instruction::U32CheckedAdd,
                // [target_ptr + 1, value, target_ptr]
                Instruction::MemLoad(None),
                // [...values, target_ptr]
            ]
        }))
        .chain([
            // [value_0, value_1, ..., value_7, target_ptr]
            Instruction::MovUp(8),
            // [target_ptr, value_0, value_1, ..., value_7]
            Instruction::Drop,
            // [value_0, value_1, ..., value_7, ...old_hash]
            Instruction::Exec(hash_fn_name),
            // [...hash]
            Instruction::MemLoad(Some(index.memory_addr)),
            Instruction::Push(1),
            Instruction::U32CheckedAdd,
            Instruction::MemStore(Some(index.memory_addr)),
            // [...hash], index += 1
        ])
        .collect(),
    }]);

    for i in 0..8 {
        compiler.instructions.push(encoder::Instruction::If {
            condition: vec![
                Instruction::MemLoad(Some(len_mod_8.memory_addr)),
                // [len_mod_8]
                Instruction::Push(i),
                // [i, len_mod_8]
                Instruction::U32CheckedGT,
                // [len_mod_8 > i]
            ],
            then: vec![
                Instruction::MemLoad(Some(data_ptr(arr).memory_addr)),
                // [data_ptr]
                Instruction::MemLoad(Some(len_div_8.memory_addr)),
                // [len_div_8, data_ptr]
                Instruction::Push(8),
                // [8, len_div_8, data_ptr]
                Instruction::U32CheckedMul,
                // [offset = len_div_8 * 8, data_ptr]
                Instruction::Push(i),
                // [i + offset, data_ptr]
                Instruction::U32CheckedAdd,
                // [target_ptr = data_ptr + offset + i]
                Instruction::MemLoad(None),
            ],
            else_: vec![Instruction::Push(0)],
        });
    }

    compiler.instructions.extend([
        // [value_0, value_1, ..., value_7, ...old_hash]
        Instruction::Exec(hash_fn_name),
        // [...hash]
    ]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack; 8],
    );

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
        // [source_len]
        Instruction::Push(element_width),
        // [element_width, source_len]
        Instruction::U32CheckedMul,
        // [total_length]
        Instruction::Dup(None),
        // [total_length, total_length]
        Instruction::MemLoad(Some(target_capacity.memory_addr)),
        // [capacity, total_length, total_length]
        Instruction::U32CheckedLTE,
        // [total_length <= capacity, total_length]
        Instruction::Assert,
        // [total_length]
    ]);

    compiler.instructions.extend([
        Instruction::Push(0),
        // [offset = 0, total_length]
        Instruction::While {
            condition: vec![
                Instruction::Dup(Some(1)),
                // [total_length, offset, total_length]
                Instruction::Dup(Some(1)),
                // [offset, total_length, offset, total_length]
                Instruction::U32CheckedGTE,
                // [total_length >= offset, offset, total_length]
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

pub(crate) fn includes(compiler: &mut Compiler, arr: &Symbol, el: &Symbol) -> Result<Symbol> {
    ensure_eq_type!(arr, Type::Array(_));
    let element_type = element_type(&arr.type_);
    ensure_eq_type!(el, @element_type);

    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    let index = find_index(compiler, arr, el)?;
    compiler.memory.read(
        compiler.instructions,
        index.memory_addr,
        index.type_.miden_width(),
    );
    // [index]
    compiler.instructions.extend([
        Instruction::Push(-1i32 as u32),
        // [-1, index]
        Instruction::U32CheckedNeq,
        // [index != -1]
        Instruction::MemStore(Some(result.memory_addr)),
    ]);

    Ok(result)
}

pub(crate) fn splice(
    compiler: &mut Compiler,
    arr: &Symbol,
    start: &Symbol,
    delete_count: &Symbol,
) -> Result<Symbol> {
    ensure_eq_type!(arr, Type::Array(_));
    ensure_eq_type!(start, Type::PrimitiveType(PrimitiveType::UInt32));
    ensure_eq_type!(delete_count, Type::PrimitiveType(PrimitiveType::UInt32));

    let element_type = element_type(&arr.type_);

    // Assert that start is less than length
    compiler.instructions.extend([
        Instruction::MemLoad(Some(array::length(arr).memory_addr)),
        // [length]
        Instruction::MemLoad(Some(start.memory_addr)),
        // [start, length]
        Instruction::Dup(Some(1)),
        // [length, start, length]
        Instruction::Dup(Some(1)),
        // [start, length, start, length]
        Instruction::U32CheckedGTE,
        // [length >= start, start, length]
        Instruction::Assert,
        // [start, length]
    ]);

    // If delete_count is higher than length - start, set it to length - start
    compiler.instructions.extend([
        Instruction::If {
            condition: vec![
                // [start, length]
                Instruction::Dup(Some(1)),
                Instruction::Dup(Some(1)),
                // [start, length, start, length]
                Instruction::U32CheckedSub,
                // [length - start, start, length]
                Instruction::MemLoad(Some(delete_count.memory_addr)),
                // [delete_count, length - start, start, length]
                Instruction::U32CheckedLT,
                // [length - start < delete_count, start, length]
            ],
            then: vec![
                // [start, length]
                Instruction::Dup(Some(1)),
                Instruction::Dup(Some(1)),
                // [start, length, start, length]
                Instruction::U32CheckedSub,
                // [length - start, start, length]
                Instruction::MemStore(Some(delete_count.memory_addr)),
                // [start, length]
            ],
            else_: vec![],
        },
        // [start, length]
        Instruction::Drop,
        // [length]
        Instruction::Drop,
        // []
    ]);

    let array_of_deletions = dynamic_new(compiler, element_type.clone(), delete_count.clone())?;

    let new_arr_len = {
        let new_arr_len = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

        // length(arr) - delete_count
        compiler
            .memory
            .read(compiler.instructions, length(arr).memory_addr, 1);
        // [length(arr)]
        compiler
            .memory
            .read(compiler.instructions, delete_count.memory_addr, 1);
        // [delete_count, length(arr)]
        compiler.instructions.push(Instruction::U32CheckedSub);
        // [length(arr) - delete_count]
        compiler.memory.write(
            compiler.instructions,
            new_arr_len.memory_addr,
            &[ValueSource::Stack],
        );

        new_arr_len
    };
    let new_arr = dynamic_new(compiler, element_type.clone(), new_arr_len.clone())?;

    let start_data_ptr = {
        let ptr = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

        compiler
            .memory
            .read(compiler.instructions, data_ptr(arr).memory_addr, 1);
        // [data_ptr]
        compiler.memory.read(
            compiler.instructions,
            start.memory_addr,
            start.type_.miden_width(),
        );
        // [start]
        compiler.instructions.push(Instruction::U32CheckedAdd);
        // [data_ptr + start]
        compiler.memory.write(
            compiler.instructions,
            ptr.memory_addr,
            &[ValueSource::Stack],
        );
        // []

        ptr
    };
    copy(
        compiler,
        &start_data_ptr,
        delete_count,
        &data_ptr(&array_of_deletions),
        &capacity(&array_of_deletions),
        element_type.miden_width(),
    )?;
    compiler.memory.write(
        compiler.instructions,
        length(&array_of_deletions).memory_addr,
        &[ValueSource::Memory(delete_count.memory_addr)],
    );

    copy(
        compiler,
        &data_ptr(arr),
        start,
        &data_ptr(&new_arr),
        &capacity(&new_arr),
        element_type.miden_width(),
    )?;

    let second_source_data_ptr = {
        let ptr = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

        compiler
            .memory
            .read(compiler.instructions, data_ptr(arr).memory_addr, 1);
        // [data_ptr]
        compiler.memory.read(
            compiler.instructions,
            start.memory_addr,
            start.type_.miden_width(),
        );
        // [start]
        compiler.instructions.push(Instruction::U32CheckedAdd);
        // [data_ptr + start]
        compiler.memory.read(
            compiler.instructions,
            delete_count.memory_addr,
            delete_count.type_.miden_width(),
        );
        // [delete_count, data_ptr + start]
        compiler.instructions.push(Instruction::U32CheckedAdd);
        // [data_ptr + start + delete_count]
        compiler.memory.write(
            compiler.instructions,
            ptr.memory_addr,
            &[ValueSource::Stack],
        );
        // []

        ptr
    };

    let second_target_data_ptr = {
        let ptr = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

        compiler
            .memory
            .read(compiler.instructions, data_ptr(&new_arr).memory_addr, 1);
        // [data_ptr]
        compiler.memory.read(
            compiler.instructions,
            start.memory_addr,
            start.type_.miden_width(),
        );
        // [start]
        compiler.instructions.push(Instruction::U32CheckedAdd);
        // [data_ptr + start]
        compiler.memory.write(
            compiler.instructions,
            ptr.memory_addr,
            &[ValueSource::Stack],
        );
        // []

        ptr
    };

    let second_length = {
        let len = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

        compiler.memory.read(
            compiler.instructions,
            length(&arr).memory_addr,
            length(&arr).type_.miden_width(),
        );
        // [length]
        compiler.memory.read(
            compiler.instructions,
            delete_count.memory_addr,
            delete_count.type_.miden_width(),
        );
        // [delete_count, length]
        compiler.instructions.push(Instruction::U32CheckedSub);
        // [length - delete_count]
        compiler.memory.write(
            compiler.instructions,
            len.memory_addr,
            &[ValueSource::Stack],
        );
        // []

        len
    };

    copy(
        compiler,
        &second_source_data_ptr,
        &second_length,
        &second_target_data_ptr,
        &capacity(&new_arr),
        element_type.miden_width(),
    )?;

    compiler.memory.write(
        compiler.instructions,
        length(&new_arr).memory_addr,
        &vec![ValueSource::Memory(new_arr_len.memory_addr)],
    );

    compiler.memory.read(
        compiler.instructions,
        new_arr.memory_addr,
        new_arr.type_.miden_width(),
    );
    compiler.memory.write(
        compiler.instructions,
        arr.memory_addr,
        &vec![ValueSource::Stack; arr.type_.miden_width() as usize],
    );

    return Ok(array_of_deletions);
}

pub(crate) fn slice(
    compiler: &mut Compiler,
    arr: &Symbol,
    start: Option<Symbol>,
    end: Option<&Symbol>,
) -> Result<Symbol> {
    ensure_eq_type!(arr, Type::Array(_));
    let start = start.unwrap_or(uint32::new(compiler, 0));
    ensure_eq_type!(start, Type::PrimitiveType(PrimitiveType::UInt32));
    if let Some(end) = end {
        ensure_eq_type!(end, Type::PrimitiveType(PrimitiveType::UInt32));
    }

    let element_type = element_type(&arr.type_);

    compiler.instructions.extend([
        Instruction::MemLoad(Some(start.memory_addr)),
        // [start]
        Instruction::MemLoad(Some(array::length(arr).memory_addr)),
        // [length, start]
        Instruction::U32CheckedMin,
        // [actual_start = min(start, length)]
    ]);

    match end {
        Some(end) => {
            compiler.instructions.extend([
                Instruction::MemLoad(Some(end.memory_addr)),
                // [end]
                Instruction::MemLoad(Some(array::length(arr).memory_addr)),
                // [length, end]
                Instruction::U32CheckedMin,
                // [actual_end = min(end, length)]
            ]);
        }
        None => {
            compiler
                .memory
                .read(compiler.instructions, array::length(arr).memory_addr, 1);
            // [actual_end = length]
        }
    }

    compiler.instructions.extend([
        // [actual_end, actual_start]
        Instruction::Dup(Some(1)),
        // [actual_start, actual_end, actual_start]
        Instruction::U32CheckedSub,
        // [new_len = end - start, actual_start]
    ]);

    let new_len = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler.memory.write(
        compiler.instructions,
        new_len.memory_addr,
        &[ValueSource::Stack],
    );
    // [actual_start]

    let new_arr = dynamic_new(compiler, element_type.clone(), new_len.clone())?;

    let source_data_ptr = {
        let ptr = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

        compiler
            .memory
            .read(compiler.instructions, data_ptr(arr).memory_addr, 1);
        // [data_ptr, actual_start]
        compiler.instructions.push(Instruction::U32CheckedAdd);
        // [actual_start + data_ptr]
        compiler.memory.write(
            compiler.instructions,
            ptr.memory_addr,
            &[ValueSource::Stack],
        );
        // []

        ptr
    };

    copy(
        compiler,
        &source_data_ptr,
        &new_len,
        &data_ptr(&new_arr),
        &capacity(&new_arr),
        element_type.miden_width(),
    )?;

    compiler.memory.write(
        compiler.instructions,
        length(&new_arr).memory_addr,
        &[ValueSource::Memory(new_len.memory_addr)],
    );

    Ok(new_arr)
}

fn copy_from_element(
    compiler: &mut Compiler,
    source_element: &Symbol,
    target_data_ptr: &Symbol,
    #[allow(unused)] target_capacity: &Symbol,
    element_width: u32,
    index: u32,
) -> Result<()> {
    // Ensure that the target array has enough capacity to hold the source element.
    // Should be covered by the caller, so we only do this in tests.
    #[cfg(test)]
    compiler.instructions.extend([
        Instruction::MemLoad(Some(target_capacity.memory_addr)),
        // [capacity]
        Instruction::Push(index + 1),
        // [index + 1, capacity]
        Instruction::U32CheckedGTE,
        // [capacity >= index + 1]
        Instruction::Assert,
    ]);

    // Copy the source element to the target array at the specified position
    compiler.memory.read(
        compiler.instructions,
        target_data_ptr.memory_addr,
        target_data_ptr.type_.miden_width(),
    );
    // [target_data_ptr]
    for i in 0..element_width {
        let offset = index * element_width + i;
        compiler.instructions.extend([
            // [target_data_ptr]
            Instruction::MemLoad(Some(source_element.memory_addr + i)),
            // [value, target_data_ptr]
            Instruction::Dup(Some(1)),
            // [target_data_ptr, value, target_data_ptr]
            Instruction::Push(offset),
            // [offset, target_data_ptr, value, target_data_ptr]
            Instruction::U32CheckedAdd,
            // [ptr = target_data_ptr + offset, value, target_data_ptr]
            Instruction::MemStore(None),
            // [target_data_ptr]
        ]);
    }
    compiler.instructions.push(Instruction::Drop);
    // []

    Ok(())
}

pub(crate) fn unshift(
    compiler: &mut Compiler,
    arr: &Symbol,
    elements: &[Symbol],
) -> Result<Symbol> {
    let element_type = element_type(&arr.type_);
    for el in elements {
        ensure_eq_type!(el, @element_type);
    }

    compiler
        .memory
        .read(compiler.instructions, array::length(arr).memory_addr, 1);
    // [len]
    compiler
        .instructions
        .push(encoder::Instruction::Push(elements.len() as u32));
    // [elements.len(), len]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);
    // [len + elements.len()]
    let new_len = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler.memory.write(
        compiler.instructions,
        new_len.memory_addr,
        &[ValueSource::Stack],
    );
    // []

    let new_arr = dynamic_new(compiler, element_type.clone(), new_len.clone())?;
    for (i, el) in elements.iter().enumerate() {
        copy_from_element(
            compiler,
            el,
            &data_ptr(&new_arr),
            &capacity(&new_arr),
            element_type.miden_width(),
            i as u32,
        )?;
    }

    let data_ptr_after_new_elements = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    compiler
        .memory
        .read(compiler.instructions, data_ptr(&new_arr).memory_addr, 1);
    // [data_ptr]
    compiler.instructions.extend([
        Instruction::Push(elements.len() as u32 * element_type.miden_width()),
        // [offset, data_ptr]
        Instruction::U32CheckedAdd,
        // [data_ptr + offset]
    ]);
    compiler.memory.write(
        compiler.instructions,
        data_ptr_after_new_elements.memory_addr,
        &[ValueSource::Stack],
    );

    copy(
        compiler,
        &data_ptr(arr),
        &length(arr),
        &data_ptr_after_new_elements,
        &capacity(&new_arr),
        element_type.miden_width(),
    )?;

    compiler.memory.write(
        compiler.instructions,
        length(&new_arr).memory_addr,
        &vec![ValueSource::Memory(new_len.memory_addr)],
    );

    compiler.memory.read(
        compiler.instructions,
        new_arr.memory_addr,
        new_arr.type_.miden_width(),
    );
    compiler.memory.write(
        compiler.instructions,
        arr.memory_addr,
        &vec![ValueSource::Stack; arr.type_.miden_width() as usize],
    );

    Ok(length(&new_arr))
}
