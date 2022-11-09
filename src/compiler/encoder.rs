#[derive(Debug, PartialEq)]
pub(crate) enum Instruction<'a> {
    Comment(String),            // # ...
    Drop,                       // drop
    Push(u32),                  // push.1234
    Assert,                     // assert
    Dup,                        // dup
    Add,                        // add
    U32CheckedAdd,              // u32checked_add
    U32CheckedSub,              // u32checked_sub
    U32CheckedMod,              // u32checked_mod
    U32CheckedDiv,              // u32checked_div
    U32CheckedEq,               // u32checked_eq
    U32CheckedLTE,              // u32checked_lte
    U32CheckedLT,               // u32checked_lt
    U32CheckedGTE,              // u32checked_gte
    U32CheckedGT,               // u32checked_gt
    U32CheckedSHL(Option<u32>), // u32checked_shl
    U32CheckedSHR(Option<u32>), // u32checked_shr
    Exec(&'a str),              // exec.u64::checked_add
    MemStore(Option<u32>),      // mem_store.1234
    MemLoad(Option<u32>),       // mem_load.1234
    AdvPush(u32),               // adv_push.1234
    While {
        condition: Vec<Instruction<'a>>,
        body: Vec<Instruction<'a>>,
    },
    If {
        condition: Vec<Instruction<'a>>,
        then: Vec<Instruction<'a>>,
        else_: Vec<Instruction<'a>>,
    },
    Abstract(AbstractInstruction<'a>),
}

#[derive(Debug, PartialEq)]
pub(crate) enum AbstractInstruction<'a> {
    Break,
    Return,
    InlinedFunction(Vec<Instruction<'a>>),
}

impl Instruction<'_> {
    pub(crate) fn encode(&self, f: &mut impl std::io::Write, depth: usize) -> std::io::Result<()> {
        // write_indent wraps write! but first writes depth*2 spaces
        macro_rules! write_indent {
            ($($arg:tt)*) => {{
                for _ in 0..depth {
                    f.write(b"  ")?;
                }

                write!($($arg)*)?
            }}
        }

        match self {
            Instruction::Comment(s) => write_indent!(f, "# {}", s),
            Instruction::Drop => write_indent!(f, "drop"),
            Instruction::Push(value) => write_indent!(f, "push.{}", value),
            Instruction::Assert => write_indent!(f, "assert"),
            Instruction::Dup => write_indent!(f, "dup"),
            Instruction::Add => write_indent!(f, "add"),
            Instruction::U32CheckedAdd => write_indent!(f, "u32checked_add"),
            Instruction::U32CheckedSub => write_indent!(f, "u32checked_sub"),
            Instruction::U32CheckedMod => write_indent!(f, "u32checked_mod"),
            Instruction::U32CheckedDiv => write_indent!(f, "u32checked_div"),
            Instruction::U32CheckedEq => write_indent!(f, "u32checked_eq"),
            Instruction::U32CheckedLTE => write_indent!(f, "u32checked_lte"),
            Instruction::U32CheckedLT => write_indent!(f, "u32checked_lt"),
            Instruction::U32CheckedGTE => write_indent!(f, "u32checked_gte"),
            Instruction::U32CheckedGT => write_indent!(f, "u32checked_gt"),
            Instruction::U32CheckedSHL(Some(value)) => write_indent!(f, "u32checked_shl.{}", value),
            Instruction::U32CheckedSHL(None) => write_indent!(f, "u32checked_shl"),
            Instruction::U32CheckedSHR(Some(value)) => write_indent!(f, "u32checked_shr.{}", value),
            Instruction::U32CheckedSHR(None) => write_indent!(f, "u32checked_shr"),
            Instruction::Exec(name) => write_indent!(f, "exec.{}", name),
            Instruction::While { condition, body } => {
                for instruction in condition {
                    instruction.encode(f, depth)?;
                    f.write(b"\n")?;
                }
                write_indent!(f, "while.true");
                f.write(b"\n")?;
                for instruction in body {
                    instruction.encode(f, depth + 1)?;
                    f.write(b"\n")?;
                }
                for instruction in condition {
                    instruction.encode(f, depth + 1)?;
                    f.write(b"\n")?;
                }
                write_indent!(f, "end");
            }
            Instruction::MemStore(Some(addr)) => write_indent!(f, "mem_store.{}", addr),
            Instruction::MemStore(None) => write_indent!(f, "mem_store"),
            Instruction::MemLoad(Some(addr)) => write_indent!(f, "mem_load.{}", addr),
            Instruction::MemLoad(None) => write_indent!(f, "mem_load"),
            Instruction::AdvPush(addr) => write_indent!(f, "adv_push.{}", addr),
            Instruction::If {
                condition,
                then,
                else_,
            } => {
                for instruction in condition {
                    instruction.encode(f, depth)?;
                    f.write(b"\n")?;
                }

                write_indent!(f, "if.true\n");

                for instruction in then {
                    instruction.encode(f, depth + 1)?;
                    f.write(b"\n")?;
                }
                if then.len() == 0 {
                    write_indent!(f, "  push.0\n");
                    write_indent!(f, "  drop\n");
                }

                if else_.len() > 0 {
                    write_indent!(f, "else\n");
                    for instruction in else_ {
                        instruction.encode(f, depth + 1)?;
                        f.write(b"\n")?;
                    }
                }

                write_indent!(f, "end");
            }
            Instruction::Abstract(_) => {
                unreachable!("abstract instructions should be unabstracted before encoding")
            }
        };

        std::io::Result::Ok(())
    }
}

