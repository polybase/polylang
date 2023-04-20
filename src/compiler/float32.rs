/// Notation:
/// - x_sign - float sign bit of x
/// - x_exp  - float exponent of x
/// - x_mant - float mantissa of x
/// - z^     - float value without shifting, i.e x_exp^ = x_exp << 23; x_mant^ = x_mant << 0 = x_mant

#[allow(unused)]
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
        &mut compiler.instructions,
        symbol.memory_addr,
        &[ValueSource::Immediate(value.to_bits())],
    );

    symbol
}

// [a, b] -> [a_sign^, b_sign^, a_exp^, b_exp^, a_mant, b_mant]
fn prepare_stack_for_arithmetic(compiler: &mut Compiler, a: &Symbol, b: &Symbol) {
    compiler.memory.read(
        &mut compiler.instructions,
        a.memory_addr,
        a.type_.miden_width(),
    );
    decompose(compiler);
    // [a_sign^, a_exp^, a]
    compiler.memory.read(
        &mut compiler.instructions,
        b.memory_addr,
        b.type_.miden_width(),
    );
    decompose(compiler);
    // [b_sign^, b_exp^, b, a_sign^, a_exp^, a]

    compiler.instructions.push(Instruction::MovDown(3));
    // [b_exp^, b, a_sign^, b_sign^, a_exp^, a]
    compiler.instructions.push(Instruction::MovDown(4));
    // [b, a_sign^, b_sign^, a_exp^, b_exp^, a]
    compiler.instructions.push(Instruction::MovDown(5));
    // [a_sign^, b_sign^, a_exp^, b_exp^, a, b]
}

// [a] -> [a_sign^, a_exp^, a_mant]
fn decompose(compiler: &mut Compiler) {
    compiler.instructions.extend(
        [
            // [a]
            Instruction::Dup(None),
            // [a, a]
            Instruction::Push(MANT_MASK),
            Instruction::U32CheckedAnd,
            // [a_mant, a]
            Instruction::Dup(Some(1)),
            Instruction::Push(EXP_MASK),
            Instruction::U32CheckedAnd,
            // [a_exp^, a_mant, a]
            Instruction::Dup(Some(2)),
            Instruction::Push(SIGN_MASK),
            Instruction::U32CheckedAnd,
            // [a_sign^, a_exp^, a_mant, a]
            Instruction::MovUp(3),
            Instruction::Drop,
            // [a_sign^, a_exp^, a_mant]
        ]
        .into_iter(),
    );
}

