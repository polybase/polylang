use std::{borrow::Cow, collections::HashMap, io::Write};

use mil_parser::ast;

#[derive(Debug)]
enum Expression {
    Number(u64),
    Dup(ExpressionRef),
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
struct Function {
    name: std::borrow::Cow<'static, str>,
    num_args: usize,
    num_outputs: usize,
    /// If None, we call using `exec`, otherwise we output the instruction
    instruction: Option<&'static str>,
    pure: bool,
}

struct Compiler {
    expressions: Vec<Expression>,
    /// Bindings are names of stack elements returned by expressions.
    bindings: Vec<(ExpressionRef, String)>,
    /// Stack is a list of expression references.
    /// It's consistent with the stack of the miden code that is being generated.
    stack: Vec<ExpressionRef>,
    /// Expressions that have already been compiled.
    used_exprs: Vec<usize>,
    /// Expression -> dependency.
    expr_to_dependency: Vec<(ExpressionRef, ExpressionRef)>,
    /// Expression -> usage count.
    expr_use_count: HashMap<ExpressionRef, usize>,
    /// Functions available to call.
    functions: Vec<Function>,
    /// Generated miden code.
    miden_code: String,
    grapher: Grapher,
}

impl Compiler {
    fn new(user_functions: &[Function], grapher: Grapher) -> Self {
        let default_functions = [
            Function {
                name: Cow::Borrowed("add"),
                num_args: 2,
                num_outputs: 1,
                instruction: Some("add"),
                pure: true,
            },
            Function {
                name: Cow::Borrowed("sub"),
                num_args: 2,
                num_outputs: 1,
                instruction: Some("sub"),
                pure: true,
            },
            Function {
                name: Cow::Borrowed("dup"),
                num_args: 1,
                num_outputs: 2,
                instruction: Some("dup"),
                pure: true,
            },
            Function {
                name: Cow::Borrowed("eq"),
                num_args: 2,
                num_outputs: 1,
                instruction: Some("eq"),
                pure: true,
            },
            Function {
                name: Cow::Borrowed("not"),
                num_args: 1,
                num_outputs: 1,
                instruction: Some("not"),
                pure: true,
            },
            Function {
                name: Cow::Borrowed("assert"),
                num_args: 1,
                num_outputs: 0,
                instruction: Some("assert"),
                pure: false,
            },
            Function {
                name: Cow::Borrowed("u32wrapping_add"),
                num_args: 2,
                num_outputs: 1,
                instruction: Some("u32wrapping_add"),
                pure: true,
            },
            Function {
                name: Cow::Borrowed("u32wrapping_sub"),
                num_args: 2,
                num_outputs: 1,
                instruction: Some("u32wrapping_sub"),
                pure: true,
            },
            Function {
                name: "u32checked_shr".into(),
                num_args: 2,
                num_outputs: 1,
                instruction: Some("u32checked_shr"),
                pure: false,
            },
        ];

        let mut functions = Vec::with_capacity(default_functions.len() + user_functions.len());
        functions.extend(default_functions);
        functions.extend(user_functions.iter().cloned());

        Self {
            expressions: vec![],
            bindings: vec![],
            stack: vec![],
            used_exprs: vec![],
            expr_to_dependency: vec![],
            expr_use_count: HashMap::new(),
            functions,
            grapher,
            miden_code: String::new(),
        }
    }

    fn comment(&mut self, comment: &str) {
        self.miden_code.push_str("# ");
        self.miden_code.push_str(comment);
        self.miden_code.push_str("\n");
    }

    /// Returns (code, graph)
    fn compile(
        mut self,
        output_names: &[&str],
        dependencies: &[ExpressionRef],
    ) -> (String, String) {
        let outputs = self.find_bindings(output_names);

        let outputs_len = outputs.len();
        if outputs_len == 0 {
            panic!("no outputs");
        }

        for output in outputs.iter().rev() {
            self.compile_expr(*output);
        }

        for dep in dependencies.iter() {
            if self.used_exprs.contains(&dep.expr_index) {
                // THIS WAS THE CRUCIAL FIX! OUTPUTS ALREADY COMPILED THE DEPS, COMPILING THEM AGAIN WAS WRONG! MAYBE WE SHOULD HAVE TRY_COMPILE_EXPR?
                // TODO: bring back automatic dup-ing, remove manual dup
                continue;
            }

            self.compile_expr(*dep);
        }

        self.comment("Moving outputs to the top of the stack");
        self.movup_many(&outputs);

        self.comment("Cleaning up the stack");
        self.cleanup_stack(outputs_len);

        (self.miden_code, self.grapher.finish())
    }

