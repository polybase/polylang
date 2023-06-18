use super::{encoder::Instruction, *};

// [is_not_null, t_elements...]
pub(crate) fn width(t: &Type) -> u32 {
    1 + t.miden_width()
}

pub(crate) fn is_not_null(value: &Symbol) -> Symbol {
    Symbol {
        memory_addr: value.memory_addr,
        type_: Type::PrimitiveType(PrimitiveType::Boolean),
    }
}

pub(crate) fn value(value: Symbol) -> Symbol {
    Symbol {
        memory_addr: value.memory_addr + 1,
        type_: match value.type_ {
            Type::Nullable(t) => *t,
            _ => panic!("value is not nullable"),
        },
    }
}

pub(crate) fn eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    let (inner_type_eq_result, inner_type_eq_insts) = {
        let mut insts = vec![];
        std::mem::swap(compiler.instructions, &mut insts);

        let res = compile_eq(compiler, &value(a.clone()), &value(b.clone()));

        std::mem::swap(compiler.instructions, &mut insts);
        (res, insts)
    };

    compiler.instructions.extend([
        // if (a.is_not_null && b.is_not_null) {
        Instruction::If {
            condition: vec![
                Instruction::MemLoad(Some(is_not_null(a).memory_addr)),
                Instruction::MemLoad(Some(is_not_null(b).memory_addr)),
                Instruction::And,
            ],
            // return a.value == b.value
            then: inner_type_eq_insts
                .into_iter()
                .chain([
                    Instruction::MemLoad(Some(inner_type_eq_result.memory_addr)),
                    Instruction::MemStore(Some(result.memory_addr)),
                ])
                .collect(),
            // else { return a.is_not_null == b.is_not_null }
            else_: vec![
                Instruction::MemLoad(Some(is_not_null(a).memory_addr)),
                Instruction::MemLoad(Some(is_not_null(b).memory_addr)),
                Instruction::Eq,
                Instruction::MemStore(Some(result.memory_addr)),
            ],
        },
    ]);

    result
}