pub(crate) fn unabstract<'a>(
    instructions: Vec<Instruction<'a>>,
    allocate: &mut impl FnMut(u32) -> u32,
    break_ptr: &mut Option<u32>,
    return_ptr: &mut Option<u32>,
    is_condition: bool,
) -> Vec<Instruction<'a>> {
    let mut result = Vec::new();
    let mut ptr_value_might_have_been_flipped = false;
    for instruction in instructions {
        let mut unabstract_inst =
            |result: &mut Vec<Instruction<'a>>,
             instruction: Instruction<'a>,
             break_ptr: &mut Option<u32>,
             return_ptr: &mut Option<u32>,
             ptr_value_might_have_been_flipped: &mut bool| {
                match instruction {
                    Instruction::Abstract(instruction) => match instruction {
                        AbstractInstruction::Break => {
                            if let Some(break_ptr) = break_ptr {
                                *ptr_value_might_have_been_flipped = true;
                                result.push(Instruction::Push(1));
                                result.push(Instruction::MemStore(Some(*break_ptr)));
                                result.push(Instruction::Drop);
                            } else {
                                result.push(Instruction::Push(1));
                                let ptr = allocate(1);
                                result.push(Instruction::MemStore(Some(ptr)));
                                result.push(Instruction::Drop);
                                break_ptr.replace(ptr);
                            }
                        }
                        AbstractInstruction::Return => {
                            if let Some(ptr) = return_ptr {
                                *ptr_value_might_have_been_flipped = true;
                                result.push(Instruction::Push(1));
                                result.push(Instruction::MemStore(Some(*ptr)));
                                result.push(Instruction::Drop);
                            } else {
                                result.push(Instruction::Push(1));
                                let ptr = allocate(1);
                                result.push(Instruction::MemStore(Some(ptr)));
                                result.push(Instruction::Drop);
                                return_ptr.replace(ptr);
                            }
                        }
                        AbstractInstruction::InlinedFunction(func) => {
                            result.extend(unabstract(func, allocate, &mut None, &mut None, false));
                        }
                    },
                    Instruction::While { condition, body } => {
                        let mut break_ptr = None;
                        let body = unabstract(body, allocate, &mut break_ptr, return_ptr, false);
                        let condition =
                            unabstract(condition, allocate, &mut break_ptr, return_ptr, true);
                        result.push(Instruction::While {
                            condition: condition,
                            body: body,
                        });
                    }
                    Instruction::If {
                        condition,
                        then,
                        else_,
                    } => {
                        result.push(Instruction::If {
                            condition: unabstract(condition, allocate, &mut None, &mut None, true),
                            then: unabstract(then, allocate, break_ptr, return_ptr, false),
                            else_: unabstract(else_, allocate, break_ptr, return_ptr, false),
                        });
                    }
                    other => result.push(other),
                }
            };

        if let Some(break_return_ptr_inner) = break_ptr.or(*return_ptr) {
            let break_ptr = &mut None;

            let cond = || Instruction::MemLoad(Some(break_return_ptr_inner));
            match result.last_mut() {
                Some(Instruction::If {
                    condition,
                    then: _,
                    else_,
                }) if &condition[..] == &[cond()] && !ptr_value_might_have_been_flipped => {
                    // if the previous instruction is an if with the same condition,
                    // then add to that if
                    unabstract_inst(
                        else_,
                        instruction,
                        break_ptr,
                        return_ptr,
                        &mut ptr_value_might_have_been_flipped,
                    );
                }
                _ => {
                    ptr_value_might_have_been_flipped = false;

                    result.push(Instruction::If {
                        condition: vec![cond()],
                        then: if is_condition {
                            vec![Instruction::Push(0)]
                        } else {
                            vec![]
                        },
                        else_: {
                            let mut else_ = Vec::new();
                            unabstract_inst(
                                &mut else_,
                                instruction,
                                break_ptr,
                                return_ptr,
                                &mut ptr_value_might_have_been_flipped,
                            );
                            else_
                        },
                    })
                }
            }
        } else {
            unabstract_inst(
                &mut result,
                instruction,
                break_ptr,
                return_ptr,
                &mut ptr_value_might_have_been_flipped,
            );
        }
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_unabstract_break() {
        let instructions = vec![Instruction::While {
            condition: vec![Instruction::Push(1)],
            body: vec![
                Instruction::If {
                    condition: vec![Instruction::Push(1)],
                    then: vec![
                        Instruction::Abstract(AbstractInstruction::Break),
                        Instruction::Push(3),
                    ],
                    else_: vec![],
                },
                Instruction::If {
                    condition: vec![Instruction::Push(1)],
                    then: vec![Instruction::Push(1)],
                    else_: vec![],
                },
                Instruction::Push(2),
            ],
        }];

        let expected = vec![Instruction::While {
            condition: vec![Instruction::If {
                condition: vec![Instruction::MemLoad(Some(1))],
                then: vec![Instruction::Push(0)],
                else_: vec![Instruction::Push(1)],
            }],
            body: vec![
                Instruction::If {
                    condition: vec![Instruction::Push(1)],
                    then: vec![
                        Instruction::Push(1),
                        Instruction::MemStore(Some(1)),
                        Instruction::Drop,
                        Instruction::If {
                            condition: vec![Instruction::MemLoad(Some(1))],
                            then: vec![],
                            else_: vec![Instruction::Push(3)],
                        },
                    ],
                    else_: vec![],
                },
                Instruction::If {
                    condition: vec![Instruction::MemLoad(Some(1))],
                    then: vec![],
                    else_: vec![
                        Instruction::If {
                            condition: vec![Instruction::Push(1)],
                            then: vec![Instruction::Push(1)],
                            else_: vec![],
                        },
                        Instruction::Push(2),
                    ],
                },
            ],
        }];

        let unabstracted = unabstract(instructions, &mut |_| 1, &mut None, &mut None, false);
        assert_eq!(unabstracted, expected);
    }

    #[test]
    fn test_unabstract_return() {
        let instructions = vec![
            Instruction::Push(1),
            Instruction::If {
                condition: vec![Instruction::Push(1)],
                then: vec![Instruction::Abstract(AbstractInstruction::Return)],
                else_: vec![],
            },
            Instruction::Push(2),
            Instruction::Push(3),
        ];

        let expected = vec![
            Instruction::Push(1),
            Instruction::If {
                condition: vec![Instruction::Push(1)],
                then: vec![
                    Instruction::Push(1),
                    Instruction::MemStore(Some(1)),
                    Instruction::Drop,
                ],
                else_: vec![],
            },
            Instruction::If {
                condition: vec![Instruction::MemLoad(Some(1))],
                then: vec![],
                else_: vec![Instruction::Push(2), Instruction::Push(3)],
            },
        ];

        let unabstracted = unabstract(instructions, &mut |_| 1, &mut None, &mut None, false);
        assert_eq!(unabstracted, expected);
    }

    #[test]
    fn test_unabstract_return_2() {
        let instructions = vec![
            Instruction::Push(199),
            Instruction::Abstract(AbstractInstruction::Return),
            Instruction::Push(200),
            Instruction::Abstract(AbstractInstruction::Return),
            Instruction::Push(201),
        ];

        let expected = vec![
            Instruction::Push(199),
            Instruction::Push(1),
            Instruction::MemStore(Some(1)),
            Instruction::Drop,
            Instruction::If {
                condition: vec![Instruction::MemLoad(Some(1))],
                then: vec![],
                else_: vec![
                    Instruction::Push(200),
                    Instruction::Push(1),
                    Instruction::MemStore(Some(1)),
                    Instruction::Drop,
                ],
            },
            Instruction::If {
                condition: vec![Instruction::MemLoad(Some(1))],
                then: vec![],
                else_: vec![Instruction::Push(201)],
            },
        ];

        let mut ptr = 1;
        let unabstracted = unabstract(
            instructions,
            &mut |_| {
                ptr += 1;
                ptr - 1
            },
            &mut None,
            &mut None,
            false,
        );
        assert_eq!(unabstracted, expected);
    }
}