    fn compile_expr(&mut self, expr_ref: ExpressionRef) {
        for dependency in self
            .expr_to_dependency
            .iter()
            .filter(|(e, _)| e.expr_index == expr_ref.expr_index)
            .map(|(_, d)| *d)
            .collect::<Vec<_>>()
        {
            if self.used_exprs.contains(&dependency.expr_index) {
                continue;
            }

            self.compile_expr(dependency);
            self.grapher
                .define_edge(&dependency.to_string(), &expr_ref.to_string());
        }

        if self.stack.contains(&expr_ref) {
        } else if self.used_exprs.contains(&expr_ref.expr_index) {
            panic!(
                "Expression {:?} is already used and not on the stack",
                expr_ref
            );
        } else {
            // Compile the expression for the first time.
            match &self.expressions[expr_ref.expr_index] {
                Expression::Number(n) => {
                    self.miden_code.push_str(&format!("push.{}\n", n));
                    self.stack.push(ExpressionRef::new(expr_ref.expr_index, 0));
                    self.used_exprs.push(expr_ref.expr_index);
                }
                Expression::Dup(e) => {
                    let e = e.clone();
                    dbg!(e, expr_ref);
                    eprintln!("{}", &self.grapher.graph);
                    if !self.used_exprs.contains(&e.expr_index) {
                        self.compile_expr(e);
                    }

                    self.dup(&e);
                    *self.stack.last_mut().unwrap() = expr_ref;
                }
                Expression::FunctionCall { name, args } => {
                    let function = self
                        .functions
                        .iter()
                        .rev()
                        .find(|f| f.name == *name)
                        .expect(&format!("Function \"{}\" not found", name));
                    let (num_args, num_outputs, call_inst) = (
                        function.num_args,
                        function.num_outputs,
                        if let Some(inst) = function.instruction {
                            std::borrow::Cow::Borrowed(inst)
                        } else {
                            std::borrow::Cow::Owned(format!("exec.{}", name))
                        },
                    );

                    if args.len() != num_args {
                        panic!(
                            "Function {} expects {} arguments, but got {}",
                            name,
                            function.num_args,
                            args.len()
                        );
                    }

                    // These clones are a hack for now, to drop the borrow on self.expressions.
                    let name = name.clone();
                    let args = (*args).clone();
                    // Compile the arguments.
                    for arg in args.iter().rev() {
                        self.grapher.define_edge(&arg.to_string(), &name);
                        self.compile_expr(*arg);
                    }
                    self.align_exprs_stack(&args);

                    self.miden_code.push_str(&call_inst);
                    self.miden_code.push_str("\n");
                    for _ in 0..args.len() {
                        self.stack.pop();
                    }
                    for i in (0..num_outputs).rev() {
                        let expr_ref = ExpressionRef::new(expr_ref.expr_index, i);

                        self.grapher.define_edge(&name, &expr_ref.to_string());

                        self.stack.push(expr_ref);
                    }
                    self.used_exprs.push(expr_ref.expr_index);
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
                    let then_dependencies = then_dependencies.clone();
                    let otherwise = otherwise.clone();
                    let otherwise_dependencies = otherwise_dependencies.clone();

                    self.compile_expr(condition);
                    self.align_exprs_stack(&[condition]);
                    self.grapher
                        .define_edge(&condition.to_string(), &expr_ref.expr_index.to_string());

                    let used_exprs_len_before = self.used_exprs.len();

                    self.miden_code.push_str("if.true\n");
                    // if.true consumes the condition from the stack.
                    self.stack.pop();

                    if then.len() > 0 || then_dependencies.len() > 0 {
                        for expr in then.iter().rev() {
                            self.compile_expr(*expr);
                        }
                        for dep in then_dependencies {
                            self.compile_expr(dep);
                        }
                        self.align_exprs_stack(&then);
                        for _ in then.iter() {
                            self.stack.pop();
                        }
                    } else {
                        // branch cannot be empty in MASM
                        self.miden_code.push_str("push.0\ndrop\n");
                    }

                    let then_used_expressions = self.used_exprs.split_off(used_exprs_len_before);

                    if otherwise.len() > 0 || otherwise_dependencies.len() > 0 {
                        self.miden_code.push_str("else\n");
                        for expr in otherwise.iter().rev() {
                            self.compile_expr(*expr);
                        }
                        for dep in otherwise_dependencies {
                            self.compile_expr(dep);
                        }

                        self.align_exprs_stack(&otherwise);
                        for _ in otherwise.iter() {
                            self.stack.pop();
                        }
                    }

                    self.miden_code.push_str("end\n");

                    for expr in then_used_expressions {
                        self.used_exprs.push(expr);
                    }

                    for i in (0..then.len()).rev() {
                        let el_expr_ref = ExpressionRef {
                            expr_index: expr_ref.expr_index,
                            nth_element: i,
                        };

                        self.stack.push(el_expr_ref);

                        // define edge from the if to the el_expr_ref
                        self.grapher.define_edge(
                            &expr_ref.expr_index.to_string(),
                            &el_expr_ref.to_string(),
                        );

                        self.grapher
                            .define_edge(&then[i].to_string(), &el_expr_ref.to_string());

                        self.grapher
                            .define_edge(&otherwise[i].to_string(), &el_expr_ref.to_string());
                    }

                    self.used_exprs.push(expr_ref.expr_index);
                }
            }
        }
    }

