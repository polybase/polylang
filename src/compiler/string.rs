use super::{encoder::Instruction, *};

// A string is represented as [length, pointer]
pub(crate) fn new(compiler: &mut Compiler, value: &str) -> (Symbol, Option<u32>) {
    let symbol = compiler.memory.allocate_symbol(Type::String);

    let mut string_addr = None;
    if !value.is_empty() {
        string_addr = Some(compiler.memory.allocate(value.len() as u32));
        let string_addr = string_addr.unwrap();

        compiler.memory.write(
            compiler.instructions,
            symbol.memory_addr,
            &[
                ValueSource::Immediate(value.len() as u32),
                ValueSource::Immediate(string_addr),
            ],
        );

        compiler.memory.write(
            compiler.instructions,
            string_addr,
            &value
                .bytes()
                .map(|c| ValueSource::Immediate(c as u32))
                .collect::<Vec<_>>(),
        );
    }

    (symbol, string_addr)
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
    compiler.instructions.push(Instruction::While {
        // len > 0
        condition: vec![
            Instruction::Dup(None),
            // [len, len, src_ptr, dest_ptr]
            Instruction::Push(0),
            // [0, len, len, src_ptr, dest_ptr]
            Instruction::U32CheckedGT,
            // [len > 0, len, src_ptr, dest_ptr]
        ],
        // len--; *dest_ptr = *src_ptr; src_ptr++; dest_ptr++;
        body: vec![
            // [len, src_ptr, dest_ptr]
            Instruction::Push(1),
            // [1, len, src_ptr, dest_ptr]
            Instruction::U32CheckedSub,
            // [len - 1, src_ptr, dest_ptr]
            Instruction::MovDown(2),
            // [src_ptr, dest_ptr, len - 1]
            Instruction::Dup(None),
            // [src_ptr, src_ptr, dest_ptr, len - 1]
            Instruction::MemLoad(None),
            // [*src_ptr, src_ptr, dest_ptr, len - 1]
            Instruction::Dup(Some(2)),
            // [dest_ptr, *src_ptr, src_ptr, dest_ptr, len - 1]
            Instruction::MemStore(None),
            // [src_ptr, dest_ptr, len - 1]
            Instruction::Push(1),
            // [1, src_ptr, dest_ptr, len - 1]
            Instruction::U32CheckedAdd,
            // [src_ptr + 1, dest_ptr, len - 1]
            Instruction::MovDown(2),
            // [dest_ptr, len - 1, src_ptr + 1]
            Instruction::Push(1),
            // [1, dest_ptr, len - 1, src_ptr + 1]
            Instruction::U32CheckedAdd,
            // [dest_ptr + 1, len - 1, src_ptr + 1]
            Instruction::MovDown(2),
            // [len - 1, src_ptr + 1, dest_ptr + 1]
        ],
    });

    // [len, src_ptr, dest_ptr]
    compiler.instructions.push(Instruction::Drop);
    // [src_ptr, dest_ptr]
    compiler.instructions.push(Instruction::Drop);
    // [dest_ptr]
    compiler.instructions.push(Instruction::Drop);
    // []
}

