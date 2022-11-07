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

// reads the length of the string, allocates memory for the data, and reads the data
pub(crate) fn deserialize(compiler: &mut Compiler) -> Symbol {
    let string = compiler.memory.allocate_symbol(Type::String);
    let length = length(&string);
    let data_ptr = data_ptr(&string);

    compiler.memory.write(
        &mut compiler.instructions,
        length.memory_addr,
        &vec![ValueSource::Stack; length.type_.miden_width() as _],
    );

    // let mut while_instructions = Vec::new();
    // let while_compiler = Compiler::new(&mut while_instructions, compiler.memory);

    /*
    let i = 0;
    while (i < length) {
        mem_store(data_ptr + i, read());
        i = i + 1;
    }

    miden assembly for the code above:
    push.0
    # [i, bytes...]
    while (
        dup
        # [i, i, bytes...]
        mem_load.${length.memory_addr}
        # [length, i, i, bytes...]
        u32checked_lt
        # [i < length, i, bytes...]
    ) {
        # [i, bytes...]
        dup
        #[i, i, bytes...]
        push.${data_ptr.memory_addr}
        #[data_ptr, i, i, bytes...]
        u32checked_add
        #[data_ptr + i, i, bytes...]
        movup.2
        #[byte, data_ptr + i, i, bytes...]
        swap
        #[data_ptr + i, byte, i, bytes...]
        mem_store
        #[byte, i, bytes...]
        drop
        #[i, bytes...]
        push.1
        #[1, i, bytes...]
        u32checked_add
        #[i + 1, bytes...]
    }
    */
    // TODO: maybe just write this in our AST?
    compiler.instructions.push(encoder::Instruction::While {
        condition: vec![],
        body: vec![],
    });

    // compile_ast_function_call(&ast::Function {
    //     name: "",
    //     parameters: vec![],
    //     statements: todo!(),
    //     statements_code: todo!(),
    // }, compiler, scope, args);

    string
}