    /// Returns the position of the expression on the stack.
    /// If the expression is on the top of the stack, it returns 0.
    fn find_stack_element(&self, expr_ref: &ExpressionRef) -> Option<usize> {
        self.stack
            .iter()
            .rev()
            .enumerate()
            .find_map(|(i, e)| if e == expr_ref { Some(i) } else { None })
    }

    fn dup(&mut self, expr_ref: &ExpressionRef) {
        let position = self.find_stack_element(expr_ref).unwrap();
        if position >= 16 {
            panic!(
                "Stack element {:?} is too far to duplicate it (position {})",
                expr_ref, position
            );
        }
        self.miden_code.push_str(&format!("dup.{}\n", position));
        self.stack.push(*expr_ref);
    }

    fn movup(&mut self, expr_ref: &ExpressionRef) {
        let position = self
            .find_stack_element(expr_ref)
            .expect(&format!("Expression {:?} not found on the stack", expr_ref));
        match position {
            0 => {
                // The element is already on the top of the stack.
            }
            1 => {
                self.miden_code.push_str("swap\n");
                self.comment(&format!(
                    "Moved {:?} to the top, in place of {:?}",
                    expr_ref,
                    self.stack[self.stack.len() - 1],
                ));
                let stack_len = self.stack.len();
                self.stack.swap(stack_len - 1, stack_len - 2);
            }
            n if n < 16 => {
                self.miden_code.push_str(&format!("movup.{}\n", n));
                // We need to shift all of the elements above the element we're moving up.
                let element = self.stack.remove(self.stack.len() - 1 - position);
                self.stack.push(element);
                self.comment(&format!("Moved {:?} to the top", expr_ref));
            }
            n => panic!("Element {:?} is too deep on the stack: {}", expr_ref, n),
        }
    }

    fn align_exprs_stack(&mut self, exprs: &[ExpressionRef]) {
        for expr in exprs.iter().rev() {
            let count = *self.expr_use_count.get(expr).unwrap_or(&1);

            if count > 1 {
                self.dup(expr);
                self.expr_use_count.entry(*expr).and_modify(|e| *e -= 1);
            }
        }

        self.movup_many(exprs);
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
                self.miden_code.push_str("swap\n");
                let stack_len = self.stack.len();
                self.stack.swap(stack_len - 1, stack_len - 2);
                self.comment(&format!(
                    "Moved {:?} down, in place of {:?}",
                    self.stack[stack_len - 2],
                    self.stack.last().unwrap()
                ));
            }
            n if n < 16 => {
                self.miden_code.push_str(&format!("movdn.{}\n", n));
                let stack_len = self.stack.len();
                let el = self.stack.remove(stack_len - 1);
                self.stack.insert(stack_len - 1 - n, el);
                self.comment(&format!("Moved {:?} down", el));
            }
            n => panic!("cannot move down further than to the 16th position: {}", n),
        }
    }

    // TODO:
    // fn movdn_many(&mut self, how_many_els: usize, after: usize) {
    //     let in_order = (after..after + how_many_els)
    //         .rev()
    //         .all(|i| self.stack[i] == self.stack[i + how_many_els]);

    //     if in_order {
    //         return;
    //     }

    //     if how_many_els > 0 {
    //         self.movdn(how_many_els);
    //         self.movdn_many(how_many_els - 1, after);
    //     }
    // }

    /// Removes all stack elements after the top `n` elements.
    fn cleanup_stack(&mut self, n: usize) {
        // We need to first movdn the top `n` elements to the bottom of the stack.
        // Then we can start dropping elements from the top of the stack, until self.stack.len() is n.
        for _ in 0..n {
            self.movdn(self.stack.len() - 1);
        }
        while self.stack.len() > n {
            self.miden_code.push_str("drop\n");
            // draw an edge from top element to drop
            let expr_ref = self.stack.pop().unwrap();
            self.grapher.define_node(
                &format!("drop ({}:{})", expr_ref.expr_index, expr_ref.nth_element),
                "drop",
            );
            self.grapher.define_edge(
                &expr_ref.to_string(),
                &format!("drop ({}:{})", expr_ref.expr_index, expr_ref.nth_element),
            );
        }
    }

    fn find_bindings(&self, names: &[&str]) -> Vec<ExpressionRef> {
        let mut bindings = Vec::new();
        for (expr_ref, name) in self.bindings.iter().rev() {
            if names.contains(&name.as_str()) {
                bindings.push((*expr_ref, name.as_str()));
            }
        }
        // sort them in the same order as the given names array
        bindings.sort_by_key(|(_, name)| names.iter().position(|n| n == name).unwrap());
        bindings.into_iter().map(|(expr_ref, _)| expr_ref).collect()
    }
}

