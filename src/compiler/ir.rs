use std::{borrow::Cow, collections::HashMap};

use super::encoder;

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

    fn add_expression(&mut self, expr: Expression) -> Vec<ExpressionRef> {
        let expr_index = self.last_expr_index();
        let outputs = match expr {
            Expression::Number(_) => 1,
            Expression::FunctionCall { ref name, args: _ } => {
                let func = self
                    .functions
                    .iter()
                    .find(|f| f.name == name.as_str())
                    .expect("function not found");
                func.num_outputs
            }
            Expression::If {
                condition,
                then,
                then_dependencies,
                otherwise,
                otherwise_dependencies,
            } => todo!(),
        };
        self.expressions.push(expr);

        let mut refs = Vec::new();
        for nth_element in 0..outputs {
            let expr_ref = ExpressionRef::new(expr_index, nth_element);
            refs.push(expr_ref);
        }
        refs
    }

    fn find_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().rev().find(|f| f.name == name)
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
    ) -> (usize, Vec<ExpressionRef>) {
        let func = self
            .functions
            .iter()
            .find(|f| f.name == func_name)
            .expect("function not found");
        assert_eq!(func.num_args, args.len());

        let expr = Expression::FunctionCall {
            name: func_name.to_string(),
            args: args.to_vec(),
        };
        let expr_index = self.last_expr_index();
        self.expressions.push(expr);

        let mut refs = Vec::new();
        for nth_element in 0..func.num_outputs {
            let expr_ref = ExpressionRef::new(expr_index, nth_element);
            refs.push(expr_ref);
        }
        (expr_index, refs)
    }

    fn call(&mut self, func_name: &str, args: &[ExpressionRef]) -> Vec<ExpressionRef> {
        self.call_func(func_name, args).1
    }

    fn call_for_expr_index(&mut self, func_name: &str, args: &[ExpressionRef]) -> usize {
        self.call_func(func_name, args).0
    }

    fn if_<const Outputs: usize>(
        &mut self,
        condition: ExpressionRef,
        then: impl FnOnce(&mut Self) -> [ExpressionRef; Outputs],
        otherwise: impl FnOnce(&mut Self) -> [ExpressionRef; Outputs],
    ) -> [ExpressionRef; Outputs] {
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

        let mut refs = [ExpressionRef::new(0, 0); Outputs];
        for nth_element in 0..Outputs {
            let expr_ref = ExpressionRef::new(expr_index, nth_element);
            refs[nth_element] = expr_ref;
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
    fn find_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().rev().find(|f| f.name == name)
    }

    fn find_expr_on_stack(&self, expr_ref: &ExpressionRef) -> Option<usize> {
        self.stack
            .iter()
            .rev()
            .enumerate()
            .find_map(|(i, e)| if e == expr_ref { Some(i) } else { None })
    }

    fn movup(&mut self, expr_ref: &ExpressionRef) {
        let position = self
            .find_expr_on_stack(expr_ref)
            .expect(&format!("Expression {:?} not found on the stack", expr_ref));
        match position {
            0 => {
                // The element is already on the top of the stack.
            }
            1 => {
                self.miden_code.push(encoder::Instruction::Swap);
                let stack_len = self.stack.len();
                self.stack.swap(stack_len - 1, stack_len - 2);
            }
            n if n < 16 => {
                self.miden_code.push(encoder::Instruction::MovUp(n as u32));
                let element = self.stack.remove(self.stack.len() - 1 - position);
                self.stack.push(element);
            }
            n => panic!("Element {:?} is too deep on the stack: {}", expr_ref, n),
        }
    }

    /// Exprs is highest to lowest.
    fn movup_many(&mut self, exprs: &[ExpressionRef]) {
        if self.stack.len() < exprs.len() {
            panic!("Stack is too small. Did you use a binding more than once?");
        }

        // Check if the elements are already in the right order
        // exprs[0] is the top element
        let in_order = exprs
            .iter()
            .enumerate()
            .all(|(i, expr_ref)| self.stack[self.stack.len() - 1 - i] == *expr_ref);

        if in_order {
            return;
        }

        if let Some(expr) = exprs.last() {
            self.movup(expr);
            self.movup_many(&exprs[..exprs.len() - 1]);
        }
    }

    /// Moves the top stack element to the nth position.
    fn movdn(&mut self, n: usize) {
        match n {
            0 => {}
            1 => {
                self.miden_code.push(encoder::Instruction::Swap);
                let stack_len = self.stack.len();
                self.stack.swap(stack_len - 1, stack_len - 2);
            }
            n if n < 16 => {
                self.miden_code
                    .push(encoder::Instruction::MovDown(n as u32));
                let stack_len = self.stack.len();
                let el = self.stack.remove(stack_len - 1);
                self.stack.insert(stack_len - 1 - n, el);
            }
            n => panic!("cannot move down further than to the 16th position: {}", n),
        }
    }

    /// Removes all stack elements after the top `n` elements.
    fn cleanup_stack(&mut self, n: usize) {
        // We need to first movdn the top `n` elements to the bottom of the stack.
        // Then we can start dropping elements from the top of the stack, until self.stack.len() is n.
        for _ in 0..n {
            self.movdn(self.stack.len() - 1);
        }
        while self.stack.len() > n {
            self.miden_code.push(encoder::Instruction::Drop);
            // draw an edge from top element to drop
            self.stack.pop().unwrap();
        }
    }

    fn align_stack(&mut self, expr_refs: &[ExpressionRef]) {
        self.movup_many(expr_refs);
        debug_assert!(
            self.stack.iter().rev().take(expr_refs.len()).eq(expr_refs),
            "invalid stack alignment"
        );
    }

    fn compile_expr(&mut self, expr_ref: &ExpressionRef) {
        let expr = &self.expressions[expr_ref.expr_index];

        if self.used_exprs.contains(&expr_ref.expr_index) {
            return;
        }

        self.used_exprs.push(expr_ref.expr_index);

        match expr {
            Expression::Number(n) => {
                self.miden_code.push(encoder::Instruction::Push(*n as u32));

                self.stack.push(ExpressionRef {
                    expr_index: expr_ref.expr_index,
                    nth_element: 0,
                });
            }
            Expression::FunctionCall { name, args } => {
                let func = self
                    .functions
                    .iter()
                    .rev()
                    .find(|f| f.name == name.as_str())
                    .expect("function not found")
                    .clone();
                let args = args.clone();

                assert_eq!(func.num_args, args.len());

                for arg in args.iter().rev() {
                    self.compile_expr(arg);
                }
                self.align_stack(&args);

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
                then_dependencies,
                otherwise,
                otherwise_dependencies,
            } => {
                let condition = condition.clone();
                let then = then.clone();
                let otherwise = otherwise.clone();

                self.compile_expr(&condition);
                self.movup(&condition);
                self.miden_code.push(encoder::Instruction::IfTrue);
                self.stack.pop();

                for expr in &then {
                    self.compile_expr(expr);
                }
                self.align_stack(&then);

                for _ in &then {
                    self.stack.pop();
                }

                self.miden_code.push(encoder::Instruction::IfElse);

                for expr in &otherwise {
                    self.compile_expr(expr);
                }
                self.align_stack(&otherwise);

                for _ in &otherwise {
                    self.stack.pop();
                }

                self.miden_code.push(encoder::Instruction::IfEnd);

                assert_eq!(then.len(), otherwise.len());
                for (i, _) in then.iter().enumerate().rev() {
                    self.stack.push(ExpressionRef {
                        expr_index: expr_ref.expr_index,
                        nth_element: i,
                    });
                }
            }
        }
    }

    fn compile(mut self, outputs: &[ExpressionRef]) -> Vec<encoder::Instruction<'a>> {
        for output in outputs {
            self.compile_expr(output);
        }
        self.align_stack(outputs);
        self.cleanup_stack(outputs.len());

        self.miden_code
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
        let [result] = builder.call("u32wrapping_add", &[a, b])[..] else { unreachable!() };

        let mut compiler = Compiler::default();
        compiler.expressions = builder.build();
        compiler.functions = functions.to_vec();

        let code = compiler.compile(&[result]);

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
        let [a2, a] = builder.call("dup", &[a])[..] else { unreachable!() };
        let [result] = builder.call("u32wrapping_add", &[a2, a])[..] else { unreachable!() };

        let mut compiler = Compiler::default();
        compiler.expressions = builder.build();
        compiler.functions = functions.to_vec();

        let code = compiler.compile(&[result]);

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
        let [a2, a] = builder.call("dup", &[a])[..] else { unreachable!() };
        let [result] = builder.call("u32wrapping_add", &[a, a2])[..] else { unreachable!() };

        let mut compiler = Compiler::default();
        compiler.expressions = builder.build();
        compiler.functions = functions.to_vec();

        let code = compiler.compile(&[result]);

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

        let mut compiler = Compiler::default();
        compiler.expressions = builder.build();
        compiler.functions = vec![];

        let code = compiler.compile(&[a_from_if]);
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
        let [n123] = builder.call("assert_guard", &[true_, n123])[..] else { unreachable!() };

        let mut compiler = Compiler::default();
        compiler.expressions = builder.build();
        compiler.functions = functions.to_vec();

        let code = compiler.compile(&[n123]);
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
