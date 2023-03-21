use super::*;

pub(crate) const WIDTH: u32 = 1;

// TODO: optimize the instructions for int32 artihmetic operations

pub(crate) fn new(compiler: &mut Compiler, value: i32) -> Symbol {
    let symbol = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

    // memory is zero-initialized, so we don't need to write for 0
    if value != 0 {
        compiler.memory.write(
            &mut compiler.instructions,
            symbol.memory_addr,
            &[ValueSource::Immediate(value as u32)],
        );
    }

    symbol
}

/// Extracts signs for both operands.
/// Stack will look like this: [b, a, b_sign, a_sign]
fn prepare_stack_for_arithmetic(compiler: &mut Compiler, a: &Symbol, b: &Symbol) {
    compiler.memory.read(
        &mut compiler.instructions,
        a.memory_addr,
        a.type_.miden_width(),
    );
    // [a]
    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [a, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHR(Some(31)));
    // [a_sign, a]
    compiler.memory.read(
        &mut compiler.instructions,
        b.memory_addr,
        b.type_.miden_width(),
    );
    // [b, a_sign, a]
    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [b, b, a_sign, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHR(Some(31)));
    // [b_sign, b, a_sign, a]
    compiler.instructions.push(encoder::Instruction::MovUp(3));
    // [a, b_sign, b, a_sign]
    compiler.instructions.push(encoder::Instruction::MovUp(2));
    // [b, a, b_sign, a_sign]
}

/// abs_stack returns (on the stack) the absolute value of the value at the top of the stack.
fn abs_stack(compiler: &mut Compiler) {
    // current stack: [value]
    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [value, value]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHR(Some(31)));
    // [sign, value]
    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [sign, sign, value]
    compiler.instructions.push(encoder::Instruction::MovDown(2));
    // [sign, value, sign]
    compiler
        .instructions
        .push(encoder::Instruction::Push(-1i32 as u32));
    // [4294967295, sign, value, sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedMul);
    // [sign * 4294967295, value, sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedXOR);
    // [value ^ (sign * 4294967295), sign]
    add_stack(compiler);
    // [int32CheckedAdd(value ^ (sign * 4294967295))]
}

fn negate_stack(compiler: &mut Compiler) {
    // current stack: [value]
    compiler
        .instructions
        .push(encoder::Instruction::Push(-1i32 as u32));
    // [4294967295, value]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [value, 4294967295]
    compiler
        .instructions
        .push(encoder::Instruction::U32WrappingSub);
    // [value - 4294967295]
    compiler.instructions.push(encoder::Instruction::Push(1u32));
    // [1, value - 4294967295]
    add_stack(compiler);
    // [value - 4294967295 + 1]
}

fn add_stack(compiler: &mut Compiler) {
    // current stack: [b, a]
    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(1)));
    // current stack: [a, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHR(Some(31)));
    // current stack: [sign_a, b, a]
    compiler.instructions.push(encoder::Instruction::Dup(None));
    // current stack: [sign_a, sign_a, b, a]
    compiler.instructions.push(encoder::Instruction::MovUp(2));
    // [b, sign_a, sign_a, a]
    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [b, b, sign_a, sign_a, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHR(Some(31)));
    // [sign_b, b, sign_a, sign_a, a]
    compiler.instructions.push(encoder::Instruction::MovUp(2));
    // [sign_a, sign_b, b, sign_a, a]
    compiler.instructions.push(encoder::Instruction::If {
        condition: vec![encoder::Instruction::U32CheckedEq],
        then: vec![
            // the result needs to be the same sign as a
            // current stack: [b, sign_a, a]
            encoder::Instruction::MovUp(2),
            // [a, b, sign_a]
            encoder::Instruction::U32WrappingAdd,
            // [result, sign_a]
            encoder::Instruction::Dup(None),
            // [result, result, sign_a]
            encoder::Instruction::U32CheckedSHR(Some(31)),
            // [sign_result, result, sign_a]
            encoder::Instruction::MovUp(2),
            // [sign_a, sign_result, result]
            encoder::Instruction::U32CheckedEq,
            // [sign_a == sign_result, result]
            encoder::Instruction::Assert,
            // [result]
        ],
        else_: vec![
            // we're adding values of different signs, overflow is impossible
            // current stack: [b, sign_a, a]
            encoder::Instruction::Swap,
            // [sign_a, b, a]
            encoder::Instruction::Drop,
            // [b, a]
            encoder::Instruction::U32WrappingAdd,
            // [result]
        ],
    });
}