pub(crate) fn mul(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [a_sign^, b_sign^, a_exp^, b_exp^, a_mant, b_mant]

    compiler.instructions.extend(
        [
            Instruction::U32CheckedXOR,
            // sign_result
            // [a_sign^ ^ b_sign^, ..]
            Instruction::MovUp(2),
            Instruction::U32CheckedSHR(Some(EXP_SHIFT)),
            // [b_exp, sign_result, a_exp^, a_mant, b_mant]
            Instruction::MovUp(2),
            Instruction::U32CheckedSHR(Some(EXP_SHIFT)),
            // [a_exp, b_exp, sign_result, a_mant, b_mant]
            Instruction::Dup(Some(1)),
            Instruction::Push(0),
            Instruction::U32CheckedEq,
            // [b_exp == 0, ..]
            Instruction::Dup(Some(5)),
            Instruction::Push(0),
            Instruction::U32CheckedEq,
            Instruction::U32CheckedAnd,
            // b is zero?
            // [b_exp == 0 & b_mant == 0, a_exp, b_exp, ..]
            Instruction::Dup(Some(2)),
            Instruction::Push(0xff),
            Instruction::U32CheckedEq,
            // b is inf?
            // [b_exp == 0xff, b_is_zero, a_exp,..]
            Instruction::Dup(Some(2)),
            Instruction::Push(0),
            Instruction::U32CheckedEq,
            // [a_exp == 0, b_is_inf, b_is_zero, ..]
            Instruction::Dup(Some(6)),
            Instruction::Push(0),
            Instruction::U32CheckedEq,
            Instruction::U32CheckedAnd,
            // a is zero?
            // [a_exp == 0 & a_mant == 0, b_is_inf, b_is_zero, ..]
            Instruction::Dup(Some(3)),
            Instruction::Push(0xff),
            Instruction::U32CheckedEq,
            // a is inf?
            // [a_exp == 0xff, a_is_zero, b_is_inf, b_is_zero, ..]
            Instruction::Dup(Some(0)),
            // [a_is_inf, a_is_inf, a_is_zero, b_is_inf, b_is_zero, ..]
            Instruction::Dup(Some(4)),
            // [b_is_zero, a_is_inf, a_is_inf, a_is_zero, b_is_inf, b_is_zero, ..]
            Instruction::U32CheckedAnd,
            // [b_is_zero & a_is_inf, a_is_inf, a_is_zero, b_is_inf, b_is_zero, ..]
            Instruction::Dup(Some(3)),
            // [b_is_inf, b_is_zero & a_is_inf, a_is_inf, a_is_zero, b_is_inf, b_is_zero, ..]
            Instruction::Dup(Some(3)),
            // [a_is_zero, b_is_inf, b_is_zero & a_is_inf, a_is_inf, a_is_zero, b_is_inf, b_is_zero, ..]
            Instruction::U32CheckedAnd,
            // [a_is_zero & b_is_inf, b_is_zero & a_is_inf, a_is_inf, a_is_zero, b_is_inf, b_is_zero, ..]
            Instruction::U32CheckedOr,
            // [a_is_zero & b_is_inf | b_is_zero & a_is_inf, a_is_inf, a_is_zero, b_is_inf, b_is_zero, a_exp, b_exp, ..]
            Instruction::Dup(Some(6)),
            // [b_exp, ..]
            Instruction::Push(EXP_MASK >> EXP_SHIFT),
            Instruction::U32CheckedEq,
            // [b_exp == EXP_MASK, ..]
            Instruction::Dup(Some(10)),
            // [b_mant, b_exp == EXP_MASK, ..]
            Instruction::Push(0),
            Instruction::U32CheckedNeq,
            // [b_mant != 0, b_exp == EXP_MASK, ..]
            Instruction::U32CheckedAnd,
            // b is nan?
            // [b_mant != 0 & b_exp == EXP_MASK, ..]
            Instruction::Dup(Some(6)),
            // [a_exp, ..]
            Instruction::Push(EXP_MASK >> EXP_SHIFT),
            Instruction::U32CheckedEq,
            // [a_exp == EXP_MASK, ..]
            Instruction::Dup(Some(10)),
            // [a_mant, a_exp == EXP_MASK, ..]
            Instruction::Push(0),
            Instruction::U32CheckedNeq,
            // [a_mant != 0, a_exp == EXP_MASK, ..]
            Instruction::U32CheckedAnd,
            // a is nan?
            // [a_mant != 0 & a_exp == EXP_MASK, ..]
            Instruction::U32CheckedOr,
            // [a_is_nan | b_is_nan, ..]
            Instruction::If {
                condition: vec![
                    Instruction::U32CheckedOr,
                    // [a_is_nan | b_is_nan | a_is_zero & b_is_inf | b_is_zero & a_is_inf, a_is_inf, a_is_zero, b_is_inf, b_is_zero, a_exp, b_exp, sign_result, a_mant, b_mant]
                    Instruction::Push(0),
                    Instruction::U32CheckedNeq,
                ],
                // [a_is_inf, a_is_zero, b_is_inf, b_is_zero, a_exp, b_exp, sign_result, a_mant, b_mant]
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
                    Instruction::Push(NAN),
                ],
                else_: vec![Instruction::If {
                    condition: vec![
                        Instruction::MovUp(2),
                        Instruction::U32CheckedOr,
                        // [b_is_inf | a_is_inf, a_is_zero, b_is_zero, a_exp, b_exp, sign_result, a_mant, b_mant]
                        Instruction::Push(0),
                        Instruction::U32CheckedNeq,
                    ],
                    then: vec![
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
                        Instruction::Drop,
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
                            // [a_is_zero | b_is_zero, a_exp, b_exp, sign_result, a_mant, b_mant]
                            Instruction::Push(0),
                            Instruction::U32CheckedNeq,
                        ],
                        then: vec![
                            Instruction::Drop,
                            Instruction::Drop,
                            Instruction::MovDown(2),
                            // [a_mant, b_mant, sign_result]
                            Instruction::Drop,
                            Instruction::Drop,
                        ],
                        else_: vec![
                            Instruction::U32CheckedAdd,
                            Instruction::Push(EXP_BIAS),
                            Instruction::U32WrappingSub,
                            // exp_result
                            // [a_exp + b_exp - EXP_BIAS, sign_result, a_mant, b_mant]
                            Instruction::MovUp(3),
                            Instruction::Push(LEADING_ONE_BIT),
                            Instruction::U32CheckedAdd,
                            Instruction::MovUp(3),
                            Instruction::Push(LEADING_ONE_BIT),
                            Instruction::U32CheckedAdd,
                            // [a_mant | LEADING_ONE_BIT, b_mant | LEADING_ONE_BIT, exp_result, sign_result]
                            Instruction::U32OverflowingMul,
                            Instruction::U32CheckedSHL(Some(9)),
                            Instruction::Swap,
                            Instruction::U32CheckedSHR(Some(23)),
                            Instruction::U32WrappingAdd,
                            // mant_result
                            // [((a_mant | LEADING_ONE_BIT) * (b_mant | LEADING_ONE_BIT)) >> 23, exp_result, sign_result]
                            Instruction::If {
                                condition: vec![
                                    Instruction::Dup(None),
                                    Instruction::Push(0x0100_0000),
                                    Instruction::U32CheckedAnd,
                                    // [mant_result & 0x0100_0000, mant_result, exp_result, sign_result]
                                    Instruction::Push(0),
                                    Instruction::U32CheckedNeq,
                                ],
                                then: vec![
                                    Instruction::U32CheckedSHR(Some(1)),
                                    Instruction::Swap,
                                    Instruction::Push(1),
                                    Instruction::U32CheckedAdd,
                                    // [exp_result + 1, mant_result >> 1, sign_result]
                                ],
                                else_: vec![Instruction::Swap],
                            },
                            // [exp_result, mant_result, sign_result]
                            Instruction::If {
                                condition: vec![
                                    Instruction::Dup(None),
                                    Instruction::Push(SIGN_MASK),
                                    Instruction::U32CheckedAnd,
                                    Instruction::Push(SIGN_MASK),
                                    Instruction::U32CheckedEq,
                                ],
                                then: vec![Instruction::Drop, Instruction::Drop],
                                else_: vec![Instruction::If {
                                    condition: vec![
                                        Instruction::Dup(None),
                                        Instruction::Push(0xff),
                                        Instruction::U32CheckedGTE,
                                    ],
                                    then: vec![
                                        Instruction::Drop,
                                        Instruction::Drop,
                                        Instruction::Push(INFINITY),
                                        Instruction::U32CheckedAdd,
                                    ],
                                    else_: vec![
                                        Instruction::U32CheckedSHL(Some(23)),
                                        Instruction::Swap,
                                        Instruction::Push(MANT_MASK),
                                        Instruction::U32CheckedAnd,
                                        Instruction::U32CheckedAdd,
                                        Instruction::U32CheckedAdd,
                                    ],
                                }],
                            },
                        ],
                    }],
                }],
            },
        ]
        .into_iter(),
    );

    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

