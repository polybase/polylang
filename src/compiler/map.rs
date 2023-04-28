use crate::compiler::encoder::Instruction;

use super::*;

/// [keys_array..., values_array...]
pub(crate) const WIDTH: u32 = array::WIDTH * 2;

pub(crate) fn new_map(
    compiler: &mut Compiler,
    len: u32,
    key_type: Type,
    value_type: Type,
) -> Symbol {
    let (keys_array, _) = array::new(compiler, len, key_type.clone());
    let (values_array, _) = array::new(compiler, len, value_type.clone());

    let map_symbol = Symbol {
        memory_addr: compiler.memory.allocate(WIDTH),
        type_: Type::Map(Box::new(key_type), Box::new(value_type)),
        ..Default::default()
    };

    let map_keys_ptr = keys_arr(&map_symbol);
    let map_values_ptr = values_arr(&map_symbol);

    compiler.memory.write(
        &mut compiler.instructions,
        map_keys_ptr.memory_addr,
        &[ValueSource::Immediate(keys_array.memory_addr)],
    );

    compiler.memory.write(
        &mut compiler.instructions,
        map_values_ptr.memory_addr,
        &[ValueSource::Immediate(values_array.memory_addr)],
    );

    map_symbol
}

pub(crate) fn keys_arr(map_symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: map_symbol.memory_addr,
        type_: match &map_symbol.type_ {
            Type::Map(key_type, _) => Type::Array(key_type.clone()),
            _ => panic!("Expected map type"),
        },
        ..Default::default()
    }
}

pub(crate) fn values_arr(map_symbol: &Symbol) -> Symbol {
    match &map_symbol.type_ {
        Type::Map(_, value_type) => Symbol {
            memory_addr: map_symbol.memory_addr + array::WIDTH,
            type_: Type::Array(value_type.clone()),
            ..Default::default()
        },
        _ => panic!("Expected map type"),
    }
}

