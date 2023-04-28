use super::*;

// [is_null, t_elements...]
pub(crate) fn width(t: &Type) -> u32 {
    1 + t.miden_width()
}

pub(crate) fn is_not_null(value: &Symbol) -> Symbol {
    Symbol {
        memory_addr: value.memory_addr,
        type_: Type::PrimitiveType(PrimitiveType::Boolean),
        ..Default::default()
    }
}

pub(crate) fn value(value: Symbol) -> Symbol {
    Symbol {
        memory_addr: value.memory_addr + 1,
        type_: match value.type_ {
            Type::Nullable(t) => *t,
            _ => panic!("value is not nullable"),
        },
        ..Default::default()
    }
}