pub(super) fn builtins() -> impl Iterator<Item = (String, Option<TypeConstraint>, Function<'static>)>
{
    IntoIterator::into_iter([
        (
            "startsWith",
            Function::Builtin(|compiler, _scope, args| -> Result<Symbol> {
                ensure!(
                    args.len() == 2,
                    ArgumentsCountSnafu {
                        found: args.len(),
                        expected: 2usize
                    }
                );
                let a = &args[0];
                let b = &args[1];
                starts_with(compiler, a, b)
            }),
        ),
        (
            "includes",
            Function::Builtin(|compiler, _scope, args| -> Result<Symbol> {
                ensure!(
                    args.len() == 2,
                    ArgumentsCountSnafu {
                        found: args.len(),
                        expected: 2usize
                    }
                );
                let a = &args[0];
                let b = &args[1];
                includes(compiler, a, b)
            }),
        ),
        (
            "indexOf",
            Function::Builtin(|compiler, _scope, args| -> Result<Symbol> {
                ensure!(
                    args.len() == 2,
                    ArgumentsCountSnafu {
                        found: args.len(),
                        expected: 2usize
                    }
                );
                let a = &args[0];
                let b = &args[1];
                index_of(compiler, a, b)
            }),
        ),
    ])
    .map(|(name, func)| {
        (
            name.to_string(),
            Some(TypeConstraint::Exact(Type::String)),
            func,
        )
    })
}

fn starts_with(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Result<Symbol> {
    let a_len = length(a);
    let a_data_ptr = data_ptr(a);

    let b_len = length(b);
    let b_data_ptr = data_ptr(b);

    compiler.memory.read(
        compiler.instructions,
        b_len.memory_addr,
        b_len.type_.miden_width(),
    );
    compiler.memory.read(
        compiler.instructions,
        a_len.memory_addr,
        a_len.type_.miden_width(),
    );

    compiler.instructions.push(Instruction::If {
        condition: vec![
            Instruction::Dup(Some(1)),
            // [b_len, a_len, b_len]
            Instruction::U32CheckedGTE,
            // [b_len, b_len >= a_len]
        ],
        then: {
            let mut then = vec![];
            compiler.memory.read(
                &mut then,
                a_data_ptr.memory_addr,
                a_data_ptr.type_.miden_width(),
            );
            compiler.memory.read(
                &mut then,
                b_data_ptr.memory_addr,
                b_data_ptr.type_.miden_width(),
            );
            // [b_len, a_ptr, b_ptr]

            starts_with_inner(&mut then);

            then
        },
        else_: vec![Instruction::Push(0)],
    });

    let result = boolean::new(compiler, true);
    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    Ok(result)
}

// [b_len, a_ptr, b_ptr] -> [starts_with]
fn starts_with_inner(instructions: &mut Vec<Instruction>) {
    instructions.extend([
        Instruction::MovUp(2),
        // [a_ptr, b_ptr, b_len]
        Instruction::While {
            // len > 0
            condition: vec![Instruction::If {
                condition: vec![
                    Instruction::Dup(None),
                    Instruction::Push(0),
                    Instruction::U32CheckedGT,
                    // [.., len > 0]
                ],
                then: vec![Instruction::If {
                    condition: vec![
                        Instruction::Dup(Some(2)),
                        Instruction::MemLoad(None),
                        Instruction::Dup(Some(2)),
                        Instruction::MemLoad(None),
                        Instruction::U32CheckedEq,
                        // [*a_ptr == *b_ptr]
                    ],
                    then: vec![Instruction::Push(1)],
                    else_: vec![
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Push(0),
                        Instruction::Push(0),
                        // [result=false, false]
                    ],
                }],
                else_: vec![
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Push(1),
                    Instruction::Push(0),
                    // [result=true, false]
                ],
            }],
            body: vec![
                // [a_ptr, b_ptr, len]
                Instruction::MovUp(2),
                Instruction::Push(1),
                Instruction::U32CheckedAdd,
                // [b_ptr, len, a_ptr + 1]
                Instruction::MovUp(2),
                Instruction::Push(1),
                Instruction::U32CheckedAdd,
                // [len, a_ptr + 1, b_ptr + 1]
                Instruction::MovUp(2),
                Instruction::Push(1),
                Instruction::U32CheckedSub,
                // [a_ptr + 1, b_ptr + 1, len - 1]
            ],
        },
    ]);
}

fn includes(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Result<Symbol> {
    let a_len = length(a);
    let a_data_ptr = data_ptr(a);

    let b_len = length(b);
    let b_data_ptr = data_ptr(b);

    compiler.memory.read(
        compiler.instructions,
        a_len.memory_addr,
        a_len.type_.miden_width(),
    );
    compiler.memory.read(
        compiler.instructions,
        b_len.memory_addr,
        b_len.type_.miden_width(),
    );
    compiler.memory.read(
        compiler.instructions,
        a_data_ptr.memory_addr,
        a_data_ptr.type_.miden_width(),
    );
    compiler.memory.read(
        compiler.instructions,
        b_data_ptr.memory_addr,
        b_data_ptr.type_.miden_width(),
    );
    // [a_len, b_len, a_data_ptr, b_data_ptr]

    /*
        for i in 0..a.len {
            if a.len - i < b.len {
                return false;
            }

            let matched = true;
            for j in 0..b.len {
                 if b[j] != a[i + j] {
                    matched = false;
                    break;
                }
            }

            if matched {
                return true;
            }
        }

        return a.len == 0 && b.len == 0
    */
    compiler.instructions.extend([
        Instruction::Dup(Some(3)),
        Instruction::Push(0),
        Instruction::U32CheckedEq,
        Instruction::Dup(Some(3)),
        Instruction::Push(0),
        Instruction::U32CheckedEq,
        Instruction::U32CheckedAnd,
        // [.., result = a_len == 0 && b_len == 0]
        Instruction::Push(0),
        // [a_len, b_len, a_data_ptr, b_data_ptr, result, i = 0]
        Instruction::While {
            condition: vec![
                Instruction::Dup(Some(0)),
                Instruction::Dup(Some(6)),
                Instruction::U32CheckedLT,
                // [i < a_len]
            ],
            body: vec![
                // [a_len, b_len, a_data_ptr, b_data_ptr, result, i]
                Instruction::If {
                    condition: vec![
                        Instruction::Dup(Some(5)),
                        Instruction::Dup(Some(1)),
                        Instruction::U32CheckedSub,
                        // [a_len - i]
                        Instruction::Dup(Some(5)),
                        Instruction::U32CheckedGTE,
                        // [a_left >= b_len]
                    ],
                    then: search_inner_loop(),
                    else_: vec![
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Push(0),
                        // [.., result = false]
                        Instruction::Push(u32::MAX - 1),
                        // [.., i = max - 1], i.e end outer iteration
                        Instruction::Push(0),
                    ],
                },
                Instruction::If {
                    condition: vec![
                        // [.., matched]
                    ],
                    then: vec![
                        Instruction::Drop,
                        Instruction::Push(1),
                        // [.., result = true]
                        Instruction::Push(u32::MAX - 1),
                        // [.., i = max - 1], i.e end outer iteration
                    ],
                    else_: vec![
                        Instruction::Push(1),
                        Instruction::U32CheckedAdd,
                        // [.., i = i + 1]
                    ],
                },
            ],
        },
    ]);

    let result = boolean::new(compiler, true);
    compiler.instructions.extend([
        Instruction::Drop,
        Instruction::MemStore(Some(result.memory_addr)),
        Instruction::Drop,
        Instruction::Drop,
        Instruction::Drop,
        Instruction::Drop,
    ]);
    Ok(result)
}

fn index_of(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Result<Symbol> {
    let a_len = length(a);
    let a_data_ptr = data_ptr(a);

    let b_len = length(b);
    let b_data_ptr = data_ptr(b);

    compiler.memory.read(
        compiler.instructions,
        a_len.memory_addr,
        a_len.type_.miden_width(),
    );
    compiler.memory.read(
        compiler.instructions,
        b_len.memory_addr,
        b_len.type_.miden_width(),
    );
    compiler.memory.read(
        compiler.instructions,
        a_data_ptr.memory_addr,
        a_data_ptr.type_.miden_width(),
    );
    compiler.memory.read(
        compiler.instructions,
        b_data_ptr.memory_addr,
        b_data_ptr.type_.miden_width(),
    );
    // [a_len, b_len, a_data_ptr, b_data_ptr]

    /*
        for i in 0..a.len {
            if a.len - i < b.len {
                return -1;
            }

            let matched = true;
            for j in 0..b.len {
                 if b[j] != a[i + j] {
                    matched = false;
                    break;
                }
            }

            if matched {
                return i;
            }
        }

        return if a.len == 0 && b.len == 0 { 0 } else { -1 }
    */
    compiler.instructions.extend([
        Instruction::If {
            condition: vec![
                Instruction::Dup(Some(3)),
                Instruction::Push(0),
                Instruction::U32CheckedEq,
                Instruction::Dup(Some(3)),
                Instruction::Push(0),
                Instruction::U32CheckedEq,
                Instruction::U32CheckedAnd,
                // a_len == 0 && b_len == 0
            ],
            then: vec![Instruction::Push(0)],
            else_: vec![Instruction::Push(u32::MAX)],
        },
        Instruction::Push(0),
        // [a_len, b_len, a_data_ptr, b_data_ptr, result, i = 0]
        Instruction::While {
            condition: vec![
                Instruction::Dup(Some(0)),
                Instruction::Dup(Some(6)),
                Instruction::U32CheckedLT,
                // [i < a_len]
            ],
            body: vec![
                // [a_len, b_len, a_data_ptr, b_data_ptr, result, i]
                Instruction::If {
                    condition: vec![
                        Instruction::Dup(Some(5)),
                        Instruction::Dup(Some(1)),
                        Instruction::U32CheckedSub,
                        // [a_len - i]
                        Instruction::Dup(Some(5)),
                        Instruction::U32CheckedGTE,
                        // [a_left >= b_len]
                    ],
                    then: search_inner_loop(),
                    else_: vec![
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Push(u32::MAX),
                        // [result = false]
                        Instruction::Push(u32::MAX - 1),
                        // [i = max - 1], i.e end outer iteration
                        Instruction::Push(0),
                    ],
                },
                Instruction::If {
                    condition: vec![
                        // [.., matched]
                    ],
                    then: vec![
                        Instruction::Swap,
                        Instruction::Drop,
                        Instruction::Push(u32::MAX - 1),
                        // [result = i, i = max - 1]
                    ],
                    else_: vec![
                        Instruction::Push(1),
                        Instruction::U32CheckedAdd,
                        // [.., i = i + 1]
                    ],
                },
            ],
        },
    ]);

    let result = int32::new(compiler, 0);
    compiler.instructions.extend([
        Instruction::Drop,
        Instruction::MemStore(Some(result.memory_addr)),
        Instruction::Drop,
        Instruction::Drop,
        Instruction::Drop,
        Instruction::Drop,
    ]);
    Ok(result)
}

// Given [a_len, b_len, a_data_ptr, b_data_ptr, result, i]
// Appends `matched` equals to `a_data_ptr[i..i+j] == b_data_ptr[0..j]`
fn search_inner_loop() -> Vec<Instruction<'static>> {
    vec![
        Instruction::Dup(None),
        Instruction::Push(0),
        // [.., i, j = 0]
        Instruction::While {
            condition: vec![Instruction::If {
                condition: vec![
                    Instruction::Dup(Some(0)),
                    Instruction::Dup(Some(7)),
                    Instruction::U32CheckedLT,
                    // [j < b_len]
                ],
                then: vec![
                    Instruction::Dup(Some(5)),
                    // TODO: support non-ASCII UTF chars here
                    Instruction::Dup(Some(2)),
                    Instruction::U32CheckedAdd,
                    Instruction::MemLoad(None),
                    // [.., a_data_ptr[i]]
                    Instruction::Dup(Some(5)),
                    // TODO: support non-ASCII UTF chars here
                    Instruction::Dup(Some(2)),
                    Instruction::U32CheckedAdd,
                    Instruction::MemLoad(None),
                    Instruction::U32CheckedEq,
                    // [.., a_data_ptr[i] == b_data_ptr[j]]
                ],
                else_: vec![Instruction::Push(0)],
            }],
            body: vec![
                Instruction::Push(1),
                Instruction::U32CheckedAdd,
                // [i, j = j + 1]
                Instruction::Swap,
                Instruction::Push(1),
                Instruction::U32CheckedAdd,
                // [j, i = i + 1]
                Instruction::Swap,
            ],
        },
        Instruction::Swap,
        Instruction::Drop,
        Instruction::Dup(Some(5)),
        Instruction::U32CheckedEq,
        // [.., matched = j == b_len]
    ]
}

pub(crate) fn concat(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Result<Symbol> {
    let (result, _) = new(compiler, "");
    let result_data_ptr = data_ptr(&result);
    let result_len = length(&result);

    let a_len = length(a);
    let a_data_ptr = data_ptr(a);

    let b_len = length(b);
    let b_data_ptr = data_ptr(b);

    // Set the length of the result
    compiler.memory.read(
        compiler.instructions,
        a_len.memory_addr,
        a_len.type_.miden_width(),
    );
    // [a_len]

    compiler.memory.read(
        compiler.instructions,
        b_len.memory_addr,
        b_len.type_.miden_width(),
    );
    // [b_len, a_len]

    compiler.instructions.push(Instruction::U32CheckedAdd);
    // [a_len + b_len]

    compiler.memory.write(
        compiler.instructions,
        result_len.memory_addr,
        &[ValueSource::Stack],
    );

    let allocated_ptr = dynamic_alloc(compiler, &[result_len])?;

    compiler.memory.write(
        compiler.instructions,
        result_data_ptr.memory_addr,
        &[ValueSource::Memory(allocated_ptr.memory_addr)],
    );

    compiler.memory.read(
        compiler.instructions,
        result_data_ptr.memory_addr,
        result_data_ptr.type_.miden_width(),
    );
    // [result_data_ptr]

    compiler.memory.read(
        compiler.instructions,
        a_data_ptr.memory_addr,
        a_data_ptr.type_.miden_width(),
    );
    // [a_data_ptr, result_data_ptr]

    compiler.memory.read(
        compiler.instructions,
        a_len.memory_addr,
        a_len.type_.miden_width(),
    );
    // [a_len, a_data_ptr, result_data_ptr]

    copy_str_stack(compiler);
    // []

    compiler.memory.read(
        compiler.instructions,
        result_data_ptr.memory_addr,
        result_data_ptr.type_.miden_width(),
    );
    // [result_data_ptr]

    compiler.memory.read(
        compiler.instructions,
        a_len.memory_addr,
        a_len.type_.miden_width(),
    );
    // [a_len, result_data_ptr]

    compiler.instructions.push(Instruction::U32CheckedAdd);
    // [result_data_ptr + a_len]

    compiler.memory.read(
        compiler.instructions,
        b_data_ptr.memory_addr,
        b_data_ptr.type_.miden_width(),
    );
    // [b_data_ptr, result_data_ptr + a_len]

    compiler.memory.read(
        compiler.instructions,
        b_len.memory_addr,
        b_len.type_.miden_width(),
    );
    // [b_len, b_data_ptr, result_data_ptr + a_len]

    copy_str_stack(compiler);
    // []

    Ok(result)
}

pub(crate) fn eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    compiler.memory.read(
        compiler.instructions,
        length(a).memory_addr,
        length(a).type_.miden_width(),
    );
    // [a_len]
    compiler.memory.read(
        compiler.instructions,
        length(b).memory_addr,
        length(b).type_.miden_width(),
    );
    // [b_len, a_len]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            Instruction::U32CheckedEq,
            // [a_len == b_len]
            Instruction::Dup(None),
            // [a_len == b_len, a_len == b_len]
            // Covers the case of '' == ''
            Instruction::MemStore(Some(result.memory_addr)),
            // [a_len == b_len]
        ],
        then: vec![
            Instruction::MemLoad(Some(length(a).memory_addr)),
            // [len]
            Instruction::Push(0),
            // [i = 0, len]
            Instruction::Push(0),
            // [0, i, len]
            Instruction::Dup(Some(2)),
            // [len, 0, i, len]
            Instruction::U32CheckedLT,
            // [0 < len, i, len]
            Instruction::WhileTrueRaw {
                instructions: vec![
                    Instruction::MemLoad(Some(data_ptr(a).memory_addr)),
                    // [a_data_ptr, i, len]
                    Instruction::Dup(Some(1)),
                    // [i, a_data_ptr, i, len]
                    Instruction::U32CheckedAdd,
                    // [a_data_ptr + i, i, len]
                    Instruction::MemLoad(None),
                    // [a_data_ptr[i], i, len]
                    Instruction::MemLoad(Some(data_ptr(b).memory_addr)),
                    // [b_data_ptr, a_data_ptr[i], i, len]
                    Instruction::Dup(Some(2)),
                    // [i, b_data_ptr, a_data_ptr[i], i, len]
                    Instruction::U32CheckedAdd,
                    // [b_data_ptr + i, a_data_ptr[i], i, len]
                    Instruction::MemLoad(None),
                    // [b_data_ptr[i], a_data_ptr[i], i, len]
                    Instruction::U32CheckedEq,
                    // [a_data_ptr[i] == b_data_ptr[i], i, len]
                    Instruction::Dup(None),
                    Instruction::MemStore(Some(result.memory_addr)),
                    // [a_data_ptr[i] == b_data_ptr[i], i, len]
                    Instruction::Swap,
                    // [i, a_data_ptr[i] == b_data_ptr[i], len]
                    Instruction::Push(1),
                    // [1, i, a_data_ptr[i] == b_data_ptr[i], len]
                    Instruction::U32CheckedAdd,
                    // [i + 1, a_data_ptr[i] == b_data_ptr[i], len]
                    Instruction::MovDown(2),
                    // [a_data_ptr[i] == b_data_ptr[i], i + 1, len]
                    Instruction::Dup(Some(1)),
                    // [i + 1, a_data_ptr[i] == b_data_ptr[i], i + 1, len]
                    Instruction::Dup(Some(3)),
                    // [len, i + 1, a_data_ptr[i] == b_data_ptr[i], i + 1, len]
                    Instruction::U32CheckedLT,
                    // [i + 1 < len, a_data_ptr[i] == b_data_ptr[i], i + 1, len]
                    Instruction::And,
                    // [a_data_ptr[i] == b_data_ptr[i] && i + 1 < len, i + 1, len]
                ],
            },
            // [i, len]
            Instruction::Drop,
            // [len]
            Instruction::Drop,
            // []
        ],
        else_: vec![],
    }]);

    result
}

