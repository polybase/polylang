/// Notation:
/// - x_sign - float sign bit of x
/// - x_exp  - float exponent of x
/// - x_mant - float mantissa of x
/// - z^     - float value without shifting, i.e x_exp^ = x_exp << 23; x_mant^ = x_mant << 0 = x_mant
use super::*;

use encoder::Instruction;

pub(crate) const WIDTH: u32 = 1;

const SIGN_MASK: u32 = 0x8000_0000;
const EXP_MASK: u32 = 0x7f80_0000;
const EXP_SHIFT: u32 = 23;
const MANT_MASK: u32 = 0x007f_ffff;
const EXP_BIAS: u32 = 0x7f;
const NAN: u32 = EXP_MASK | 0x0040_0000;
const INFINITY: u32 = EXP_MASK;
const LEADING_ONE_BIT: u32 = 0x0080_0000;

pub(crate) fn new(compiler: &mut Compiler, value: f32) -> Symbol {
    let symbol = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    compiler.memory.write(
        compiler.instructions,
        symbol.memory_addr,
        &[ValueSource::Immediate(value.to_bits())],
    );

    symbol
}

// [a, b] -> [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
fn prepare_stack_for_arithmetic(compiler: &mut Compiler, a: &Symbol, b: &Symbol) {
    compiler
        .memory
        .read(compiler.instructions, a.memory_addr, a.type_.miden_width());
    decompose(compiler);
    // [a_mant, a_sign^, a_exp]
    compiler
        .memory
        .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
    decompose(compiler);
    // [b_mant, b_sign^, b_exp, a_mant, a_sign^, a_exp]

    compiler.instructions.push(Instruction::MovDown(5));
    // [b_sign^, b_exp, a_mant, a_sign^, a_exp, b_mant]
    compiler.instructions.push(Instruction::MovDown(4));
    // [b_exp, a_mant, a_sign^, a_exp, b_sign^, b_mant]
    compiler.instructions.push(Instruction::MovDown(3));
    // [a_mant, a_sign^, a_exp, b_exp, b_sign^, b_mant]
    compiler.instructions.push(Instruction::MovDown(4));
    // [a_sign^, a_exp, b_exp, b_sign^, a_mant, b_mant]
    compiler.instructions.push(Instruction::MovDown(2));
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
}

// [a] -> [a_mant, a_sign^, a_exp]
fn decompose(compiler: &mut Compiler) {
    compiler.instructions.extend([
        // [a]
        Instruction::Dup(None),
        // [a, a]
        Instruction::Push(EXP_MASK),
        Instruction::U32CheckedAnd,
        // [a_exp^, a]
        Instruction::U32CheckedSHR(Some(EXP_SHIFT)),
        // [a_exp, a]
        Instruction::Dup(Some(1)),
        Instruction::Push(SIGN_MASK),
        Instruction::U32CheckedAnd,
        // [a_sign^, a_exp, a]
        Instruction::Dup(Some(2)),
        Instruction::Push(MANT_MASK),
        Instruction::U32CheckedAnd,
        // [a_mant, a_sign^, a_exp, a]
        Instruction::MovUp(3),
        Instruction::Drop,
        // [a_mant, a_sign^, a_exp]
    ]);
}

// [.., a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
//      ^ stack_offset
// ->
// [a_is_zero, b_is_zero, .., a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
pub(crate) fn add_is_zero(compiler: &mut Compiler, stack_offset: u32) {
    compiler.instructions.extend([
        Instruction::Dup(Some(stack_offset + 1)),
        Instruction::Push(0),
        Instruction::U32CheckedEq,
        // [b_exp == 0, ..]
        Instruction::Dup(Some(stack_offset + 6)),
        Instruction::Push(0),
        Instruction::U32CheckedEq,
        Instruction::U32CheckedAnd,
        // b is zero?
        // [b_exp == 0 & b_mant == 0, a_exp, b_exp, ..]
        Instruction::Dup(Some(stack_offset + 1)),
        Instruction::Push(0),
        Instruction::U32CheckedEq,
        // [a_exp == 0, b_is_zero, ..]
        Instruction::Dup(Some(stack_offset + 6)),
        Instruction::Push(0),
        Instruction::U32CheckedEq,
        Instruction::U32CheckedAnd,
        // a is zero?
        // [a_exp == 0 & a_mant == 0, b_is_inf, b_is_zero, ..]
    ]);
}

// [.., a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
//      ^ stack_offset
// ->
// [a_is_inf, b_is_inf, .., a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
pub(crate) fn add_is_inf(compiler: &mut Compiler, stack_offset: u32) {
    compiler.instructions.extend([
        Instruction::Dup(Some(stack_offset + 1)),
        Instruction::Push(0xff),
        Instruction::U32CheckedEq,
        // b is inf?
        // [b_exp == 0xff, ..]
        Instruction::Dup(Some(stack_offset + 1)),
        Instruction::Push(0xff),
        Instruction::U32CheckedEq,
        // a is inf?
        // [a_exp == 0xff, b_is_inf, ..]
    ]);
}

// [.., a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
//      ^ stack_offset
// ->
// [a_is_nan || b_is_nan, .., a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
pub(crate) fn add_is_nan(compiler: &mut Compiler, stack_offset: u32) {
    compiler.instructions.extend([
        Instruction::Dup(Some(stack_offset + 1)),
        // [b_exp, ..]
        Instruction::Push(EXP_MASK >> EXP_SHIFT),
        Instruction::U32CheckedEq,
        // [b_exp^ == EXP_MASK, ..]
        Instruction::Dup(Some(stack_offset + 6)),
        // [b_mant, b_exp^ == EXP_MASK, ..]
        Instruction::Push(0),
        Instruction::U32CheckedNeq,
        // [b_mant != 0, b_exp^ == EXP_MASK, ..]
        Instruction::U32CheckedAnd,
        // b is nan?
        // [b_mant != 0 & b_exp^ == EXP_MASK, ..]
        Instruction::Dup(Some(stack_offset + 1)),
        // [a_exp, ..]
        Instruction::Push(EXP_MASK >> EXP_SHIFT),
        Instruction::U32CheckedEq,
        // [a_exp^ == EXP_MASK, ..]
        Instruction::Dup(Some(stack_offset + 6)),
        // [a_mant, a_exp^ == EXP_MASK, ..]
        Instruction::Push(0),
        Instruction::U32CheckedNeq,
        // [a_mant != 0, a_exp^ == EXP_MASK, ..]
        Instruction::U32CheckedAnd,
        // a is nan?
        // [a_mant != 0 & a_exp^ == EXP_MASK, ..]
        Instruction::U32CheckedOr,
        // [a_is_nan | b_is_nan, ..]
    ]);
}