fn handle_ast_expression(
    expressions: &mut Vec<Expression>,
    bindings: &[(ExpressionRef, String)],
    expr_to_dependency: &mut Vec<(ExpressionRef, ExpressionRef)>,
    dependencies: &mut Vec<ExpressionRef>,
    expr_use_count: &mut HashMap<ExpressionRef, usize>,
    grapher: &mut Grapher,
    expr: ast::Node<ast::Expression>,
) -> ExpressionRef {
    let expr_ref = match expr.node {
        ast::Expression::Number(n) => {
            expressions.push(Expression::Number(n));
            let expr_ref = ExpressionRef::new(expressions.len() - 1, 0);

            grapher.define_node(
                &expr_ref.to_string(),
                &format!("{} ({}:{})", n, &expr_ref.expr_index, &expr_ref.nth_element),
            );

            expr_ref
        }
        ast::Expression::Identifier(id) => {
            // Find the binding for this identifier, and use it's expr_ref.
            let expr_ref = bindings
                .iter()
                .rev()
                .find_map(|(expr_ref, name)| {
                    if name == &id.0 {
                        Some(expr_ref.clone())
                    } else {
                        None
                    }
                })
                .expect(&format!("Could not find binding for identifier {}", &id.0));

            expr_ref
        }
        ast::Expression::FunctionCall(fc) => {
            let mut args = vec![];

            for arg in fc.args {
                args.push(handle_ast_expression(
                    expressions,
                    bindings,
                    expr_to_dependency,
                    dependencies,
                    expr_use_count,
                    grapher,
                    arg,
                ));
            }

            let expr_ref = ExpressionRef::new(expressions.len(), 0);

            grapher.define_node(
                &expr_ref.to_string(),
                &format!(
                    "{} ({}:{})",
                    fc.name.0, &expr_ref.expr_index, &expr_ref.nth_element
                ),
            );

            expressions.push(Expression::FunctionCall {
                name: fc.name.0,
                args,
            });

            for dependency in dependencies {
                expr_to_dependency.push((expr_ref, *dependency));
            }

            expr_ref
        }
    };

    expr_use_count
        .entry(expr_ref)
        .and_modify(|e| *e += 1)
        .or_insert(1);

    expr_ref
}

