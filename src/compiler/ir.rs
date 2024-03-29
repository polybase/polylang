// TODO: remove
#![allow(unused)]

use std::{borrow::Cow, collections::HashMap};

use super::encoder;
use error::prelude::*;

#[derive(Debug, Clone)]
enum Expression {
    Number(u64),
    FunctionCall {
        name: String,
        // args are indexes to expression values
        args: Vec<ExpressionRef>,
    },
    If {
        condition: ExpressionRef,
        then: Vec<ExpressionRef>,
        then_dependencies: Vec<ExpressionRef>,
        otherwise: Vec<ExpressionRef>,
        otherwise_dependencies: Vec<ExpressionRef>,
    },
}

#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash)]
struct ExpressionRef {
    expr_index: usize,
    nth_element: usize,
}

impl ExpressionRef {
    fn new(expr_index: usize, nth_element: usize) -> Self {
        Self {
            expr_index,
            nth_element,
        }
    }
}

impl ToString for ExpressionRef {
    fn to_string(&self) -> String {
        format!("{}[{}]", self.expr_index, self.nth_element)
    }
}

#[derive(Clone)]
struct Function<'a> {
    name: Cow<'static, str>,
    num_args: usize,
    num_outputs: usize,
    instruction: encoder::Instruction<'a>,
    pure: bool,
}

struct Builder<'a> {
    start_expr_index: usize,
    expressions: Vec<Expression>,
    functions: &'a [Function<'a>],
}

impl<'a> Builder<'a> {
    fn new(functions: &'a [Function]) -> Builder<'a> {
        Self {
            start_expr_index: 0,
            expressions: Vec::new(),
            functions,
        }
    }

    fn add_expression(&mut self, expr: Expression) -> Result<Vec<ExpressionRef>> {
        let expr_index = self.last_expr_index();
        let outputs = match expr {
            Expression::Number(_) => 1,
            Expression::FunctionCall { ref name, args: _ } => {
                let func = self.find_function(name)?;
                func.num_outputs
            }
            Expression::If {
                condition: _,
                then: _,
                then_dependencies: _,
                otherwise: _,
                otherwise_dependencies: _,
            } => return Err(Error::unimplemented("builder if's".into())),
        };
        self.expressions.push(expr);

        let mut refs = Vec::with_capacity(outputs);
        for nth_element in 0..outputs {
            let expr_ref = ExpressionRef::new(expr_index, nth_element);
            refs.push(expr_ref);
        }
        Ok(refs)
    }

    fn find_function(&self, name: &str) -> Result<&Function> {
        self.functions
            .iter()
            .rev()
            .find(|f| f.name == name)
            .not_found("function", name)
    }

    fn number(&mut self, n: u64) -> ExpressionRef {
        let expr = Expression::Number(n);
        let expr_index = self.last_expr_index();
        self.expressions.push(expr);
        ExpressionRef::new(expr_index, 0)
    }

    fn boolean(&mut self, b: bool) -> ExpressionRef {
        let expr = Expression::Number(b as u64);
        let expr_index = self.last_expr_index();
        self.expressions.push(expr);
        ExpressionRef::new(expr_index, 0)
    }

    fn call_func(
        &mut self,
        func_name: &str,
        args: &[ExpressionRef],
    ) -> Result<(usize, Vec<ExpressionRef>)> {
        let func = self.find_function(func_name)?;
        let num_outputs = func.num_outputs;
        ensure!(
            func.num_args == args.len(),
            TypeMismatchSnafu {
                context: format!(
                    "expected {} but found {} arguments in {}",
                    func.num_args,
                    args.len(),
                    func_name
                )
            }
        );

        let expr = Expression::FunctionCall {
            name: func_name.to_string(),
            args: args.to_vec(),
        };
        let expr_index = self.last_expr_index();
        self.expressions.push(expr);

        let mut refs = Vec::with_capacity(num_outputs);
        for nth_element in 0..num_outputs {
            let expr_ref = ExpressionRef::new(expr_index, nth_element);
            refs.push(expr_ref);
        }
        Ok((expr_index, refs))
    }

    fn call(&mut self, func_name: &str, args: &[ExpressionRef]) -> Result<Vec<ExpressionRef>> {
        self.call_func(func_name, args).map(|(_, e)| e)
    }

    fn call_for_expr_index(&mut self, func_name: &str, args: &[ExpressionRef]) -> Result<usize> {
        self.call_func(func_name, args).map(|(i, _)| i)
    }

    fn if_<const OUTPUTS: usize>(
        &mut self,
        condition: ExpressionRef,
        then: impl FnOnce(&mut Self) -> [ExpressionRef; OUTPUTS],
        otherwise: impl FnOnce(&mut Self) -> [ExpressionRef; OUTPUTS],
    ) -> [ExpressionRef; OUTPUTS] {
        let mut then_builder = Self::new(self.functions);
        then_builder.start_expr_index = self.last_expr_index();
        let then_refs = then(&mut then_builder);

        let mut otherwise_builder = Self::new(self.functions);
        otherwise_builder.start_expr_index = then_builder.last_expr_index();
        let otherwise_refs = otherwise(&mut otherwise_builder);

        self.expressions.append(&mut then_builder.expressions);
        self.expressions.append(&mut otherwise_builder.expressions);

        let expr_index = self.last_expr_index();
        self.expressions.push(Expression::If {
            condition,
            then: then_refs.to_vec(),
            then_dependencies: vec![],
            otherwise: otherwise_refs.to_vec(),
            otherwise_dependencies: vec![],
        });

        let mut refs = [ExpressionRef::new(0, 0); OUTPUTS];
        for (i, ref_) in refs.iter_mut().enumerate() {
            *ref_ = ExpressionRef::new(expr_index, i);
        }
        refs
    }

    fn last_expr_index(&self) -> usize {
        self.start_expr_index + self.expressions.len()
    }

    fn build(self) -> Vec<Expression> {
        self.expressions
    }
}

#[derive(Default)]
struct Compiler<'a> {
    expressions: Vec<Expression>,
    /// Stack is a list of expression references.
    /// It's consistent with the stack of the miden code that is being generated.
    stack: Vec<ExpressionRef>,
    /// Expressions that have already been compiled.
    used_exprs: Vec<usize>,
    /// Expression -> dependency.
    expr_to_dependency: Vec<(ExpressionRef, ExpressionRef)>,
    dependencies: Vec<ExpressionRef>,
    /// Expression -> usage count.
    expr_ref_use_count: HashMap<ExpressionRef, usize>,
    /// Functions available to call.
    functions: Vec<Function<'a>>,
    /// Generated miden code.
    miden_code: Vec<encoder::Instruction<'a>>,
}