//                                                             [a_sign^, b_sign^, a_exp^, b_exp^, a_mant, b_mant]
// ->
// [a_is_nan || b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
pub(crate) fn add_nan_inf_zero(compiler: &mut Compiler) {
    add_is_zero(compiler, 0);
    // [a_is_zero, b_is_zero, ..]
    add_is_inf(compiler, 2);
    // [a_is_inf, b_is_inf, a_is_zero, b_is_zero, ..]
    add_is_nan(compiler, 4);
    // [a_is_nan || b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero, ..]
}

pub(crate) fn mul(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    add_nan_inf_zero(compiler);
    // [a_is_nan || b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            Instruction::Dup(Some(3)),
            Instruction::Dup(Some(3)),
            Instruction::U32CheckedAnd,
            Instruction::U32CheckedOr,
            // [a_is_nan || b_is_nan | a_is_zero & b_is_inf, ..]
            Instruction::Dup(Some(4)),
            Instruction::Dup(Some(2)),
            Instruction::U32CheckedAnd,
            Instruction::U32CheckedOr,
            // [a_is_nan || b_is_nan | a_is_zero & b_is_inf | b_is_zero & a_is_inf, ..]
        ],
        then: vec![
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Push(NAN),
        ],
        else_: vec![Instruction::If {
            condition: vec![
                Instruction::U32CheckedOr,
                // [b_is_inf | a_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
            ],
            then: vec![
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::U32CheckedXOR,
                Instruction::MovDown(2),
                // [a_mant, b_mant, sign_result]
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Push(INFINITY),
                Instruction::U32CheckedOr,
                // [sign_result | INFINITY]
            ],
            else_: vec![Instruction::If {
                condition: vec![
                    Instruction::U32CheckedOr,
                    // [a_is_zero | b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                ],
                then: vec![
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::MovDown(2),
                    // [a_mant, b_mant, a_sign^, b_sign^]
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::U32CheckedXOR,
                    // [a_sign^ ^ b_sign^]
                ],
                else_: vec![
                    Instruction::U32CheckedAdd,
                    Instruction::Push(EXP_BIAS),
                    Instruction::U32WrappingSub,
                    // exp_result
                    // [a_exp + b_exp - EXP_BIAS, a_sign^, b_sign^, a_mant, b_mant]
                    Instruction::MovUp(4),
                    Instruction::Push(LEADING_ONE_BIT),
                    Instruction::U32CheckedAdd,
                    Instruction::MovUp(4),
                    Instruction::Push(LEADING_ONE_BIT),
                    Instruction::U32CheckedAdd,
                    // [a_mant | LEADING_ONE_BIT, b_mant | LEADING_ONE_BIT, exp_result, a_sign^, b_sign^]
                    Instruction::U32OverflowingMul,
                    Instruction::U32CheckedSHL(Some(9)),
                    Instruction::Swap,
                    Instruction::U32CheckedSHR(Some(23)),
                    Instruction::U32WrappingAdd,
                    // mant_result
                    // [((a_mant | LEADING_ONE_BIT) * (b_mant | LEADING_ONE_BIT)) >> 23, exp_result, a_sign^, b_sign^]
                    Instruction::If {
                        condition: vec![
                            Instruction::Dup(None),
                            Instruction::Push(0x0100_0000),
                            Instruction::U32CheckedAnd,
                            // [mant_result & 0x0100_0000, mant_result, exp_result, a_sign^, b_sign^]
                            Instruction::Push(0),
                            Instruction::U32CheckedNeq,
                            // mant_result & 0x0100_0000 != 0
                        ],
                        then: vec![
                            Instruction::U32CheckedSHR(Some(1)),
                            Instruction::Swap,
                            Instruction::Push(1),
                            Instruction::U32CheckedAdd,
                            // [exp_result + 1, mant_result >> 1, a_sign^, b_sign^]
                        ],
                        else_: vec![Instruction::Swap],
                    },
                    // [exp_result, mant_result, a_sign^, b_sign^]
                    Instruction::If {
                        condition: vec![
                            Instruction::Dup(None),
                            Instruction::Push(SIGN_MASK),
                            Instruction::U32CheckedAnd,
                            Instruction::Push(SIGN_MASK),
                            Instruction::U32CheckedEq,
                            // int32(exp_result) < 0
                        ],
                        then: vec![
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::U32CheckedXOR,
                            // [a_sign^ ^ b_sign^]
                        ],
                        else_: vec![Instruction::If {
                            condition: vec![
                                Instruction::Dup(None),
                                Instruction::Push(0xff),
                                Instruction::U32CheckedGTE,
                                // mant_result >= 0xff
                            ],
                            then: vec![
                                Instruction::Drop,
                                Instruction::Drop,
                                Instruction::U32CheckedXOR,
                                // sign_result
                                // [a_sign^ ^ b_sign^]
                                Instruction::Push(INFINITY),
                                Instruction::U32CheckedAdd,
                            ],
                            else_: vec![
                                Instruction::U32CheckedSHL(Some(23)),
                                // [exp_result^, mant_result, a_sign^ ^ b_sign^]
                                Instruction::Swap,
                                // [mant_result, exp_result^, a_sign^ ^ b_sign^]
                                Instruction::Push(MANT_MASK),
                                Instruction::U32CheckedAnd,
                                Instruction::U32CheckedAdd,
                                // [mant_result & MANT_MASK + exp_result^, a_sign^ ^ b_sign^]
                                Instruction::MovDown(2),
                                Instruction::U32CheckedXOR,
                                Instruction::U32CheckedAdd,
                                // [sign_result + mant_result & MANT_MASK + exp_result^]
                            ],
                        }],
                    },
                ],
            }],
        }],
    }]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn div(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    add_nan_inf_zero(compiler);
    // [a_is_nan || b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            Instruction::Dup(Some(2)),
            Instruction::Dup(Some(2)),
            Instruction::U32CheckedAnd,
            Instruction::U32CheckedOr,
            // [a_is_nan || b_is_nan | a_is_inf & b_is_inf]
            Instruction::Dup(Some(4)),
            Instruction::Dup(Some(4)),
            Instruction::U32CheckedAnd,
            Instruction::U32CheckedOr,
            // [a_is_nan || b_is_nan | a_is_inf & b_is_inf | a_is_zero & b_is_zero]
        ],
        then: vec![
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Push(NAN),
        ],
        else_: vec![Instruction::If {
            condition: vec![
                Instruction::MovUp(3),
                Instruction::U32CheckedOr,
                // [a_is_inf | b_is_zero, b_is_inf, a_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
            ],
            then: vec![
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::U32CheckedXOR,
                Instruction::MovDown(2),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Push(INFINITY),
                Instruction::U32CheckedAdd,
                // a_sign^ ^ b_sign^ + INFINITY
            ],
            else_: vec![Instruction::If {
                condition: vec![Instruction::U32CheckedOr],
                then: vec![
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::U32CheckedXOR,
                    Instruction::MovDown(2),
                    Instruction::Drop,
                    Instruction::Drop,
                    // a_sign^ ^ b_sign^
                ],
                else_: vec![
                    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                    Instruction::Push(EXP_BIAS),
                    Instruction::U32CheckedAdd,
                    Instruction::Swap,
                    Instruction::U32WrappingSub,
                    // exp_result
                    // [a_exp + EXP_BIAS - b_exp, a_sign^, b_sign^, a_mant, b_mant]
                    Instruction::MovDown(4),
                    Instruction::U32CheckedXOR,
                    Instruction::MovDown(3),
                    // [a_mant, b_mant, exp_result, sign_result]
                    Instruction::Push(LEADING_ONE_BIT),
                    Instruction::U32CheckedAdd,
                    Instruction::U32CheckedSHL(Some(8)),
                    Instruction::Swap,
                    Instruction::Push(LEADING_ONE_BIT),
                    Instruction::U32CheckedAdd,
                    Instruction::U32CheckedSHL(Some(8)),
                    Instruction::Push(0),
                    // mant_result, divisor, remainder
                    // [0, b_mant | LEADING_ONE_BIT, a_mant | LEADING_ONE_BIT, exp_result, sign_result]
                    Instruction::Repeat {
                        count: 24,
                        instructions: vec![
                            Instruction::U32CheckedSHL(Some(1)),
                            Instruction::If {
                                condition: vec![
                                    Instruction::Dup(Some(2)),
                                    Instruction::Dup(Some(2)),
                                    Instruction::U32CheckedGTE,
                                    // divisor >= remainder
                                ],
                                then: vec![
                                    Instruction::MovUp(2),
                                    Instruction::Dup(Some(2)),
                                    Instruction::U32CheckedSub,
                                    // [remainder - divisor, mant_result, divisor, ..]
                                    Instruction::MovDown(2),
                                    Instruction::Push(1),
                                    Instruction::U32CheckedAdd,
                                    // [mant_result, divisor, remainder - divisor, ..]
                                ],
                                else_: vec![],
                            },
                            Instruction::Swap,
                            Instruction::U32CheckedSHR(Some(1)),
                            Instruction::Swap,
                            // [mant_result, divisor >> 1, remainder, ..]
                        ],
                    },
                    Instruction::MovDown(2),
                    Instruction::Drop,
                    Instruction::Drop,
                    // [mant_result, exp_result, sign_result]
                    Instruction::While {
                        condition: vec![
                            Instruction::Dup(Some(0)),
                            Instruction::Push(LEADING_ONE_BIT),
                            Instruction::U32CheckedAnd,
                            Instruction::Push(LEADING_ONE_BIT),
                            Instruction::U32CheckedNeq,
                            // mant_result & LEADING_ONE_BIT != LEADING_ONE_BIT
                        ],
                        body: vec![
                            Instruction::U32CheckedSHL(Some(1)),
                            Instruction::Swap,
                            Instruction::Push(1),
                            Instruction::U32WrappingSub,
                            Instruction::Swap,
                            // [mant_result << 1, exp_result - 1, remainder, ..]
                        ],
                    },
                    Instruction::Push(MANT_MASK),
                    Instruction::U32CheckedAnd,
                    Instruction::Swap,
                    // [exp_result, mant_result & MANT_MASK, sign_result]
                    Instruction::If {
                        condition: vec![
                            Instruction::Dup(Some(0)),
                            Instruction::Push(SIGN_MASK),
                            Instruction::U32CheckedAnd,
                            Instruction::Push(SIGN_MASK),
                            Instruction::U32CheckedEq,
                            // exp_result & SIGN_MASK == SIGN_MASK
                        ],
                        then: vec![
                            Instruction::Drop,
                            Instruction::Drop,
                            // sign_result
                        ],
                        else_: vec![Instruction::If {
                            condition: vec![
                                Instruction::Dup(Some(0)),
                                Instruction::Push(0xff),
                                Instruction::U32CheckedGTE,
                                // exp_result >= 0xff
                            ],
                            then: vec![
                                Instruction::Drop,
                                Instruction::Drop,
                                Instruction::Push(INFINITY),
                                Instruction::U32CheckedOr,
                                // sign_result + INFINITY
                            ],
                            else_: vec![
                                Instruction::U32CheckedSHL(Some(23)),
                                Instruction::U32CheckedOr,
                                Instruction::U32CheckedOr,
                            ],
                        }],
                    },
                ],
            }],
        }],
    }]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