fn handle_statement(
    expressions: &mut Vec<Expression>,
    bindings: &mut Vec<(ExpressionRef, String)>,
    expr_to_dependency: &mut Vec<(ExpressionRef, ExpressionRef)>,
    dependencies: &mut Vec<ExpressionRef>,
    expr_use_count: &mut HashMap<ExpressionRef, usize>,
    grapher: &mut Grapher,
    statement: ast::Statement,
) {
    match statement {
        ast::Statement::Binding(binding) => {
            let expr_ref = handle_ast_expression(
                expressions,
                bindings,
                expr_to_dependency,
                dependencies,
                expr_use_count,
                grapher,
                binding.expr,
            );
            for (i, name) in binding.names.iter().enumerate() {
                let expr_ref = ExpressionRef {
                    expr_index: expr_ref.expr_index,
                    nth_element: expr_ref.nth_element + i,
                };
                grapher.define_node(
                    &expr_ref.to_string(),
                    &format!(
                        "{} ({}:{})",
                        name.0, expr_ref.expr_index, expr_ref.nth_element
                    ),
                );
                bindings.push((expr_ref, name.0.clone()));
            }
        }
        ast::Statement::FunctionCall(fc) => {
            let impure = fc.node.name.0 == "assert";

            let expr_ref = handle_ast_expression(
                expressions,
                bindings,
                expr_to_dependency,
                dependencies,
                expr_use_count,
                grapher,
                ast::Node::new(ast::Expression::FunctionCall(fc.node), fc.span),
            );

            if impure {
                dependencies.push(expr_ref);
            }
        }
        ast::Statement::If(if_) => {
            let condition = handle_ast_expression(
                expressions,
                bindings,
                expr_to_dependency,
                dependencies,
                expr_use_count,
                grapher,
                if_.condition,
            );
            let mut then_bindings = bindings.clone();
            let mut then_dependencies = vec![];
            for statement in if_.then {
                handle_statement(
                    expressions,
                    &mut then_bindings,
                    expr_to_dependency,
                    &mut then_dependencies,
                    expr_use_count,
                    grapher,
                    statement,
                );
            }
            let mut else_bindings = bindings.clone();
            let mut else_dependencies = vec![];
            for statement in if_.otherwise {
                handle_statement(
                    expressions,
                    &mut else_bindings,
                    expr_to_dependency,
                    &mut else_dependencies,
                    expr_use_count,
                    grapher,
                    statement,
                );
            }

            // We want to find the bindings that are in both then_bindings and else_bindings.
            // Those are the bindings we can use outside the if.

            let mut common_new_bindings = vec![];
            for binding in &then_bindings[bindings.len()..] {
                if let Some(else_binding) = else_bindings.iter().find(|b| b.1 == binding.1) {
                    common_new_bindings.push((binding, else_binding));
                }
            }

            let if_has_any_dependencies =
                then_dependencies.len() > 0 || else_dependencies.len() > 0;

            let expr = Expression::If {
                condition,
                then: common_new_bindings
                    .iter()
                    .map(|(then_binding, _)| then_binding.0.clone())
                    .collect(),
                then_dependencies,
                otherwise: common_new_bindings
                    .iter()
                    .map(|(_, else_binding)| else_binding.0.clone())
                    .collect(),
                otherwise_dependencies: else_dependencies,
            };

            let expr_ref = ExpressionRef::new(expressions.len(), 0);
            expressions.push(expr);
            for dependency in dependencies.iter_mut() {
                expr_to_dependency.push((expr_ref.clone(), dependency.clone()));
            }
            if if_has_any_dependencies {
                dependencies.push(expr_ref.clone());
            }

            for (i, (then_binding, _)) in common_new_bindings.iter().enumerate() {
                let expr_ref = ExpressionRef {
                    expr_index: expr_ref.expr_index,
                    nth_element: i,
                };

                bindings.push((expr_ref, then_binding.1.clone()));

                grapher.define_node(
                    &expr_ref.to_string(),
                    &format!(
                        "{} ({}:{})",
                        then_binding.1, expr_ref.expr_index, expr_ref.nth_element
                    ),
                );
            }

            // TODO: we need to remove the bindings that appear in bindings, but not in either then_bindings or else_bindings

            grapher.define_node(
                &expr_ref.expr_index.to_string(),
                &format!("if ({}:{})", expr_ref.expr_index, expr_ref.nth_element),
            );
        }
    }
}

struct Grapher {
    graph: String,
}

impl Grapher {
    fn new() -> Self {
        Self {
            graph: "digraph {\n".to_string(),
        }
    }

    fn define_node(&mut self, id: &str, label: &str) {
        self.graph
            .push_str(&format!("  \"{}\" [label=\"{}\"];\n", id, label));
    }

    fn define_edge(&mut self, from: &str, to: &str) {
        self.graph
            .push_str(&format!("  \"{}\" -> \"{}\";\n", from, to));
    }

    fn finish(self) -> String {
        self.graph + "\n}"
    }
}

pub(crate) fn compile(code: &str) -> (String, String) {
    let program = mil_parser::mil::ProgramParser::new().parse(code).unwrap();

    let mut miden_code = String::new();
    let mut functions = Vec::<Function>::new();
    for function in program.functions {
        let mut compiler = Compiler::new(&functions, Grapher::new());

        let num_args = function.args.len();
        for arg in function.args.into_iter().rev() {
            // Insert dummy expression
            compiler.expressions.push(Expression::Number(0));
            let expr_ref = ExpressionRef::new(compiler.expressions.len() - 1, 0);
            // Pretend that it's already compiled
            compiler.used_exprs.push(expr_ref.expr_index);
            // Add it on the stack
            compiler.stack.push(expr_ref);
            // Make it usable in code
            compiler.bindings.push((expr_ref, arg.0));
        }

        let mut dependencies = vec![];
        for statement in function.body {
            handle_statement(
                &mut compiler.expressions,
                &mut compiler.bindings,
                &mut compiler.expr_to_dependency,
                &mut dependencies,
                &mut compiler.expr_use_count,
                &mut compiler.grapher,
                statement,
            );
        }

        let (mc, _) = compiler.compile(
            &function
                .outputs
                .iter()
                .map(|a| a.0.as_str())
                .collect::<Vec<_>>(),
            &dependencies,
        );
        miden_code.push_str(&format!("proc.{}\n{}\nend\n", function.name.0, mc));

        functions.push(Function {
            name: Cow::Owned(function.name.0),
            num_args,
            num_outputs: function.outputs.len(),
            instruction: None,
            pure: false,
        });
    }

    (miden_code, "".to_string())
}