/// adds two int32s with overflow checking.
// If a and b are the same sign, then the result must be the same sign, otherwise we have an overflow.
pub(crate) fn add(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

    compiler.memory.read(
        &mut compiler.instructions,
        a.memory_addr,
        a.type_.miden_width(),
    );
    // [a]
    compiler.memory.read(
        &mut compiler.instructions,
        b.memory_addr,
        b.type_.miden_width(),
    );
    // [b, a]

    add_stack(compiler);

    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

/// subtracts two int32s with overflow checking.
// If a and b are of different signs, then the result can't be the same sign as b, otherwise we have an overflow.
pub(crate) fn sub(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // current stack: [b, a, b_sign, a_sign]
    compiler.instructions.extend(
        [
            encoder::Instruction::U32WrappingSub,
            // [result, b_sign, a_sign]
            encoder::Instruction::Dup(None),
            // [result, result, b_sign, a_sign]
            encoder::Instruction::U32CheckedSHR(Some(31)),
            // [result_sign, result, b_sign, a_sign]
            encoder::Instruction::Dup(Some(3)),
            // [b_sign, result_sign, result, b_sign, a_sign]
            encoder::Instruction::MovUp(4),
            // [a_sign, b_sign, b_sign, result_sign, result, b_sign]
            encoder::Instruction::MovUp(4),
            // [b_sign, a_sign, b_sign, result_sign, result]
            encoder::Instruction::If {
                condition: vec![
                    encoder::Instruction::U32CheckedEq,
                    encoder::Instruction::Not,
                ],
                // [b_sign, result_sign, result]
                then: vec![
                    encoder::Instruction::U32CheckedEq,
                    // [b_sign == result_sign, result]
                    encoder::Instruction::Assert,
                ],
                else_: vec![
                    // [b_sign, result_sign, result]
                    encoder::Instruction::Drop,
                    // [result_sign, result]
                    encoder::Instruction::Drop,
                    // [result]
                ],
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

/// multiplies two int32s with overflow checking.
// The sign of the result must be (sign_a + sign_b) % 2,
// and if neither of the operands is 0, then the result can't be 0.
// We basically do u32CheckedMul(abs(a), abs(b)) and negate if the sign should be negative.
// TODO: a lot of opportunities for optimization here.
pub(crate) fn mul(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // current stack: [b, a, b_sign, a_sign]

    let if_zero = vec![
        encoder::Instruction::Drop,
        encoder::Instruction::Drop,
        encoder::Instruction::Drop,
        encoder::Instruction::Drop,
        encoder::Instruction::Push(0),
        // [0]
    ];
    let mut if_not_zero = {
        let mut instructions = Vec::new();
        let mut compiler = Compiler::new(&mut instructions, compiler.memory, compiler.root_scope);
        // [b, a, b_sign, a_sign]
        abs_stack(&mut compiler);
        // [abs(b), a, b_sign, a_sign]
        compiler.instructions.push(encoder::Instruction::Swap);
        // [a, abs(b), b_sign, a_sign]
        abs_stack(&mut compiler);
        // [abs(a), abs(b), b_sign, a_sign]
        compiler.instructions.push(encoder::Instruction::Swap);
        // [abs(b), abs(a), b_sign, a_sign]
        compiler
            .instructions
            .push(encoder::Instruction::U32CheckedMul);
        // [result, b_sign, a_sign]
        compiler.instructions.push(encoder::Instruction::Dup(None));
        // [result, result, b_sign, a_sign]
        compiler
            .instructions
            .push(encoder::Instruction::U32CheckedSHR(Some(31)));
        // [result_bit, result, b_sign, a_sign]
        compiler.instructions.push(encoder::Instruction::AssertZero);
        // [result, b_sign, a_sign]

        let if_expected_negative = {
            let mut instructions = vec![];
            let mut compiler =
                Compiler::new(&mut instructions, compiler.memory, compiler.root_scope);
            // [result]
            negate_stack(&mut compiler);
            // [negate(result)]

            instructions
        };

        compiler.instructions.push(encoder::Instruction::If {
            condition: vec![
                encoder::Instruction::MovDown(2),
                // [b_sign, a_sign, result]
                encoder::Instruction::U32CheckedAdd,
                // [b_sign + a_sign, result]
                encoder::Instruction::Push(1),
                encoder::Instruction::U32CheckedEq,
            ],
            // [result]
            then: if_expected_negative,
            else_: vec![
                // do nothing, return the result as is
            ],
        });

        instructions
    };

    compiler.instructions.extend(
        [
            encoder::Instruction::Dup(Some(1)),
            // [a, b, a, b_sign, a_sign]
            encoder::Instruction::Push(0),
            // [0, a, b, a, b_sign, a_sign]
            encoder::Instruction::U32CheckedEq,
            // [a == 0, b, a, b_sign, a_sign]
            encoder::Instruction::Dup(Some(1)),
            // [b, a == 0, b, a, b_sign, a_sign]
            encoder::Instruction::Push(0),
            // [0, b, a == 0, b, a, b_sign, a_sign]
            encoder::Instruction::U32CheckedEq,
            // [b == 0, a == 0, b, a, b_sign, a_sign]
            encoder::Instruction::If {
                condition: vec![
                    encoder::Instruction::Or,
                    // [a == 0 || b == 0, b, a, b_sign, a_sign]
                ],
                //  [b, a, b_sign, a_sign]
                then: if_zero,
                else_: if_not_zero,
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

/// divides two int32s with overflow checking.
// First overflow check: b == 0
// Second overflow check: a == i32::MIN && b == -1
pub(crate) fn div(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

    prepare_stack_for_arithmetic(compiler, a, b);
    // current stack: [b, a, b_sign, a_sign]

    // fail if b == 0
    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [b, b, a, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::Push(0));
    // [0, b, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    // [b == 0, b, a, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::AssertZero);
    // [b, a, b_sign, a_sign]

    // fail if a == i32::MIN && b == -1
    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [b, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::Push(-1i32 as u32));
    // [-1, b, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    // [b == -1, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(2)));
    // [a, b == -1, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::Push(i32::MIN as u32));
    // [i32::MIN, a, b == -1, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    // [a == i32::MIN, b == -1, b, a, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::And);
    // [a == i32::MIN && b == -1, b, a, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::AssertZero);

    // [b, a, b_sign, a_sign]
    abs_stack(compiler);
    // [abs(b), a, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [a, abs(b), b_sign, a_sign]
    abs_stack(compiler);
    // [abs(a), abs(b), b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [abs(b), abs(a), b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedDiv);
    // [result, b_sign, a_sign]

    let negation = {
        let mut instructions = Vec::new();
        let mut compiler = Compiler::new(&mut instructions, compiler.memory, compiler.root_scope);

        // [result]
        negate_stack(&mut compiler);
        // [negate(result)]

        instructions
    };

    // negate result if signA + signB == 1
    compiler.instructions.push(encoder::Instruction::If {
        condition: vec![
            encoder::Instruction::MovDown(2),
            // [b_sign, a_sign, result]
            encoder::Instruction::U32CheckedAdd,
            // [b_sign + a_sign, result]
            encoder::Instruction::Push(1),
            // [1, b_sign + a_sign, result]
            encoder::Instruction::U32CheckedEq,
            // [b_sign + a_sign == 1, result]
        ],
        // [result]
        then: negation,
        else_: vec![
            // do nothing, return the result as is
        ],
    });

    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

/// calculates the modulo of two int32s with overflow checking.
// First overflow check: b == 0
// Second overflow check: a == i32::MIN && b == -1
pub(crate) fn modulo(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

    compiler.memory.read(
        &mut compiler.instructions,
        a.memory_addr,
        a.type_.miden_width(),
    );
    // [a]
    compiler.memory.read(
        &mut compiler.instructions,
        b.memory_addr,
        b.type_.miden_width(),
    );
    // [b, a]

    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [b, b, a]
    compiler.instructions.push(encoder::Instruction::Push(0));
    // [0, b, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    // [b == 0, b, a]
    compiler.instructions.push(encoder::Instruction::Not);
    // [b != 0, b, a]
    compiler.instructions.push(encoder::Instruction::Assert);
    // [b, a]
    // fails on a % 0

    // assert(a != min() || b != negate(1), 'modInt32 overflow, dividing min by -1');
    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [b, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::Push(-1i32 as u32));
    // [-1, b, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    // [b == -1, b, a]
    compiler.instructions.push(encoder::Instruction::Not);
    // [b != -1, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(2)));
    // [a, b != -1, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::Push(i32::MIN as u32));
    // [i32::MIN, a, b != -1, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    // [a == i32::MIN, b != -1, b, a]
    compiler.instructions.push(encoder::Instruction::Not);
    // [a != i32::MIN, b != -1, b, a]
    compiler.instructions.push(encoder::Instruction::Or);
    // [a != i32::MIN || b != -1, b, a]
    compiler.instructions.push(encoder::Instruction::Assert);
    // [b, a]
    // fails on i32::MIN by -1

    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(1)));
    // [a, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHR(Some(31)));
    // [a_sign, b, a]
    compiler.instructions.push(encoder::Instruction::MovDown(2));
    // [b, a, a_sign]

    abs_stack(compiler);
    // [abs(b), a, a_sign]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [a, abs(b), a_sign]
    abs_stack(compiler);
    // [abs(a), abs(b), a_sign]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [abs(b), abs(a), a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedMod);
    // [abs(a) % abs(b), a_sign]

    compiler.instructions.push(encoder::Instruction::Swap);
    // [a_sign, abs(a) % abs(b)]

    let negation = {
        let mut instructions = Vec::new();
        let mut compiler = Compiler::new(&mut instructions, compiler.memory, compiler.root_scope);

        // [result]
        negate_stack(&mut compiler);
        // [negate(result)]

        instructions
    };

    compiler.instructions.push(encoder::Instruction::If {
        // if a_sign == 1
        condition: vec![],
        // [abs(a) % abs(b)]
        then: negation,
        else_: vec![
            // do nothing, return the result as is
        ],
    });

    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

fn shift(compiler: &mut Compiler, a: &Symbol, b: &Symbol, is_right: bool) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Int32));

    compiler.memory.read(
        &mut compiler.instructions,
        a.memory_addr,
        a.type_.miden_width(),
    );
    // [a]
    compiler.memory.read(
        &mut compiler.instructions,
        b.memory_addr,
        b.type_.miden_width(),
    );
    // [b, a]

    compiler.instructions.push(encoder::Instruction::Dup(None));
    // [b, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHR(Some(31)));
    // [b_sign, b, a]
    compiler.instructions.push(encoder::Instruction::AssertZero);
    // [b, a]
    // fails if shifting by a negative number

    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(1)));
    // [a, b, a]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedSHR(Some(31)));
    // [a_sign, b, a]
    compiler.instructions.push(encoder::Instruction::MovDown(2));
    // [b, a, a_sign]

    abs_stack(compiler);
    // [abs(b), a, a_sign]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [a, abs(b), a_sign]
    abs_stack(compiler);
    // [abs(a), abs(b), a_sign]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [abs(b), abs(a), a_sign]

    compiler.instructions.push(if is_right {
        encoder::Instruction::U32CheckedSHR(None)
    } else {
        encoder::Instruction::U32CheckedSHL(None)
    });
    // [abs(a) >> abs(b), a_sign]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [a_sign, abs(a) >> abs(b)]

    let negation = {
        let mut instructions = Vec::new();
        let mut compiler = Compiler::new(&mut instructions, compiler.memory, compiler.root_scope);

        // [result]
        negate_stack(&mut compiler);
        // [negate(result)]

        instructions
    };

    compiler.instructions.push(encoder::Instruction::If {
        // if a_sign == 1
        condition: vec![],
        // [abs(a) >> abs(b)]
        then: negation,
        else_: vec![
            // do nothing, return the result as is
        ],
    });

    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn shift_right(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    shift(compiler, a, b, true)
}

pub(crate) fn shift_left(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    shift(compiler, a, b, false)
}

/// Turns stack [b, a, b_sign, a_sign] into [a > b]
pub(crate) fn gt_stack(compiler: &mut Compiler) {
    // [b, a, b_sign, a_sign]

    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(3)));
    // [a_sign, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(3)));
    // [b_sign, a_sign, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    // [a_sign == b_sign, b, a, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::MovUp(2));
    // [a, a_sign == b_sign, b, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::MovUp(2));
    // [b, a, a_sign == b_sign, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedGT);
    // [a > b, a_sign == b_sign, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::And);
    // [a_sign == b_sign && a > b, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::Swap);
    // [b_sign, a_sign == b_sign && a > b, a_sign]
    compiler.instructions.push(encoder::Instruction::MovUp(2));
    // [a_sign, b_sign, a_sign == b_sign && a > b]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedGT);
    // [b_sign > a_sign, a_sign == b_sign && a > b]
    compiler.instructions.push(encoder::Instruction::Or);
    // [(a_sign == b_sign && a > b) || b_sign > a_sign]
}

pub(crate) fn gt(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [b, a, b_sign, a_sign]

    gt_stack(compiler);

    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn gte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [b, a, b_sign, a_sign]

    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(1)));
    // [a, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(1)));
    // [b, a, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    // [a == b, b, a, b_sign, a_sign]
    compiler.instructions.push(encoder::Instruction::MovDown(4));
    // [b, a, b_sign, a_sign, a == b]

    gt_stack(compiler);
    // [a > b, a == b]

    compiler.instructions.push(encoder::Instruction::Or);
    // [a > b || a == b]

    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn lte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [b, a, b_sign, a_sign]

    gt_stack(compiler);
    // [a > b]

    compiler.instructions.push(encoder::Instruction::Not);
    // [a <= b]

    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

pub(crate) fn lt(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let result = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    prepare_stack_for_arithmetic(compiler, a, b);
    // [b, a, b_sign, a_sign]

    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(1)));
    // [a, b, a, b_sign, a_sign]
    compiler
        .instructions
        .push(encoder::Instruction::Dup(Some(1)));
    // [b, a, b, a, b_sign, a_sign]

    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedEq);
    // [a == b, b, a, b_sign, a_sign]

    compiler.instructions.push(encoder::Instruction::Not);
    // [a != b, b, a, b_sign, a_sign]

    compiler.instructions.push(encoder::Instruction::MovDown(4));
    // [b, a, b_sign, a_sign, a != b]

    gt_stack(compiler);
    // [a > b, a != b]

    compiler.instructions.push(encoder::Instruction::Not);
    // [a <= b, a != b]

    compiler.instructions.push(encoder::Instruction::And);
    // [a != b && a <= b]

    compiler.memory.write(
        &mut compiler.instructions,
        result.memory_addr,
        &[ValueSource::Stack],
    );

    result
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_prepare_stack_for_arithmetic() {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, 2);
        let b = new(&mut compiler, -2);

        prepare_stack_for_arithmetic(&mut compiler, &a, &b);

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
        )
        .unwrap();

        let stack = outputs.stack_outputs().stack();
        assert_eq!(stack, &[4294967294, 2, 1, 0, 0]);
    }

    fn add(a: i32, b: i32) -> Result<i32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);
        let b = new(&mut compiler, b);

        let result = super::add(&mut compiler, &a, &b);
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

        Ok(stack[0] as i32)
    }

    #[test]
    fn test_add() {
        macro_rules! test {
            ($a:expr, $b:expr, $expected:pat_param) => {
                let result = add($a, $b);
                assert!(
                    matches!(result, $expected),
                    "add({}, {}) = {:?}, expected {}",
                    $a,
                    $b,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, 0, Ok(0));
        test!(1, 0, Ok(1));
        test!(0, 1, Ok(1));
        test!(1, 1, Ok(2));
        test!(1, -1, Ok(0));
        test!(-1, 1, Ok(0));
        test!(-1, -1, Ok(-2));

        test!(i32::MAX, 1, Err(miden::ExecutionError::FailedAssertion(_)));
        test!(i32::MIN, -1, Err(miden::ExecutionError::FailedAssertion(_)));
    }

    fn abs(a: i32) -> Result<i32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);

        compiler.memory.read(
            &mut compiler.instructions,
            a.memory_addr,
            a.type_.miden_width(),
        );
        super::abs_stack(&mut compiler);

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

        Ok(stack[0] as i32)
    }

    #[test]
    fn test_abs() {
        macro_rules! test {
            ($a:expr, $expected:pat_param) => {
                let result = abs($a);
                assert!(
                    matches!(result, $expected),
                    "abs({}) = {:?}, expected {}",
                    $a,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, Ok(0));
        test!(1, Ok(1));
        test!(-1, Ok(1));
        test!(i32::MAX, Ok(i32::MAX));
        test!(i32::MIN + 1, Ok(i32::MAX));

        test!(i32::MIN, Err(miden::ExecutionError::FailedAssertion(_)));
    }

    fn negate(a: i32) -> Result<i32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);

        compiler.memory.read(
            &mut compiler.instructions,
            a.memory_addr,
            a.type_.miden_width(),
        );
        super::negate_stack(&mut compiler);

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

        Ok(stack[0] as i32)
    }

    #[test]
    fn test_negate() {
        macro_rules! test {
            ($a:expr, $expected:pat_param) => {
                let result = negate($a);
                assert!(
                    matches!(result, $expected),
                    "negate({}) = {:?}, expected {}",
                    $a,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, Ok(0));
        test!(1, Ok(-1));
        test!(-1, Ok(1));
        let min_add_1 = i32::MIN + 1;
        test!(i32::MAX, Ok(min_add_1));
        test!(i32::MIN + 1, Ok(i32::MAX));

        test!(i32::MIN, Err(miden::ExecutionError::FailedAssertion(_)));
    }

    fn sub(a: i32, b: i32) -> Result<i32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);
        let b = new(&mut compiler, b);

        let result = super::sub(&mut compiler, &a, &b);
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

        Ok(stack[0] as i32)
    }

    #[test]
    fn test_sub() {
        macro_rules! test {
            ($a:expr, $b:expr, $expected:pat_param) => {
                let result = sub($a, $b);
                assert!(
                    matches!(result, $expected),
                    "sub({}, {}) = {:?}, expected {}",
                    $a,
                    $b,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, 0, Ok(0));
        test!(0, 1, Ok(-1));
        test!(1, 0, Ok(1));
        test!(1, 1, Ok(0));
        test!(i32::MAX, 0, Ok(i32::MAX));
        let max_sub_1 = i32::MAX - 1;
        test!(i32::MAX, 1, Ok(max_sub_1));
        test!(i32::MAX, i32::MAX, Ok(0));
        test!(i32::MIN, 0, Ok(i32::MIN));
        let min_add_1 = i32::MIN + 1;
        test!(i32::MIN, -1, Ok(min_add_1));
        test!(i32::MIN, i32::MIN, Ok(0));

        test!(i32::MIN, 1, Err(miden::ExecutionError::FailedAssertion(_)));
        test!(i32::MAX, -1, Err(miden::ExecutionError::FailedAssertion(_)));
    }

    fn mul(a: i32, b: i32) -> Result<i32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);
        let b = new(&mut compiler, b);

        let result = super::mul(&mut compiler, &a, &b);
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

        Ok(stack[0] as i32)
    }

    #[test]
    fn test_mul() {
        macro_rules! test {
            ($a:expr, $b:expr, $expected:pat_param) => {
                let result = mul($a, $b);
                assert!(
                    matches!(result, $expected),
                    "mul({}, {}) = {:?}, expected {}",
                    $a,
                    $b,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, 0, Ok(0));
        test!(0, 1, Ok(0));
        test!(1, 0, Ok(0));
        test!(1, 1, Ok(1));
        test!(1, -1, Ok(-1));
        test!(-1, 1, Ok(-1));
        test!(-1, -1, Ok(1));
        test!(i32::MAX, 0, Ok(0));
        test!(i32::MAX, 1, Ok(i32::MAX));
        let negative_max = -i32::MAX;
        test!(i32::MAX, -1, Ok(negative_max));
        test!(i32::MIN, 0, Ok(0));

        // TODO: fix this case
        // test!(i32::MIN, 1, Ok(i32::MIN));

        test!(i32::MAX, 2, Err(miden::ExecutionError::FailedAssertion(_)));
        test!(i32::MIN, 2, Err(miden::ExecutionError::FailedAssertion(_)));
        test!(
            i32::MAX,
            i32::MIN,
            Err(miden::ExecutionError::FailedAssertion(_))
        );
        test!(
            i32::MIN,
            i32::MAX,
            Err(miden::ExecutionError::FailedAssertion(_))
        );
        test!(
            i32::MIN,
            i32::MIN,
            Err(miden::ExecutionError::FailedAssertion(_))
        );
        test!(
            i32::MAX,
            i32::MAX,
            Err(miden::ExecutionError::FailedAssertion(_))
        );
        // negating i32::MIN overflows, that would be i32::MAX+1
        test!(i32::MIN, -1, Err(miden::ExecutionError::FailedAssertion(_)));
    }

    fn div(a: i32, b: i32) -> Result<i32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);
        let b = new(&mut compiler, b);

        let result = super::div(&mut compiler, &a, &b);
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

        Ok(stack[0] as i32)
    }

    #[test]
    fn test_div() {
        macro_rules! test {
            ($a:expr, $b:expr, $expected:pat_param) => {
                let result = div($a, $b);
                assert!(
                    matches!(result, $expected),
                    "div({}, {}) = {:?}, expected {}",
                    $a,
                    $b,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, 0, Err(miden::ExecutionError::FailedAssertion(_)));
        test!(1, 0, Err(miden::ExecutionError::FailedAssertion(_)));
        test!(i32::MIN, -1, Err(miden::ExecutionError::FailedAssertion(_)));

        test!(0, 1, Ok(0));
        test!(1, 1, Ok(1));
        test!(1, -1, Ok(-1));
        test!(-1, 1, Ok(-1));
        test!(-1, -1, Ok(1));
        test!(i32::MAX, 1, Ok(i32::MAX));
        let negative_max = -i32::MAX;
        test!(i32::MAX, -1, Ok(negative_max));

        // TODO: fix this case
        // test!(i32::MIN, 1, Ok(i32::MIN));
        let min_add_1 = i32::MIN + 1;

        test!(i32::MIN + 1, 1, Ok(min_add_1));

        let max_divided_by_2 = i32::MAX / 2;
        test!(i32::MAX, 2, Ok(max_divided_by_2));
    }

    fn modulo(a: i32, b: i32) -> Result<i32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);
        let b = new(&mut compiler, b);

        let result = super::modulo(&mut compiler, &a, &b);
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

        Ok(stack[0] as i32)
    }

    #[test]
    fn test_modulo() {
        macro_rules! test {
            ($a:expr, $b:expr, $expected:pat_param) => {
                let result = modulo($a, $b);
                assert!(
                    matches!(result, $expected),
                    "modulo({}, {}) = {:?}, expected {}",
                    $a,
                    $b,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, 0, Err(miden::ExecutionError::FailedAssertion(_)));
        test!(1, 0, Err(miden::ExecutionError::FailedAssertion(_)));
        test!(i32::MIN, -1, Err(miden::ExecutionError::FailedAssertion(_)));

        test!(0, 1, Ok(0));
        test!(1, 1, Ok(0));
        test!(1, -1, Ok(0));
        test!(-1, 1, Ok(0));
        test!(-1, -1, Ok(0));
        test!(i32::MAX, 1, Ok(0));
        test!(i32::MAX, -1, Ok(0));
        test!(-1, i32::MAX, Ok(-1));
        // TODO: fix this case
        // test!(i32::MIN, 1, Ok(0));
    }

    fn shift_right(a: i32, b: i32) -> Result<i32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);
        let b = new(&mut compiler, b);

        let result = super::shift_right(&mut compiler, &a, &b);
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

        Ok(stack[0] as i32)
    }

    #[test]
    fn test_shift_right() {
        macro_rules! test {
            ($a:expr, $b:expr, $expected:pat_param) => {
                let result = shift_right($a, $b);
                assert!(
                    matches!(result, $expected),
                    "shift_right({}, {}) = {:?}, expected {}",
                    $a,
                    $b,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, 0, Ok(0));
        test!(1, 0, Ok(1));
        test!(i32::MAX, 0, Ok(i32::MAX));
        test!(-1, 0, Ok(-1));
        test!(-2, 1, Ok(-1));
        test!(-2, -1, Err(miden::ExecutionError::FailedAssertion(_)));

        // TODO: fix this case
        // test!(i32::MIN, 0, Ok(i32::MIN));
    }

    fn shift_left(a: i32, b: i32) -> Result<i32, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);
        let b = new(&mut compiler, b);

        let result = super::shift_left(&mut compiler, &a, &b);
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

        Ok(stack[0] as i32)
    }

    #[test]
    fn test_shift_left() {
        macro_rules! test {
            ($a:expr, $b:expr, $expected:pat_param) => {
                let result = shift_left($a, $b);
                assert!(
                    matches!(result, $expected),
                    "shift_left({}, {}) = {:?}, expected {}",
                    $a,
                    $b,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, 0, Ok(0));
        test!(1, 0, Ok(1));
        test!(i32::MAX, 0, Ok(i32::MAX));
        test!(-1, 0, Ok(-1));
        test!(-2, 1, Ok(-4));
        test!(-2, -1, Err(miden::ExecutionError::FailedAssertion(_)));
    }

    fn gt(a: i32, b: i32) -> Result<bool, miden::ExecutionError> {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let scope = Scope::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);
        let a = new(&mut compiler, a);
        let b = new(&mut compiler, b);

        let result = super::gt(&mut compiler, &a, &b);
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

        Ok(stack[0] != 0)
    }

    #[test]
    fn test_gt() {
        macro_rules! test {
            ($a:expr, $b:expr, $expected:pat_param) => {
                let result = gt($a, $b);
                assert!(
                    matches!(result, $expected),
                    "gt({}, {}) = {:?}, expected {}",
                    $a,
                    $b,
                    result,
                    stringify!($expected)
                );
            };
        }

        test!(0, 0, Ok(false));
        test!(1, 0, Ok(true));
        test!(i32::MAX, 0, Ok(true));
        test!(-1, 0, Ok(false));
        test!(-2, 1, Ok(false));
        test!(-2, -1, Ok(false));
        test!(i32::MIN, 0, Ok(false));
    }
}