// [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
// ->
// [a + b]
fn add_impl(compiler: &mut Compiler) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    add_nan_inf_zero(compiler);
    // [a_is_nan || b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            Instruction::Dup(Some(2)),
            Instruction::Dup(Some(2)),
            Instruction::U32CheckedAnd,
            // [a_is_inf & b_is_inf, ..]
            Instruction::Dup(Some(9)),
            Instruction::Dup(Some(9)),
            Instruction::U32CheckedNeq,
            Instruction::U32CheckedAnd,
            // [a_sign^ != b_sign^ & a_is_inf & b_is_inf, ..]
            Instruction::U32CheckedOr,
            // [a_sign^ != b_sign^ & a_is_inf & b_is_inf | a_is_nan || b_is_nan]
        ],
        then: vec![
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Push(NAN),
        ],
        else_: vec![Instruction::If {
            condition: vec![
                // [a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
            ],
            then: vec![
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::MovDown(3),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                // [a_sign^]
                Instruction::Push(INFINITY),
                Instruction::U32CheckedOr,
            ],
            else_: vec![Instruction::If {
                condition: vec![
                    // [b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                ],
                then: vec![
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::MovDown(2),
                    Instruction::Drop,
                    Instruction::Drop,
                    // [b_sign^]
                    Instruction::Push(INFINITY),
                    Instruction::U32CheckedOr,
                ],
                else_: vec![Instruction::If {
                    condition: vec![
                        // [a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                    ],
                    then: vec![
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::U32CheckedSHL(Some(23)),
                        Instruction::MovDown(4),
                        // [a_sign^, b_sign^, a_mant, b_mant, b_exp^]
                        Instruction::Drop,
                        Instruction::Swap,
                        Instruction::Drop,
                        Instruction::U32CheckedOr,
                        Instruction::U32CheckedOr,
                        // [b]
                    ],
                    else_: vec![Instruction::If {
                        condition: vec![
                            // [b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                        ],
                        then: vec![
                            Instruction::U32CheckedSHL(Some(23)),
                            Instruction::MovDown(3),
                            // [b_exp, a_sign^, b_sign^, a_exp^, a_mant, b_mant]
                            Instruction::Drop,
                            Instruction::Swap,
                            Instruction::Drop,
                            // [a_sign^, a_exp^, a_mant, b_mant]
                            Instruction::U32CheckedOr,
                            Instruction::U32CheckedOr,
                            Instruction::Swap,
                            Instruction::Drop,
                        ],
                        // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                        else_: vec![
                            Instruction::If {
                                condition: vec![
                                    Instruction::Dup(Some(1)),
                                    Instruction::Dup(Some(1)),
                                    Instruction::U32CheckedGT,
                                    // [b_exp > a_exp]
                                ],
                                then: vec![
                                    Instruction::MovDown(5),
                                    // [b_exp, a_sign^, b_sign^, a_mant, b_mant, a_exp]
                                    Instruction::MovDown(4),
                                    // [a_sign^, b_sign^, a_mant, b_mant, b_exp, a_exp]
                                    Instruction::MovDown(5),
                                    Instruction::MovDown(4),
                                    // [a_mant, b_mant, b_exp, a_exp, b_sign^, a_sign^]
                                    Instruction::MovDown(5),
                                    Instruction::MovDown(4),
                                    // a <-> b
                                    // [b_exp, a_exp, b_sign^, a_sign^, b_mant, a_mant]
                                ],
                                else_: vec![],
                            },
                            Instruction::If {
                                condition: vec![
                                    Instruction::Dup(Some(0)),
                                    Instruction::MovUp(2),
                                    // [b_exp, a_exp, a_exp, ..]
                                    Instruction::U32CheckedSub,
                                    // exp_diff
                                    // [a_exp - b_exp, a_exp, a_sign^, b_sign^, a_mant, b_mant]
                                    Instruction::Dup(Some(0)),
                                    Instruction::Push(24),
                                    Instruction::U32CheckedGT,
                                    // exp_diff > 24
                                ],
                                then: vec![
                                    Instruction::Drop,
                                    Instruction::U32CheckedSHL(Some(23)),
                                    Instruction::U32CheckedOr,
                                    Instruction::Swap,
                                    Instruction::Drop,
                                    Instruction::U32CheckedOr,
                                    Instruction::Swap,
                                    Instruction::Drop,
                                ],
                                else_: vec![
                                    // [exp_diff, a_exp, a_sign^, b_sign^, a_mant, b_mant]
                                    Instruction::Swap,
                                    Instruction::MovDown(5),
                                    // [exp_diff, a_sign^, b_sign^, a_mant, b_mant, a_exp]
                                    Instruction::MovUp(4),
                                    // [b_mant, exp_diff, a_sign^, b_sign^, a_mant, a_exp]
                                    Instruction::Push(LEADING_ONE_BIT),
                                    Instruction::U32CheckedOr,
                                    Instruction::Swap,
                                    Instruction::U32CheckedSHR(None),
                                    Instruction::If {
                                        condition: vec![
                                            Instruction::Dup(Some(0)),
                                            Instruction::Push(0),
                                            Instruction::U32CheckedEq,
                                        ],
                                        then: vec![Instruction::Push(1), Instruction::U32CheckedOr],
                                        else_: vec![],
                                    },
                                    Instruction::MovDown(3),
                                    Instruction::MovUp(2),
                                    Instruction::Push(LEADING_ONE_BIT),
                                    Instruction::U32CheckedOr,
                                    Instruction::MovDown(2),
                                    // [a_sign^, b_sign^, a_mant, b_mant, a_exp]
                                    Instruction::If {
                                        condition: vec![
                                            Instruction::Dup(Some(1)),
                                            Instruction::Dup(Some(1)),
                                            Instruction::U32CheckedEq,
                                            // a_sign == b_sign
                                        ],
                                        then: vec![
                                            Instruction::MovDown(3),
                                            Instruction::Drop,
                                            Instruction::U32CheckedAdd,
                                            // sum
                                            // [a_mant + b_mant, a_sign, a_exp]
                                        ],
                                        else_: vec![Instruction::If {
                                            condition: vec![
                                                Instruction::Dup(Some(3)),
                                                Instruction::Dup(Some(3)),
                                                Instruction::U32CheckedLTE,
                                                // b_mant <= a_mant
                                            ],
                                            then: vec![
                                                Instruction::MovDown(3),
                                                Instruction::Drop,
                                                Instruction::Swap,
                                                Instruction::U32CheckedSub,
                                                // sum
                                                // [a_mant - b_mant, a_sign, a_exp]
                                            ],
                                            else_: vec![
                                                Instruction::Drop,
                                                Instruction::MovDown(2),
                                                Instruction::U32CheckedSub,
                                                // sum
                                                // [a_mant - b_mant, b_sign, a_exp]
                                            ],
                                        }],
                                    },
                                    // [mant_result, sign_result, a_exp]
                                    Instruction::If {
                                        condition: vec![
                                            Instruction::Dup(Some(0)),
                                            Instruction::Push(0),
                                            Instruction::U32CheckedEq,
                                        ],
                                        then: vec![
                                            Instruction::Drop,
                                            Instruction::Drop,
                                            Instruction::Drop,
                                            Instruction::Push(0),
                                        ],
                                        else_: vec![
                                            Instruction::Push(32),
                                            Instruction::Dup(Some(1)),
                                            // [mant_result, clz, mant_result, sign_result, a_exp]
                                            Instruction::While {
                                                condition: vec![
                                                    Instruction::Dup(Some(0)),
                                                    Instruction::Push(0),
                                                    Instruction::U32CheckedNeq,
                                                    // mant_result != 0
                                                ],
                                                body: vec![
                                                    Instruction::U32CheckedSHR(Some(1)),
                                                    Instruction::Swap,
                                                    Instruction::Push(1),
                                                    Instruction::U32CheckedSub,
                                                    Instruction::Swap,
                                                    // [mant_result >> 1, clz - 1, ..]
                                                ],
                                            },
                                            Instruction::Drop,
                                            Instruction::If {
                                                condition: vec![
                                                    Instruction::Dup(Some(0)),
                                                    Instruction::Push(8),
                                                    Instruction::U32CheckedLTE,
                                                    // clz <= 8
                                                ],
                                                then: vec![
                                                    Instruction::Push(8),
                                                    Instruction::Swap,
                                                    Instruction::U32CheckedSub,
                                                    // extra_exp
                                                    // [8 - clz, mant_result, sign_result, a_exp]
                                                    Instruction::Swap,
                                                    Instruction::Dup(Some(1)),
                                                    Instruction::U32CheckedSHR(None),
                                                    // [mant_result >> extra_exp, extra_exp, sign_result, a_exp]
                                                    Instruction::Push(MANT_MASK),
                                                    Instruction::U32CheckedAnd,
                                                    Instruction::Swap,
                                                    Instruction::MovUp(3),
                                                    Instruction::U32CheckedAdd,
                                                    // exp_result, mant_result
                                                    // [a_exp + extra_exp, mant_result >> extra_exp & MANT_MASK, sign_result]
                                                ],
                                                else_: vec![
                                                    Instruction::Push(8),
                                                    Instruction::U32CheckedSub,
                                                    // missing_exp
                                                    // [clz - 8, mant_result, sign_result, a_exp]
                                                    Instruction::Swap,
                                                    Instruction::Dup(Some(1)),
                                                    Instruction::U32CheckedSHL(None),
                                                    // [mant_result << missing_exp, missing_exp, sign_result, a_exp]
                                                    Instruction::Push(MANT_MASK),
                                                    Instruction::U32CheckedAnd,
                                                    Instruction::Swap,
                                                    Instruction::MovUp(3),
                                                    Instruction::Swap,
                                                    Instruction::U32WrappingSub,
                                                    // exp_result, mant_result
                                                    // [a_exp + missing_exp, mant_result << missing_exp & MANT_MASK, sign_result]
                                                ],
                                            },
                                            // [exp_result, mant_result & MANT_MASK, sign_result]
                                            Instruction::If {
                                                condition: vec![
                                                    Instruction::Dup(Some(0)),
                                                    Instruction::Push(SIGN_MASK),
                                                    Instruction::U32CheckedAnd,
                                                    Instruction::Push(SIGN_MASK),
                                                    Instruction::U32CheckedEq,
                                                    // exp_result & SIGN_MASK == SIGN_MASK
                                                ],
                                                then: vec![
                                                    Instruction::Drop,
                                                    Instruction::Drop,
                                                    // sign_result
                                                ],
                                                else_: vec![Instruction::If {
                                                    condition: vec![
                                                        Instruction::Dup(Some(0)),
                                                        Instruction::Push(0xff),
                                                        Instruction::U32CheckedGTE,
                                                        // exp_result >= 0xff
                                                    ],
                                                    then: vec![
                                                        Instruction::Drop,
                                                        Instruction::Drop,
                                                        Instruction::Push(INFINITY),
                                                        Instruction::U32CheckedOr,
                                                        // sign_result + INFINITY
                                                    ],
                                                    else_: vec![
                                                        Instruction::U32CheckedSHL(Some(23)),
                                                        Instruction::U32CheckedOr,
                                                        Instruction::U32CheckedOr,
                                                    ],
                                                }],
                                            },
                                        ],
                                    },
                                ],
                            },
                        ],
                    }],
                }],
            }],
        }],
    }]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn add(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    add_impl(compiler)
}