pub(crate) fn run() {
    let expressions = vec![
        Expression::Number(1),
        Expression::Number(2),
        Expression::FunctionCall {
            name: "dup".to_string(),
            args: vec![ExpressionRef::new(0, 0)],
        },
        // add(duppedA, b)
        Expression::FunctionCall {
            name: "add".to_string(),
            args: vec![ExpressionRef::new(2, 0), ExpressionRef::new(1, 0)],
        },
    ];

    let bindings = vec![
        // a = expressions[0]
        (ExpressionRef::new(0, 0), "a".to_string()),
        // b = expressions[1]
        (ExpressionRef::new(1, 0), "b".to_string()),
        // out0 = expressions[3]
        (ExpressionRef::new(3, 0), "out0".to_string()),
    ];

    let compiler = Compiler {
        expressions,
        bindings,
        stack: Vec::<ExpressionRef>::new(),
        used_exprs: Vec::<usize>::new(),
        expr_to_dependency: Vec::<(ExpressionRef, ExpressionRef)>::new(),
        expr_use_count: HashMap::new(),
        functions: vec![],
        grapher: Grapher::new(),
        miden_code: String::new(),
    };

    let (code, _) = compiler.compile(&["out0"], &[]);
    println!("{}", code);
}

pub(crate) fn run2() {
    // let code = r#"
    //     a = 2;
    //     b = 3;

    //     (b2, b3) = dup(b);

    //     if (1) {
    //         c = 4;
    //     } else {
    //         c = 5;
    //     }

    //     out = add(a, c);
    // "#;

    // let code = r#"
    //     condition = eq(1, 1);

    //     zero = 0;
    //     if (condition) {
    //         a = add(zero, 1);
    //         b = 2;
    //     } else {
    //         a = not(zero);
    //         b = 20;
    //     }

    //     out = sub(a, b);
    // "#; // result should be b - a = 2 - 1 = 1

    // let code = r#"
    //     if (1) {
    //         a = 1;
    //         b = 2;
    //     } else {
    //         a = 3;
    //         b = 4;
    //     }

    //     out = b;
    // "#;

    // let code = r#"
    //     function succ(x): (succed) {
    //         succed = add(x, 1);
    //     }

    //     function main(): (result) {
    //         result = succ(2);
    //     }
    // "#;

    // return addInt32(uint32WrappingSub(negativeOne(), a), 1);
    /*
      addInt32
      let signA = a >> 31;
        let signB = b >> 31;
        let c = uint32WrappingAdd(a, b);
        let signC = c >> 31;

        comment('https://www.doc.ic.ac.uk/~eedwards/compsys/arithmetic/index.html');
        if (signA == signB) {
            assert(signA == signC, 'addInt32 overflow, wrong sign');
        }

        return c;
    */
    let code = r#"
        function negativeOne(): (num) {
            num = 4294967295;
        }

        function negate(a): (negated) {
            negated = add(1, u32wrapping_sub(a, negativeOne()));
        }

        function addInt32(a, b): (result) {
            (a, a2) = dup(a);
            (b, b2) = dup(b);

            signA = u32checked_shr(31, a2);
            signB = u32checked_shr(31, b2);

            c = u32wrapping_add(b, a);
            (c, c2) = dup(c);
            signC = u32checked_shr(31, c2);

            (signA, signA2) = dup(signA);
            if (eq(signA2, signB)) {
                assert(eq(signA, signC));
            } else {}

            result = c;
        }

        function main(): (result) {
            result = addInt32(negativeOne(), 1);
        }
    "#;

    // let code = r#"
    //     function main(): (out0) {
    //         (dupped, out0) = dup(1);
    //         assert(dupped);
    //     }
    // "#;

    let (code, graph) = compile(code);
    println!("{}", code);

    let mut file = std::fs::File::create("graph.dot").unwrap();
    file.write_all(graph.as_bytes()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movdn() {
        let mut compiler = Compiler::new(&[], Grapher::new());
        compiler.stack = vec![ExpressionRef::new(0, 0), ExpressionRef::new(1, 0)];

        compiler.movdn(1);
        assert_eq!(
            &compiler.stack,
            &[ExpressionRef::new(1, 0), ExpressionRef::new(0, 0)],
        );
    }

    #[test]
    fn test_movdn_2() {
        let mut compiler = Compiler::new(&[], Grapher::new());
        compiler.stack = vec![
            ExpressionRef::new(0, 0),
            ExpressionRef::new(1, 0),
            ExpressionRef::new(2, 0),
        ];

        compiler.movdn(1);
        assert_eq!(
            &compiler.stack,
            &[
                ExpressionRef::new(0, 0),
                ExpressionRef::new(2, 0),
                ExpressionRef::new(1, 0),
            ],
        );
    }

    #[test]
    fn test_movdn_3() {
        let mut compiler = Compiler::new(&[], Grapher::new());
        compiler.stack = vec![
            ExpressionRef::new(0, 0),
            ExpressionRef::new(1, 0),
            ExpressionRef::new(2, 0),
        ];

        compiler.movdn(2);
        assert_eq!(
            &compiler.stack,
            &[
                ExpressionRef::new(2, 0),
                ExpressionRef::new(0, 0),
                ExpressionRef::new(1, 0),
            ],
        );
    }

    #[test]
    fn test_movup() {
        let mut compiler = Compiler::new(&[], Grapher::new());
        compiler.stack = vec![ExpressionRef::new(0, 0), ExpressionRef::new(1, 0)];

        compiler.movup(&ExpressionRef::new(0, 0));
        // Should swap 1:0 with 0:0
        assert_eq!(
            &compiler.stack,
            &[ExpressionRef::new(1, 0), ExpressionRef::new(0, 0)],
        );
    }

    #[test]
    fn test_movup_2() {
        let mut compiler = Compiler::new(&[], Grapher::new());
        compiler.stack = vec![
            ExpressionRef::new(0, 0),
            ExpressionRef::new(1, 0),
            ExpressionRef::new(2, 0),
        ];

        compiler.movup(&ExpressionRef::new(0, 0));
        assert_eq!(
            &compiler.stack,
            &[
                ExpressionRef::new(1, 0),
                ExpressionRef::new(2, 0),
                ExpressionRef::new(0, 0),
            ],
        );
    }

    #[test]
    fn test_movup_3() {
        let mut compiler = Compiler::new(&[], Grapher::new());
        compiler.stack = vec![
            ExpressionRef::new(0, 0),
            ExpressionRef::new(1, 0),
            ExpressionRef::new(2, 0),
            ExpressionRef::new(3, 0),
        ];

        compiler.movup(&ExpressionRef::new(0, 0));
        assert_eq!(
            &compiler.stack,
            &[
                ExpressionRef::new(1, 0),
                ExpressionRef::new(2, 0),
                ExpressionRef::new(3, 0),
                ExpressionRef::new(0, 0),
            ],
        );
    }

    #[test]
    fn test_movup_many() {
        let mut compiler = Compiler::new(&[], Grapher::new());
        compiler.stack = vec![
            ExpressionRef::new(0, 0),
            ExpressionRef::new(1, 0),
            ExpressionRef::new(2, 0),
        ];

        compiler.movup_many(&[ExpressionRef::new(2, 0), ExpressionRef::new(1, 0)]);
        // Stack is already in order, nothing to do
        assert_eq!(
            &compiler.stack,
            &[
                ExpressionRef::new(0, 0),
                ExpressionRef::new(1, 0),
                ExpressionRef::new(2, 0),
            ],
        );
        assert!(!compiler.miden_code.contains("movup") && !compiler.miden_code.contains("swap"));
    }

    fn run(
        miden_code: &str,
    ) -> Result<miden_processor::ProgramOutputs, miden_processor::ExecutionError> {
        let assembler =
            miden::Assembler::new().with_module_provider(miden_stdlib::StdLibrary::default());
        let program = assembler
            .compile(&miden_code)
            .expect("Failed to compile miden assembly");

        let mut process = miden_processor::Process::new_debug(
            miden::ProgramInputs::new(&[], &[], vec![]).unwrap(),
        );

        process.execute(&program)
    }

    macro_rules! test {
        ($mil_code:expr, $expected_stack:expr) => {
            let (mut code, _) = compile($mil_code);
            code.push_str("\nbegin\nexec.main\nend\n");

            let outputs = run(&code).unwrap();
            let (left, right) = outputs.stack().split_at($expected_stack.len());
            assert_eq!(left, $expected_stack);
            for &value in right {
                assert_eq!(
                    value, 0,
                    "Value on the stack (after expected values) is not zero"
                );
            }
        };
        (failure $mil_code:expr) => {
            let (mut code, _) = compile($mil_code);
            code.push_str("\nbegin\nexec.main\nend\n");
            let result = run(&code);
            assert!(result.is_err());
        };
    }

    #[test]
    fn test_add_1_2() {
        test!(
            r#"
            function main(): (out0) {
                out0 = add(1, 2);
            }
        "#,
            &[3]
        );
    }

    #[test]
    fn test_sub_1_2() {
        // This should be 2 - 1 = 1, because arguments are pushed with the top element being the first argument
        test!(
            r#"
            function main(): (out0) {
                out0 = sub(1, 2);
            }
        "#,
            &[1]
        );
    }

    #[test]
    fn test_if() {
        test!(
            r#"
            function main(): (out0) {
                if (1) {
                    out0 = 1;
                } else {
                    out0 = 2;
                }
            }
        "#,
            &[1]
        );
    }

    #[test]
    fn test_if_2() {
        test!(
            r#"
            function main(): (out0) {
                if (0) {
                    out0 = 1;
                } else {
                    out0 = 2;
                }
            }
        "#,
            &[2]
        );
    }

    #[test]
    fn test_if_3() {
        test!(
            r#"
            function main(): (out0) {
                if (1) {
                    a = 1;
                    b = 2;
                } else {
                    a = 3;
                    b = 4;
                }
    
                out0 = a;
            }
        "#,
            &[1]
        );

        test!(
            r#"
            function main(): (out0) {
                if (0) {
                    a = 1;
                    b = 2;
                } else {
                    a = 3;
                    b = 4;
                }
    
                out0 = a;
            }
        "#,
            &[3]
        );

        test!(
            r#"
            function main(): (out0) {
                if (1) {
                    a = 1;
                    b = 2;
                } else {
                    a = 3;
                    b = 4;
                }
    
                out0 = b;
            }
        "#,
            &[2]
        );

        test!(
            r#"
            function main(): (out0) {
                if (0) {
                    a = 1;
                    b = 2;
                } else {
                    b = 4;
                    a = 3;
                }
    
                out0 = b;
            }
        "#,
            &[4]
        );
    }

    #[test]
    fn test_complicated() {
        test!(
            r#"
            function main(): (out0) {
                condition = eq(1, 1);
    
                zero = 0;
                if (condition) {
                    a = add(zero, 1);
                    b = 2;
                } else {
                    a = not(zero);
                    b = 20;
                }
    
                out0 = sub(a, b);
            }
        "#, // result should be b - a = 2 - 1 = 1
            &[1]
        );
    }

    #[test]
    fn test_complicated_2() {
        test!(
            r#"
            function main(): (out0, out1, out2) {
                a = 223;
                b = 123;
                out0 = sub(b, a);
                out1 = 2;
                out2 = 3;
                assert(eq(1, 1));
            }
        "#,
            &[100, 2, 3]
        );
    }

    #[test]
    fn test_complicated_3() {
        test!(
            r#"
            function add2(a, b): (result) {
                (a, a2) = dup(a);
                (b, b2) = dup(b);

                result = add(a, b);
            }

            function main(): (out0) {
                out0 = add2(1, 2);
            }
        "#,
            &[3]
        );
    }

    #[test]
    fn test_assert() {
        test!(
            r#"
            function main(): (out0) {
                assert(eq(1, 1));
                out0 = 2;
            }
        "#,
            &[2]
        );
    }

    #[test]
    fn test_assert_2() {
        test!(
            failure
            r#"
            function main(): (out0) {
                assert(eq(1, 0));
                out0 = 2;
            }
        "#
        );
    }

    #[test]
    fn test_assert_3() {
        test!(
            failure
            r#"
            function main(): (out0) {
                out0 = 1;
                assert(eq(0, 1));
            }
        "#
        );
    }

    #[test]
    fn test_dup() {
        test!(
            r#"
            function main(): (out0) {
                a = 1;
                (a, a2) = dup(a);
    
                out0 = add(a2, a);
            }
        "#,
            &[2]
        );
    }

    #[test]
    fn test_calling_user_functions() {
        test!(
            r#"
            function two(): (out0) {
                out0 = 2;
            }
    
            function main(): (out0) {
                out0 = two();
            }
        "#,
            &[2]
        );
    }
}