// Returns (key, value, valuePtr, didFind)
pub(crate) fn get(
    compiler: &mut Compiler,
    map_symbol: &Symbol,
    key: &Symbol,
) -> (Symbol, Symbol, Symbol, Symbol) {
    let keys_ptr = keys_arr(map_symbol);
    let values_ptr = values_arr(map_symbol);

    let result = Symbol {
        memory_addr: compiler.memory.allocate(1),
        type_: match &keys_ptr.type_ {
            Type::Array(t) => *t.clone(),
            x => panic!("Expected array type, got {x:?}"),
        },
        ..Default::default()
    };

    let current_key_symbol = compiler.memory.allocate_symbol(result.type_.clone());
    let (key_equality_bool, key_equality_instructions) = {
        let mut inst = vec![];
        std::mem::swap(compiler.instructions, &mut inst);

        let eq = compile_eq(compiler, &key, &current_key_symbol);

        std::mem::swap(compiler.instructions, &mut inst);

        (eq, inst)
    };

    let value_type = match &values_ptr.type_ {
        Type::Array(t) => *t.clone(),
        _ => panic!("Expected array type"),
    };
    let found_value_symbol = compiler.memory.allocate_symbol(value_type);
    let found_value_ptr_symbol = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    compiler.instructions.extend(vec![
        // []
        Instruction::MemLoad(Some(array::length(&keys_ptr).memory_addr)),
        // [mapLength]
        Instruction::MemLoad(Some(array::data_ptr(&keys_ptr).memory_addr)),
        // [keyDataPtr, mapLength]
        Instruction::Dup(Some(1)),
        // [mapLength, keyDataPtr, mapLength]
        Instruction::Push(0),
        // [0, mapLength, keyDataPtr, mapLength]
        Instruction::U32CheckedGTE,
        // [mapLength >= 0, keyDataPtr, mapLength]
        Instruction::WhileTrueRaw {
            // [keyDataPtr, mapLength]
            instructions: vec![Instruction::If {
                condition: vec![
                    // [keyDataPtr, mapLength]
                    Instruction::Dup(Some(1)),
                    // [mapLength, keyDataPtr, mapLength]
                    Instruction::Push(0),
                    // [0, mapLength, keyDataPtr, mapLength]
                    Instruction::U32CheckedEq,
                    // [mapLength == 0, keyDataPtr, mapLength]
                ],
                then: vec![
                    // we didn't find any key
                    // [keyDataPtr, mapLength]
                    Instruction::Push(0),
                    // [0, keyDataPtr, mapLength]
                    // iteration stops
                ],
                else_: {
                    let mut inst = vec![
                        Instruction::Dup(Some(1)),
                        // [mapLength, keyDataPtr, mapLength]
                        Instruction::Push(1),
                        // [1, mapLength, keyDataPtr, mapLength]
                        Instruction::U32CheckedSub,
                        // [index = mapLength - 1, keyDataPtr, mapLength]
                        Instruction::Dup(Some(1)),
                        // [keyDataPtr, index = mapLength - 1, keyDataPtr, mapLength]
                        Instruction::U32CheckedAdd,
                        // [keyPtr = keyDataPtr + index, keyDataPtr, mapLength]
                    ];

                    for i in 0..current_key_symbol.type_.miden_width() {
                        inst.push(Instruction::Dup(None));
                        // [keyPtr, keyPtr, keyDataPtr, mapLength]
                        inst.push(Instruction::Push(i));
                        // [i, keyPtr, keyPtr, keyDataPtr, mapLength]
                        inst.push(Instruction::U32CheckedAdd);
                        // [keyPtr + i, keyPtr, keyDataPtr, mapLength]
                        inst.push(Instruction::MemLoad(None));
                        // [key[i], keyPtr, keyDataPtr, mapLength]
                        inst.push(Instruction::MemStore(Some(
                            current_key_symbol.memory_addr + i,
                        )));
                        // [keyPtr, keyDataPtr, mapLength]
                    }
                    inst.push(Instruction::Drop);
                    // [keyDataPtr, mapLength]

                    inst.extend(key_equality_instructions);

                    inst.push(Instruction::If {
                        condition: vec![Instruction::MemLoad(Some(key_equality_bool.memory_addr))],
                        then: vec![
                            // we found the key
                            // [keyDataPtr, mapLength]
                            Instruction::Push(0),
                            // [0, keyDataPtr, mapLength]
                            // iteration stops
                        ],
                        else_: vec![
                            // the keys don't match
                            // decrease mapLength by 1
                            Instruction::Swap,
                            // [mapLength, keyDataPtr]
                            Instruction::Push(1),
                            // [1, mapLength, keyDataPtr]
                            Instruction::U32CheckedSub,
                            // [mapLength - 1, keyDataPtr]
                            Instruction::Swap,
                            // [keyDataPtr, mapLength - 1]
                            Instruction::Push(1),
                            // [1, keyDataPtr, mapLength - 1]
                            // iteration continues
                        ],
                    });

                    inst
                },
            }],
        },
        // [keyDataPtr, mapLength]
        Instruction::Drop,
        // [mapLength]
        Instruction::If {
            condition: vec![Instruction::MemLoad(Some(key_equality_bool.memory_addr))],
            then: {
                // Load the value into found_value_symbol
                let mut inst = vec![
                    Instruction::Push(1),
                    // [1, mapLength]
                    Instruction::U32CheckedSub,
                    // [index = mapLength - 1]
                    Instruction::MemLoad(Some(array::data_ptr(&values_ptr).memory_addr)),
                    // [valueDataPtr, index]
                    Instruction::Dup(Some(1)),
                    // [index, valueDataPtr, index]
                    Instruction::U32CheckedAdd,
                    // [valuePtr = valueDataPtr + index, index]
                    Instruction::MemStore(Some(found_value_ptr_symbol.memory_addr)),
                    // [index]
                ];
                for i in 0..found_value_symbol.type_.miden_width() {
                    inst.push(Instruction::Dup(None));
                    // [index, index]
                    inst.push(Instruction::Push(i));
                    // [i, index, index]
                    inst.push(Instruction::U32CheckedAdd);
                    // [index + i, index]
                    inst.push(Instruction::MemLoad(Some(
                        array::data_ptr(&values_ptr).memory_addr,
                    )));
                    // [valueStartPtr, index + i, index]
                    inst.push(Instruction::U32CheckedAdd);
                    // [index + i + valueStartPtr, index]
                    inst.push(Instruction::MemLoad(None));
                    // [value, index]
                    inst.push(Instruction::MemStore(Some(
                        found_value_symbol.memory_addr + i,
                    )));
                    // [index]
                }

                inst
            },
            else_: vec![],
        },
        // [index]
        Instruction::Drop,
        // []
    ]);

    (
        current_key_symbol,
        found_value_symbol,
        found_value_ptr_symbol,
        key_equality_bool,
    )
}