pub(crate) fn sub(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    compiler.instructions.extend([
        Instruction::MovUp(3),
        Instruction::Push(SIGN_MASK),
        Instruction::U32CheckedXOR,
        Instruction::MovDown(3),
    ]);

    add_impl(compiler)
}

pub(crate) fn eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    add_is_zero(compiler, 0);
    // [a_is_zero, b_is_zero, ..]

    compiler.instructions.push(Instruction::U32CheckedAnd);

    add_is_nan(compiler, 4);
    // [a_is_nan || b_is_nan, a_is_zero && b_is_zero, ..]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            // [a_is_nan || b_is_nan]
        ],
        then: vec![
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Push(0),
        ],
        else_: vec![Instruction::If {
            condition: vec![
                // [a_is_zero && b_is_zero]
            ],
            then: vec![
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Push(1),
            ],
            else_: vec![
                // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                Instruction::U32CheckedEq,
                Instruction::MovDown(4),
                Instruction::U32CheckedEq,
                Instruction::Swap,
                Instruction::U32CheckedEq,
                // [a_mant == b_mant, a_sign^ == b_sign^, a_exp == b_exp]
                Instruction::U32CheckedAnd,
                Instruction::U32CheckedAnd,
            ],
        }],
    }]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn ne(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    add_is_zero(compiler, 0);
    // [a_is_zero, b_is_zero, ..]

    compiler.instructions.push(Instruction::U32CheckedAnd);

    add_is_nan(compiler, 4);
    // [a_is_nan || b_is_nan, a_is_zero && b_is_zero, ..]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            Instruction::U32CheckedOr,
            // [a_is_nan || b_is_nan | a_is_zero && b_is_zero]
        ],
        then: vec![
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Push(0),
        ],
        else_: vec![
            // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
            Instruction::U32CheckedNeq,
            Instruction::MovDown(4),
            Instruction::U32CheckedNeq,
            Instruction::MovDown(2),
            Instruction::U32CheckedNeq,
            // [a_mant == b_mant, a_sign^ == b_sign^, a_exp == b_exp]
            Instruction::U32CheckedOr,
            Instruction::U32CheckedOr,
        ],
    }]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn lt(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    add_nan_inf_zero(compiler);
    // [a_is_nan || b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            Instruction::MovUp(4),
            Instruction::MovUp(4),
            Instruction::U32CheckedAnd,
            Instruction::U32CheckedOr,
            // [a_is_zero & b_is_zero | a_is_nan || b_is_nan]
        ],
        then: vec![
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Push(0),
        ],
        // [a_is_inf, b_is_inf, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
        else_: vec![Instruction::If {
            condition: vec![
                // a_is_inf
            ],
            then: vec![
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::MovDown(3),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Push(0),
                Instruction::U32CheckedNeq,
                // a_sign^ != 0
            ],
            else_: vec![Instruction::If {
                condition: vec![
                    // b_is_inf
                ],
                then: vec![
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::MovDown(2),
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Push(0),
                    Instruction::U32CheckedEq,
                    // b_sign^ == 0
                ],
                else_: vec![
                    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                    Instruction::U32CheckedSHL(Some(EXP_SHIFT)),
                    Instruction::MovDown(3),
                    // [b_exp, a_sign^, b_sign^, a_mant, a_exp^, b_mant]
                    Instruction::U32CheckedSHL(Some(EXP_SHIFT)),
                    Instruction::MovDown(5),
                    // [a_sign^, b_sign^, a_mant, a_exp^, b_mant, b_exp^]
                    Instruction::Dup(Some(0)),
                    Instruction::Push(0),
                    Instruction::U32CheckedNeq,
                    Instruction::MovDown(6),
                    // [a_sign^, b_sign^, a_mant, a_exp^, b_mant, b_exp^, a_sign^ != 0]
                    Instruction::If {
                        condition: vec![
                            Instruction::U32CheckedEq,
                            // a_sign^ == b_sign^
                        ],
                        then: vec![
                            Instruction::U32CheckedOr,
                            Instruction::MovDown(2),
                            Instruction::U32CheckedOr,
                            // [b_mant | b_exp^, a_mant | a_exp^, a_sign^ != 0]
                            Instruction::U32CheckedLT,
                            // a < b
                            Instruction::U32CheckedXOR,
                            // a_sign^ != 0 ^ a < b
                        ],
                        else_: vec![
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                        ],
                    },
                ],
            }],
        }],
    }]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn lte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    add_nan_inf_zero(compiler);
    // [a_is_nan || b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            // [a_is_nan || b_is_nan]
        ],
        then: vec![
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Push(0),
        ],
        // [a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
        else_: vec![Instruction::If {
            condition: vec![
                Instruction::Dup(Some(5)),
                Instruction::Dup(Some(5)),
                Instruction::U32CheckedEq,
                // [a_exp == b_exp]
                Instruction::Dup(Some(8)),
                Instruction::Dup(Some(8)),
                Instruction::U32CheckedEq,
                Instruction::U32CheckedAnd,
                // [a_sign^ == b_sign^ & a_exp == b_exp]
                Instruction::Dup(Some(10)),
                Instruction::Dup(Some(10)),
                Instruction::U32CheckedEq,
                Instruction::U32CheckedAnd,
                // [a_mant == b_mant & a_sign^ == b_sign^ & a_exp == b_exp]
            ],
            then: vec![
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Push(1),
            ],
            else_: vec![Instruction::If {
                condition: vec![
                    // a_is_inf
                ],
                then: vec![
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::MovDown(3),
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Push(0),
                    Instruction::U32CheckedNeq,
                    // a_sign^ != 0
                ],
                else_: vec![Instruction::If {
                    condition: vec![
                        // b_is_inf
                    ],
                    then: vec![
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::MovDown(2),
                        // [a_mant, b_mant, b_sign^]
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Push(0),
                        Instruction::U32CheckedEq,
                        // b_sign^ == 0
                    ],
                    else_: vec![Instruction::If {
                        // [a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                        condition: vec![
                            Instruction::U32CheckedAnd,
                            // a_is_zero && b_is_zero
                        ],
                        then: vec![
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Push(1),
                        ],
                        else_: vec![
                            // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                            Instruction::U32CheckedSHL(Some(EXP_SHIFT)),
                            Instruction::MovDown(3),
                            // [b_exp, a_sign^, b_sign^, a_mant, a_exp^, b_mant]
                            Instruction::U32CheckedSHL(Some(EXP_SHIFT)),
                            Instruction::MovDown(5),
                            // [a_sign^, b_sign^, a_mant, a_exp^, b_mant, b_exp^]
                            Instruction::Dup(Some(0)),
                            Instruction::Push(0),
                            Instruction::U32CheckedNeq,
                            Instruction::MovDown(6),
                            // [a_sign^, b_sign^, a_mant, a_exp^, b_mant, b_exp^, a_sign^ != 0]
                            Instruction::If {
                                condition: vec![
                                    Instruction::U32CheckedEq,
                                    // a_sign^ == b_sign^
                                ],
                                then: vec![
                                    Instruction::U32CheckedOr,
                                    Instruction::MovDown(2),
                                    Instruction::U32CheckedOr,
                                    // [b_mant | b_exp^, a_mant | a_exp^, a_sign^ != 0]
                                    Instruction::U32CheckedLT,
                                    // a < b
                                    Instruction::U32CheckedXOR,
                                    // a_sign^ != 0 ^ a < b
                                ],
                                else_: vec![
                                    Instruction::Drop,
                                    Instruction::Drop,
                                    Instruction::Drop,
                                    Instruction::Drop,
                                ],
                            },
                        ],
                    }],
                }],
            }],
        }],
    }]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn gt(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    add_nan_inf_zero(compiler);
    // [a_is_nan || b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            Instruction::MovUp(4),
            Instruction::MovUp(4),
            Instruction::U32CheckedAnd,
            Instruction::U32CheckedOr,
            // [a_is_zero & b_is_zero | a_is_nan || b_is_nan]
        ],
        then: vec![
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Push(0),
        ],
        // [a_is_inf, b_is_inf, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
        else_: vec![Instruction::If {
            condition: vec![
                // a_is_inf
            ],
            then: vec![
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::MovDown(3),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Push(0),
                Instruction::U32CheckedEq,
                // a_sign^ == 0
            ],
            else_: vec![Instruction::If {
                condition: vec![
                    // b_is_inf
                ],
                then: vec![
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::MovDown(2),
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Push(0),
                    Instruction::U32CheckedNeq,
                    // b_sign^ != 0
                ],
                else_: vec![
                    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                    Instruction::U32CheckedSHL(Some(EXP_SHIFT)),
                    Instruction::MovDown(3),
                    // [b_exp, a_sign^, b_sign^, a_mant, a_exp^, b_mant]
                    Instruction::U32CheckedSHL(Some(EXP_SHIFT)),
                    Instruction::MovDown(5),
                    // [a_sign^, b_sign^, a_mant, a_exp^, b_mant, b_exp^]
                    Instruction::Dup(Some(0)),
                    Instruction::MovDown(6),
                    // [a_sign^, b_sign^, a_mant, a_exp^, b_mant, b_exp^, a_sign^ != 0]
                    Instruction::If {
                        condition: vec![
                            Instruction::U32CheckedEq,
                            // a_sign^ == b_sign^
                        ],
                        then: vec![
                            Instruction::U32CheckedOr,
                            Instruction::MovDown(2),
                            Instruction::U32CheckedOr,
                            // [b_mant | b_exp^, a_mant | a_exp^, a_sign^ != 0]
                            Instruction::U32CheckedGT,
                            // a_mant > b_mant
                            Instruction::Swap,
                            Instruction::Push(0),
                            Instruction::U32CheckedNeq,
                            Instruction::U32CheckedXOR,
                            // a_sign^ != 0 ^ b < a
                        ],
                        else_: vec![
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Push(0),
                            Instruction::U32CheckedEq,
                        ],
                    },
                ],
            }],
        }],
    }]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn gte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    add_nan_inf_zero(compiler);
    // [a_is_nan || b_is_nan, a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]

    compiler.instructions.extend([Instruction::If {
        condition: vec![
            // [a_is_nan || b_is_nan]
        ],
        then: vec![
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Drop,
            Instruction::Push(0),
        ],
        // [a_is_inf, b_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
        else_: vec![Instruction::If {
            condition: vec![
                Instruction::Dup(Some(5)),
                Instruction::Dup(Some(5)),
                Instruction::U32CheckedEq,
                // [a_exp == b_exp]
                Instruction::Dup(Some(8)),
                Instruction::Dup(Some(8)),
                Instruction::U32CheckedEq,
                Instruction::U32CheckedAnd,
                // [a_sign^ == b_sign^ & a_exp == b_exp]
                Instruction::Dup(Some(10)),
                Instruction::Dup(Some(10)),
                Instruction::U32CheckedEq,
                Instruction::U32CheckedAnd,
                // [a_mant == b_mant & a_sign^ == b_sign^ & a_exp == b_exp]
            ],
            then: vec![
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Push(1),
            ],
            else_: vec![Instruction::If {
                condition: vec![
                    // a_is_inf
                ],
                then: vec![
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::MovDown(3),
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Drop,
                    Instruction::Push(0),
                    Instruction::U32CheckedEq,
                    // a_sign^ == 0
                ],
                else_: vec![Instruction::If {
                    condition: vec![
                        // b_is_inf
                    ],
                    then: vec![
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::MovDown(2),
                        // [a_mant, b_mant, b_sign^]
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Push(0),
                        Instruction::U32CheckedNeq,
                        // b_sign^ == 0
                    ],
                    else_: vec![Instruction::If {
                        // [a_is_zero, b_is_zero, a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                        condition: vec![
                            Instruction::U32CheckedAnd,
                            // a_is_zero && b_is_zero
                        ],
                        then: vec![
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::Push(1),
                        ],
                        else_: vec![
                            // [a_exp, b_exp, a_sign^, b_sign^, a_mant, b_mant]
                            Instruction::U32CheckedSHL(Some(EXP_SHIFT)),
                            Instruction::MovDown(3),
                            // [b_exp, a_sign^, b_sign^, a_mant, a_exp^, b_mant]
                            Instruction::U32CheckedSHL(Some(EXP_SHIFT)),
                            Instruction::MovDown(5),
                            // [a_sign^, b_sign^, a_mant, a_exp^, b_mant, b_exp^]
                            Instruction::Dup(Some(0)),
                            Instruction::MovDown(6),
                            // [a_sign^, b_sign^, a_mant, a_exp^, b_mant, b_exp^, a_sign^]
                            Instruction::If {
                                condition: vec![
                                    Instruction::U32CheckedEq,
                                    // a_sign^ == b_sign^
                                ],
                                then: vec![
                                    Instruction::U32CheckedOr,
                                    Instruction::MovDown(2),
                                    Instruction::U32CheckedOr,
                                    // [b_mant | b_exp^, a_mant | a_exp^, a_sign^ != 0]
                                    Instruction::U32CheckedGT,
                                    // a > b
                                    Instruction::Swap,
                                    Instruction::Push(0),
                                    Instruction::U32CheckedNeq,
                                    Instruction::U32CheckedXOR,
                                    // a_sign^ != 0 ^ a < b
                                ],
                                else_: vec![
                                    Instruction::Drop,
                                    Instruction::Drop,
                                    Instruction::Drop,
                                    Instruction::Drop,
                                    Instruction::Push(0),
                                    Instruction::U32CheckedEq,
                                ],
                            },
                        ],
                    }],
                }],
            }],
        }],
    }]);

    compiler.memory.write(
        compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_custom() {
        assert_bin_op(f32::INFINITY, f32::MAX, Gt);
    }

    use itertools::Itertools;
    use quickcheck_macros::quickcheck;
    use test_case::test_case;

    use super::*;

    const TEST_RUST_NATIVE_F32_ACCURACY: f32 = 1e-6;
    const TEST_EDGE_CASES: &[f32] = &[
        0.0,
        -0.0,
        1.0,
        -1.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::MAX,
        f32::MIN,
        f32::MAX / 2.,
        f32::MIN / 2.,
        f32::EPSILON,
        -f32::EPSILON,
    ];

    fn new(compiler: &mut Compiler, value: f32) -> Symbol {
        let symbol = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

        compiler.memory.write(
            compiler.instructions,
            symbol.memory_addr,
            &[ValueSource::Immediate(value.to_bits())],
        );

        symbol
    }

    fn helper_bin_op(
        a: f32,
        b: f32,
        f: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol,
    ) -> Result<f32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);
        let b = new(&mut compiler, b);

        let result = f(&mut compiler, &a, &b);
        compiler
            .memory
            .read(compiler.instructions, result.memory_addr, WIDTH);

        let mut program = "begin\n".to_string();
        for instruction in &instructions {
            instruction
                .encode(unsafe { program.as_mut_vec() }, 1)
                .unwrap();
        }
        program.push_str("\nend\n");

        let outputs = miden::execute(
            &miden::Assembler::default().compile(&program).unwrap(),
            miden::StackInputs::default(),
            miden::MemAdviceProvider::default(),
        )?;

        let stack = outputs.stack_outputs().stack();

        dbg!(&stack);

        assert!(stack[1..].iter().all(|&x| x == 0));

        Ok(f32::from_bits(stack[0] as u32))
    }

    trait BinaryOp: Copy {
        const STR: &'static str;
        const RUST_FN: fn(f32, f32) -> f32;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol;

        const INACCURATE: bool = true;
    }

    #[derive(Clone, Copy)]
    struct Mul;
    impl BinaryOp for Mul {
        const STR: &'static str = "*";
        const RUST_FN: fn(f32, f32) -> f32 = <f32 as std::ops::Mul>::mul;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::mul;
    }

    #[derive(Clone, Copy)]
    struct Div;
    impl BinaryOp for Div {
        const STR: &'static str = "/";
        const RUST_FN: fn(f32, f32) -> f32 = <f32 as std::ops::Div>::div;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::div;
    }

    #[derive(Clone, Copy)]
    struct Add;
    impl BinaryOp for Add {
        const STR: &'static str = "+";
        const RUST_FN: fn(f32, f32) -> f32 = <f32 as std::ops::Add>::add;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::add;
    }

    #[derive(Clone, Copy)]
    struct Sub;
    impl BinaryOp for Sub {
        const STR: &'static str = "-";
        const RUST_FN: fn(f32, f32) -> f32 = <f32 as std::ops::Sub>::sub;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::sub;
    }

    fn eq(a: f32, b: f32) -> f32 {
        if a == b {
            f32::from_bits(1)
        } else {
            f32::from_bits(0)
        }
    }

    #[derive(Clone, Copy)]
    struct Eq;
    impl BinaryOp for Eq {
        const STR: &'static str = "==";
        const RUST_FN: fn(f32, f32) -> f32 = self::eq;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::eq;

        const INACCURATE: bool = false;
    }

    fn ne(a: f32, b: f32) -> f32 {
        if a != b {
            f32::from_bits(1)
        } else {
            f32::from_bits(0)
        }
    }

    #[derive(Clone, Copy)]
    struct Ne;
    impl BinaryOp for Ne {
        const STR: &'static str = "!=";
        const RUST_FN: fn(f32, f32) -> f32 = self::ne;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::ne;

        const INACCURATE: bool = false;
    }

    fn lt(a: f32, b: f32) -> f32 {
        if a < b {
            f32::from_bits(1)
        } else {
            f32::from_bits(0)
        }
    }

    #[derive(Clone, Copy)]
    struct Lt;
    impl BinaryOp for Lt {
        const STR: &'static str = "<";
        const RUST_FN: fn(f32, f32) -> f32 = self::lt;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::lt;

        const INACCURATE: bool = false;
    }

    fn lte(a: f32, b: f32) -> f32 {
        if a <= b {
            f32::from_bits(1)
        } else {
            f32::from_bits(0)
        }
    }

    #[derive(Clone, Copy)]
    struct Lte;
    impl BinaryOp for Lte {
        const STR: &'static str = "<=";
        const RUST_FN: fn(f32, f32) -> f32 = self::lte;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::lte;

        const INACCURATE: bool = false;
    }

    fn gt(a: f32, b: f32) -> f32 {
        if a > b {
            f32::from_bits(1)
        } else {
            f32::from_bits(0)
        }
    }

    #[derive(Clone, Copy)]
    struct Gt;
    impl BinaryOp for Gt {
        const STR: &'static str = ">";
        const RUST_FN: fn(f32, f32) -> f32 = self::gt;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::gt;

        const INACCURATE: bool = false;
    }

    fn gte(a: f32, b: f32) -> f32 {
        if a >= b {
            f32::from_bits(1)
        } else {
            f32::from_bits(0)
        }
    }

    #[derive(Clone, Copy)]
    struct Gte;
    impl BinaryOp for Gte {
        const STR: &'static str = ">=";
        const RUST_FN: fn(f32, f32) -> f32 = self::gte;
        const VM_FN: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol = super::gte;

        const INACCURATE: bool = false;
    }

    fn assert_bin_op<T: BinaryOp>(a: f32, b: f32, _bin_op: T) {
        println!("running for {a} {} {b}", T::STR);
        let expected = T::RUST_FN(a, b);
        let result = helper_bin_op(a, b, T::VM_FN).unwrap();
        assert!(
            result.is_nan() && expected.is_nan()
                || result == expected
                || if T::INACCURATE {
                    (result - expected).abs()
                        <= TEST_RUST_NATIVE_F32_ACCURACY * (1.0 + expected.abs())
                } else {
                    result == expected
                },
            "{a} {} {b}, result: {result}, expected: {expected}",
            T::STR
        );
    }

    #[test_case(Mul; "mul")]
    #[test_case(Div; "div")]
    #[test_case(Add; "add")]
    #[test_case(Sub; "sub")]
    #[test_case(Eq; "eq")]
    #[test_case(Ne; "ne")]
    #[test_case(Lt; "lt")]
    #[test_case(Lte; "lte")]
    #[test_case(Gt; "gt")]
    #[test_case(Gte; "gte")]
    fn test_edge_cases(bin_op: impl BinaryOp) {
        TEST_EDGE_CASES
            .iter()
            .copied()
            .permutations(2)
            .for_each(|x| assert_bin_op(x[0], x[1], bin_op));
    }

    #[quickcheck]
    fn test_mul(a: f32, b: f32) {
        assert_bin_op(a, b, Mul)
    }

    #[quickcheck]
    fn test_div(a: f32, b: f32) {
        assert_bin_op(a, b, Div)
    }

    #[quickcheck]
    fn test_add(a: f32, b: f32) {
        assert_bin_op(a, b, Add)
    }

    #[quickcheck]
    fn test_sub(a: f32, b: f32) {
        assert_bin_op(a, b, Sub)
    }

    #[quickcheck]
    fn test_eq(a: f32, b: f32) {
        assert_bin_op(a, b, Eq)
    }

    #[quickcheck]
    fn test_ne(a: f32, b: f32) {
        assert_bin_op(a, b, Ne)
    }

    #[quickcheck]
    fn test_lt(a: f32, b: f32) {
        assert_bin_op(a, b, Lt)
    }

    #[quickcheck]
    fn test_lte(a: f32, b: f32) {
        assert_bin_op(a, b, Lte)
    }

    #[quickcheck]
    fn test_gt(a: f32, b: f32) {
        assert_bin_op(a, b, Gt)
    }

    #[quickcheck]
    fn test_gte(a: f32, b: f32) {
        assert_bin_op(a, b, Gte)
    }
}