pub(crate) fn hash(compiler: &mut Compiler, _scope: &Scope, args: &[Symbol]) -> Result<Symbol> {
    ensure!(
        args.len() == 1,
        ArgumentsCountSnafu {
            found: args.len(),
            expected: 1usize
        }
    );
    let string = &args[0];
    ensure_eq_type!(
        string,
        Type::String | Type::Bytes | Type::CollectionReference { .. }
    );

    let result = compiler.memory.allocate_symbol(Type::Hash);

    compiler.instructions.extend([
        Instruction::Push(0),
        Instruction::Push(0),
        Instruction::Push(0),
        Instruction::Push(0),
    ]);
    // [h[3], h[2], h[1], h[0]]
    compiler.memory.read(
        compiler.instructions,
        string::data_ptr(string).memory_addr,
        string::data_ptr(string).type_.miden_width(),
    );
    // [data_ptr, h[3], h[2], h[1], h[0]]
    compiler.memory.read(
        compiler.instructions,
        string::length(string).memory_addr,
        string::length(string).type_.miden_width(),
    );
    // [len, data_ptr, h[3], h[2], h[1], h[0]]

    compiler.instructions.push(Instruction::While {
        // len > 0
        condition: vec![
            Instruction::Dup(None),
            // [len, len, data_ptr, h[3], h[2], h[1], h[0]]
            Instruction::Push(0),
            // [0, len, len, data_ptr, h[3], h[2], h[1], h[0]]
            Instruction::U32CheckedGT,
            // [len > 0, len, data_ptr, h[3], h[2], h[1], h[0]]
        ],
        body: vec![
            // [len, data_ptr, h[3], h[2], h[1], h[0]]
            Instruction::Push(1),
            // [1, len, data_ptr, h[3], h[2], h[1], h[0]]
            Instruction::U32CheckedSub,
            // [len - 1, data_ptr, h[3], h[2], h[1], h[0]]
            Instruction::MovDown(5),
            // [data_ptr, h[3], h[2], h[1], h[0], len - 1]
            Instruction::Dup(None),
            // [data_ptr, data_ptr, h[3], h[2], h[1], h[0], len - 1]
            Instruction::MovDown(6),
            // [data_ptr, h[3], h[2], h[1], h[0], len - 1, data_ptr]
            Instruction::MemLoad(None),
            // [byte, h[3], h[2], h[1], h[0], len - 1, data_ptr]
            Instruction::Push(0),
            Instruction::Push(0),
            Instruction::Push(0),
            // [0, 0, 0, byte, h[3], h[2], h[1], h[0], len - 1, data_ptr]
            Instruction::HMerge,
            // [h[3], h[2], h[1], h[0], len - 1, data_ptr]
            Instruction::MovUp(5),
            // [data_ptr, h[3], h[2], h[1], h[0], len - 1]
            Instruction::Push(1),
            // [1, data_ptr, h[3], h[2], h[1], h[0], len - 1]
            Instruction::U32CheckedAdd,
            // [data_ptr + 1, h[3], h[2], h[1], h[0], len - 1]
            Instruction::MovUp(5),
            // [len - 1, data_ptr + 1, h[3], h[2], h[1], h[0]]
        ],
    });

    // [len, data_ptr, h[3], h[2], h[1], h[0]]
    compiler.instructions.push(Instruction::Drop);
    // [data_ptr, h[3], h[2], h[1], h[0]]
    compiler.instructions.push(Instruction::Drop);
    // [h[3], h[2], h[1], h[0]]

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[
            ValueSource::Stack,
            ValueSource::Stack,
            ValueSource::Stack,
            ValueSource::Stack,
        ],
    );

    Ok(result)
}