#[cfg(test)]
mod tests {
    use std::ops::*;

    use itertools::Itertools;
    use quickcheck_macros::quickcheck;
    use test_case::test_case;

    use super::*;

    const TEST_RUST_NATIVE_F32_ACCURACY: f32 = 1e-6;
    const TEST_EDGE_CASES: &[f32] = &[
        0.0,
        -0.0,
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
            &mut compiler.instructions,
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
            .read(&mut compiler.instructions, result.memory_addr, WIDTH);

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
        Ok(f32::from_bits(stack[0] as u32))
    }

    fn assert_bin_op(
        a: f32,
        b: f32,
        rust_op: fn(f32, f32) -> f32,
        vm_op: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol,
    ) {
        let expected = rust_op(a, b);
        let result = helper_bin_op(a, b, vm_op).unwrap();
        assert!(
            result.is_nan() && expected.is_nan()
                || result == expected
                || (result - expected).abs()
                    < TEST_RUST_NATIVE_F32_ACCURACY * (1.0 + expected.abs()),
            "test_mul: {a} * {b}, result: {result}, expected: {expected}"
        );
    }

    #[test_case(f32::mul, super::mul; "mul")]
    fn test_edge_cases(
        rust_op: fn(f32, f32) -> f32,
        vm_op: fn(&mut Compiler, &Symbol, &Symbol) -> Symbol,
    ) {
        TEST_EDGE_CASES
            .iter()
            .copied()
            .permutations(2)
            .for_each(|x| assert_bin_op(x[0], x[1], rust_op, vm_op));
    }

    #[quickcheck_macros::quickcheck]
    fn test_mul(a: f32, b: f32) {
        assert_bin_op(a, b, f32::mul, mul)
    }
}