impl<'a> Compiler<'a> {
    fn find_function(&self, name: &str) -> Result<&Function<'a>> {
        self.functions
            .iter()
            .rev()
            .find(|f| f.name == name)
            .not_found("function", name)
    }

    fn find_expr_on_stack(&self, expr_ref: &ExpressionRef) -> Result<usize> {
        self.stack
            .iter()
            .rev()
            .enumerate()
            .find_map(|(i, e)| if e == expr_ref { Some(i) } else { None })
            .not_found("expr", &format!("expr {expr_ref:?}"))
    }

    fn movup(&mut self, expr_ref: &ExpressionRef) -> Result<()> {
        let position = self.find_expr_on_stack(expr_ref)?;
        match position {
            0 => {
                // The element is already on the top of the stack.
            }
            1 => {
                self.miden_code.push(encoder::Instruction::Swap);
                let stack_len = self.stack.len();
                ensure!(
                    stack_len >= 2,
                    StackSnafu {
                        stack_len,
                        expected: Some(2),
                    }
                );
                self.stack.swap(stack_len - 1, stack_len - 2);
            }
            n if n < 16 => {
                self.miden_code.push(encoder::Instruction::MovUp(n as u32));
                let stack_len = self.stack.len();
                ensure!(
                    stack_len > position,
                    StackSnafu {
                        stack_len,
                        expected: Some(1 + position),
                    }
                );
                let element = self.stack.remove(stack_len - 1 - position);
                self.stack.push(element);
            }
            n => {
                return Err(Error::simple(format!(
                    "Cannot move up further than to the 16th position: {n}"
                )))
            }
        }
        Ok(())
    }

    /// Exprs is highest to lowest.
    fn movup_many(&mut self, exprs: &[ExpressionRef]) -> Result<()> {
        let stack_len = self.stack.len();
        ensure!(
            stack_len >= exprs.len(),
            StackSnafu {
                stack_len,
                expected: Some(exprs.len()),
            }
        );

        // Check if the elements are already in the right order
        // exprs[0] is the top element
        let in_order = exprs
            .iter()
            .enumerate()
            .all(|(i, expr_ref)| self.stack[self.stack.len() - 1 - i] == *expr_ref);

        if in_order {
            return Ok(());
        }

        if let Some(expr) = exprs.last() {
            self.movup(expr)?;
            self.movup_many(&exprs[..exprs.len() - 1])?;
        }

        Ok(())
    }

    /// Moves the top stack element to the nth position.
    fn movdn(&mut self, n: usize) -> Result<()> {
        match n {
            0 => {}
            1 => {
                self.miden_code.push(encoder::Instruction::Swap);
                let stack_len = self.stack.len();
                ensure!(
                    stack_len >= 2,
                    StackSnafu {
                        stack_len,
                        expected: Some(2),
                    }
                );
                self.stack.swap(stack_len - 1, stack_len - 2);
            }
            n if n < 16 => {
                self.miden_code
                    .push(encoder::Instruction::MovDown(n as u32));
                let stack_len = self.stack.len();
                ensure!(
                    stack_len > n,
                    StackSnafu {
                        stack_len,
                        expected: Some(1 + n),
                    }
                );
                let el = self.stack.remove(stack_len - 1);
                self.stack.insert(stack_len - 1 - n, el);
            }
            n => {
                return Err(Error::simple(format!(
                    "Cannot move down further than to the 16th position: {n}"
                )))
            }
        }
        Ok(())
    }

    /// Removes all stack elements after the top `n` elements.
    fn cleanup_stack(&mut self, n: usize) -> Result<()> {
        // We need to first movdn the top `n` elements to the bottom of the stack.
        // Then we can start dropping elements from the top of the stack, until self.stack.len() is n.
        for _ in 0..n.min(self.stack.len()) {
            self.movdn(self.stack.len() - 1)?;
        }
        while self.stack.len() > n {
            self.miden_code.push(encoder::Instruction::Drop);
            // draw an edge from top element to drop
            self.stack.pop().unwrap();
        }
        Ok(())
    }

    fn align_stack(&mut self, expr_refs: &[ExpressionRef]) -> Result<()> {
        self.movup_many(expr_refs)?;
        debug_assert!(
            self.stack.iter().rev().take(expr_refs.len()).eq(expr_refs),
            "invalid stack alignment"
        );
        Ok(())
    }

    fn compile_expr(&mut self, expr_ref: &ExpressionRef) -> Result<()> {
        if self.used_exprs.contains(&expr_ref.expr_index) {
            return Ok(());
        }

        self.used_exprs.push(expr_ref.expr_index);

        match &self.expressions[expr_ref.expr_index] {
            Expression::Number(n) => {
                self.miden_code.push(encoder::Instruction::Push(*n as u32));

                self.stack.push(ExpressionRef {
                    expr_index: expr_ref.expr_index,
                    nth_element: 0,
                });
            }
            Expression::FunctionCall { name, args } => {
                let func = self.find_function(name)?.clone();
                let args = args.clone();

                ensure!(
                    func.num_args == args.len(),
                    TypeMismatchSnafu {
                        context: format!(
                            "tried to call function {} with {} arguments but requied {}",
                            name,
                            args.len(),
                            func.num_args
                        ),
                    }
                );

                let name = name.clone();
                for (i, arg) in args.iter().enumerate().rev() {
                    self.compile_expr(arg)
                        .nest_err(|| format!("{i}-th arg of {name}"))?;
                }
                self.align_stack(&args)
                    .nest_err(|| format!("at align of fn {name}"))?;

                self.miden_code.push(func.instruction);

                for _ in 0..func.num_args {
                    self.stack.pop();
                }

                for i in (0..func.num_outputs).rev() {
                    let expr_ref = ExpressionRef::new(expr_ref.expr_index, i);
                    self.stack.push(expr_ref);
                }
            }
            Expression::If {
                condition,
                then,
                then_dependencies: _,
                otherwise,
                otherwise_dependencies: _,
            } => {
                let condition = *condition;
                let then = then.clone();
                let otherwise = otherwise.clone();

                self.compile_expr(&condition)?;
                self.movup(&condition);
                self.miden_code.push(encoder::Instruction::IfTrue);
                self.stack.pop();

                for expr in &then {
                    self.compile_expr(expr)?;
                }
                self.align_stack(&then)?;

                for _ in &then {
                    self.stack.pop();
                }

                self.miden_code.push(encoder::Instruction::IfElse);

                for expr in &otherwise {
                    self.compile_expr(expr)?;
                }
                self.align_stack(&otherwise);

                for _ in &otherwise {
                    self.stack.pop();
                }

                self.miden_code.push(encoder::Instruction::IfEnd);

                ensure!(
                    then.len() == otherwise.len(),
                    TypeMismatchSnafu {
                        context: format!(
                            "num exprs of then branch ({}) mismatches the else one ({})",
                            then.len(),
                            otherwise.len()
                        )
                    }
                );
                for (i, _) in then.iter().enumerate().rev() {
                    self.stack.push(ExpressionRef {
                        expr_index: expr_ref.expr_index,
                        nth_element: i,
                    });
                }
            }
        }

        Ok(())
    }

    fn compile(mut self, outputs: &[ExpressionRef]) -> Result<Vec<encoder::Instruction<'a>>> {
        for output in outputs {
            self.compile_expr(output)?;
        }
        self.align_stack(outputs)?;
        self.cleanup_stack(outputs.len())?;

        Ok(self.miden_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let functions = [Function {
            name: Cow::Borrowed("u32wrapping_add"),
            num_args: 2,
            num_outputs: 1,
            instruction: encoder::Instruction::U32WrappingAdd,
            pure: true,
        }];

        let mut builder = Builder::new(&functions);

        let a = builder.number(1);
        let b = builder.number(2);
        let [result] = builder.call("u32wrapping_add", &[a, b]).unwrap()[..] else {
            unreachable!()
        };

        let compiler = Compiler {
            expressions: builder.build(),
            functions: functions.to_vec(),
            ..Default::default()
        };

        let code = compiler
            .compile(&[result])
            .unwrap_or_else(|e| panic!("{e}"));

        assert_eq!(
            code,
            vec![
                encoder::Instruction::Push(2),
                encoder::Instruction::Push(1),
                encoder::Instruction::U32WrappingAdd,
            ]
        );
    }

    #[test]
    fn test_dup() {
        let functions = [
            Function {
                name: Cow::Borrowed("u32wrapping_add"),
                num_args: 2,
                num_outputs: 1,
                instruction: encoder::Instruction::U32WrappingAdd,
                pure: true,
            },
            Function {
                name: Cow::Borrowed("dup"),
                num_args: 1,
                num_outputs: 2,
                instruction: encoder::Instruction::Dup(None),
                pure: true,
            },
        ];
        let mut builder = Builder::new(&functions);

        let a = builder.number(1);
        let [a2, a] = builder.call("dup", &[a]).unwrap()[..] else {
            unreachable!()
        };
        let [result] = builder.call("u32wrapping_add", &[a2, a]).unwrap()[..] else {
            unreachable!()
        };

        let compiler = Compiler {
            expressions: builder.build(),
            functions: functions.to_vec(),
            ..Default::default()
        };

        let code = compiler
            .compile(&[result])
            .unwrap_or_else(|e| panic!("{e}"));

        assert_eq!(
            code,
            vec![
                encoder::Instruction::Push(1),
                encoder::Instruction::Dup(None),
                encoder::Instruction::U32WrappingAdd,
            ],
        );
    }

    #[test]
    fn test_dup_with_align() {
        let functions = [
            Function {
                name: Cow::Borrowed("u32wrapping_add"),
                num_args: 2,
                num_outputs: 1,
                instruction: encoder::Instruction::U32WrappingAdd,
                pure: true,
            },
            Function {
                name: Cow::Borrowed("dup"),
                num_args: 1,
                num_outputs: 2,
                instruction: encoder::Instruction::Dup(None),
                pure: true,
            },
        ];
        let mut builder = Builder::new(&functions);

        let a = builder.number(1);
        let [a2, a] = builder.call("dup", &[a]).unwrap()[..] else {
            unreachable!()
        };
        let [result] = builder.call("u32wrapping_add", &[a, a2]).unwrap()[..] else {
            unreachable!()
        };

        let compiler = Compiler {
            expressions: builder.build(),
            functions: functions.to_vec(),
            ..Default::default()
        };

        let code = compiler
            .compile(&[result])
            .unwrap_or_else(|e| panic!("{e}"));

        assert_eq!(
            code,
            vec![
                encoder::Instruction::Push(1),
                encoder::Instruction::Dup(None),
                encoder::Instruction::Swap,
                encoder::Instruction::U32WrappingAdd,
            ]
        );
    }

    #[test]
    fn test_if() {
        let mut builder = Builder::new(&[]);

        let boolean = builder.boolean(true);
        let [a_from_if] = builder.if_(
            boolean,
            |builder| {
                let a = builder.number(1);
                [a]
            },
            |builder| {
                let a = builder.number(2);
                [a]
            },
        );

        let compiler = Compiler {
            expressions: builder.build(),
            ..Default::default()
        };

        let code = compiler
            .compile(&[a_from_if])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            code,
            vec![
                encoder::Instruction::Push(1),
                encoder::Instruction::IfTrue,
                encoder::Instruction::Push(1),
                encoder::Instruction::IfElse,
                encoder::Instruction::Push(2),
                encoder::Instruction::IfEnd,
            ]
        );
    }

    #[test]
    fn test_with_dependencies() {
        let functions = [Function {
            name: Cow::Borrowed("assert_guard"),
            num_args: 2,
            num_outputs: 1,
            instruction: encoder::Instruction::Assert,
            pure: false,
        }];
        let mut builder = Builder::new(&functions);

        let true_ = builder.boolean(true);
        let n123 = builder.number(123);
        let [n123] = builder.call("assert_guard", &[true_, n123]).unwrap()[..] else {
            unreachable!()
        };

        let compiler = Compiler {
            expressions: builder.build(),
            functions: functions.to_vec(),
            ..Default::default()
        };

        let code = compiler.compile(&[n123]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            code,
            vec![
                encoder::Instruction::Push(123),
                encoder::Instruction::Push(1),
                encoder::Instruction::Assert,
            ]
        );
    }
}
