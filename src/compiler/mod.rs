mod boolean;
mod encoder;
mod string;
mod uint32;
mod uint64;

use std::{collections::HashMap, ops::Deref};

use crate::ast::{self, Expression, Statement};

macro_rules! comment {
    ($compiler:expr, $($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $compiler.comment(format!($($arg)*));
    };
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum PrimitiveType {
    Boolean,
    UInt32,
    UInt64,
}

impl PrimitiveType {
    fn miden_width(&self) -> u32 {
        match self {
            PrimitiveType::Boolean => boolean::WIDTH,
            PrimitiveType::UInt32 => uint32::WIDTH,
            PrimitiveType::UInt64 => uint64::WIDTH,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Struct {
    name: String,
    fields: Vec<(String, PrimitiveType)>,
}

#[derive(Clone, Debug, PartialEq)]
enum Type {
    PrimitiveType(PrimitiveType),
    String,
    Struct(Struct),
}

impl Type {
    fn miden_width(&self) -> u32 {
        match self {
            Type::PrimitiveType(pt) => pt.miden_width(),
            Type::String => string::WIDTH,
            Type::Struct(struct_) => struct_.fields.iter().map(|(_, t)| t.miden_width()).sum(),
        }
    }
}

fn new_struct(compiler: &mut Compiler, struct_: Struct) -> Symbol {
    let symbol = compiler.memory.allocate_symbol(Type::Struct(struct_));

    symbol
}

fn struct_field(struct_symbol: &Symbol, field_name: &str) -> Option<Symbol> {
    let struct_ = match &struct_symbol.type_ {
        Type::Struct(struct_) => struct_,
        t => panic!("expected struct, got: {:?}", t),
    };

    let mut offset = 0;
    for (name, field_type) in &struct_.fields {
        if name == field_name {
            return Some(Symbol {
                type_: Type::PrimitiveType(*field_type),
                memory_addr: struct_symbol.memory_addr + offset,
            });
        }

        offset += field_type.miden_width();
    }

    None
}

#[derive(Debug, Clone)]
pub(crate) struct Symbol {
    type_: Type,
    memory_addr: u32,
}

#[derive(Debug, Clone)]
struct Contract<'ast> {
    name: String,
    fields: Vec<(String, Type)>,
    functions: Vec<(String, &'ast ast::Function)>,
}

#[derive(Clone)]
enum Function<'ast> {
    AST(&'ast ast::Function),
    Builtin(Box<&'static dyn Fn(&mut Compiler, &[Symbol]) -> Symbol>),
}

impl std::fmt::Debug for Function<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Function::AST(ast) => write!(f, "Function::AST({:?})", ast),
            Function::Builtin(_) => write!(f, "Function::Builtin"),
        }
    }
}

#[derive(Debug, Clone)]
struct Scope<'ast> {
    symbols: Vec<(String, Symbol)>,
    functions: Vec<(String, Function<'ast>)>,
    contracts: Vec<(String, Contract<'ast>)>,
}

impl<'ast> Scope<'ast> {
    fn new() -> Self {
        Scope {
            symbols: vec![],
            functions: vec![],
            contracts: vec![],
        }
    }

    fn with(&mut self, body: impl FnOnce(&mut Self)) {
        let symbols_start_len = self.symbols.len();
        let functions_start_len = self.functions.len();
        body(self);
        self.symbols.truncate(symbols_start_len);
        self.functions.truncate(functions_start_len);
    }

    fn add_symbol(&mut self, name: String, symbol: Symbol) {
        self.symbols.push((name, symbol));
    }

    fn find_symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbols
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s)
    }

    fn add_function(&mut self, name: String, function: Function<'ast>) {
        self.functions.push((name, function));
    }

    fn find_function(&self, name: &str) -> Option<&Function<'ast>> {
        self.functions
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f)
    }

    fn add_contract(&mut self, name: String, contract: Contract<'ast>) {
        if self.find_contract(&name).is_some() {
            panic!("Contract {} already exists", name);
        }

        self.contracts.push((name, contract));
    }

    fn find_contract(&self, name: &str) -> Option<&Contract<'ast>> {
        self.contracts
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c)
    }
}

#[derive(Copy, Clone)]
enum ValueSource {
    Immediate(u32),
    Memory(u32),
    Stack,
}

impl ValueSource {
    fn load(&self, instructions: &mut Vec<encoder::Instruction>) {
        match self {
            ValueSource::Immediate(v) => instructions.push(encoder::Instruction::Push(*v)),
            ValueSource::Memory(addr) => {
                instructions.push(encoder::Instruction::MemLoad(Some(*addr)));
            }
            ValueSource::Stack => {}
        }
    }
}

struct Memory {
    static_alloc_ptr: u32,
}

impl Memory {
    fn new() -> Self {
        Memory {
            // 0 is reserved for the null pointer
            // 1, 2 is reserved for the error string
            static_alloc_ptr: 3,
        }
    }

    fn allocate(&mut self, size: u32) -> u32 {
        let addr = self.static_alloc_ptr;
        self.static_alloc_ptr += size;
        addr
    }

    fn allocate_symbol(&mut self, type_: Type) -> Symbol {
        let addr = self.allocate(type_.miden_width());
        Symbol {
            type_,
            memory_addr: addr,
        }
    }

    // write(vec![], addr, &[ValueSource::Immediate(0), ValueSource::Immediate(1)])
    // will set addr to 0 and addr + 1 to 1
    fn write(
        &self,
        instructions: &mut Vec<encoder::Instruction>,
        start_addr: u32,
        values: &[ValueSource],
    ) {
        let mut addr = start_addr;
        for v in values {
            v.load(instructions);
            instructions.push(encoder::Instruction::MemStore(Some(addr)));
            instructions.push(encoder::Instruction::Drop);
            addr += 1;
        }
    }

    // read reads the values from the memory starting at start_addr and pushes them to the stack
    // the top most stack item will be the value of start_addr
    // the bottom most stack item will be the value of start_addr + count - 1
    fn read(&self, instructions: &mut Vec<encoder::Instruction>, start_addr: u32, count: u32) {
        for i in 1..=count {
            ValueSource::Memory(start_addr + count - i).load(instructions);
        }
    }
}

pub(crate) struct Compiler<'ast, 'b> {
    instructions: &'b mut Vec<encoder::Instruction<'ast>>,
    memory: &'b mut Memory,
}

impl<'ast, 'b> Compiler<'ast, 'b> {
    fn new(instructions: &'b mut Vec<encoder::Instruction<'ast>>, memory: &'b mut Memory) -> Self {
        Compiler {
            instructions,
            memory,
        }
    }

    fn comment(&mut self, comment: String) {
        self.instructions
            .push(encoder::Instruction::Comment(comment));
    }
}

fn compile_expression(expr: &Expression, compiler: &mut Compiler, scope: &Scope) -> Symbol {
    comment!(compiler, "Compiling expression {expr:?}");

    let symbol = match expr {
        Expression::Ident(id) => scope.find_symbol(id).unwrap().clone(),
        Expression::Primitive(ast::Primitive::Number(n)) => uint32::new(compiler, *n as u32),
        Expression::Primitive(ast::Primitive::String(s)) => string::new(compiler, s),
        Expression::Add(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_add(compiler, &a, &b)
        }
        Expression::Subtract(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_sub(compiler, &a, &b)
        }
        Expression::Equal(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_eq(compiler, &a, &b)
        }
        Expression::Call(func, args) => {
            let func_name = match func.deref() {
                Expression::Ident(id) => id,
                _ => panic!("expected function name"),
            };
            let func = scope.find_function(func_name).unwrap();
            let mut args_symbols = vec![];
            for arg in args {
                args_symbols.push(compile_expression(arg, compiler, scope));
            }

            match func {
                Function::AST(a) => {
                    compile_ast_function_call(a, compiler, &mut scope.clone(), &args_symbols)
                }
                Function::Builtin(f) => f(compiler, &args_symbols),
            }
        }
        Expression::Assign(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            assert_eq!(a.type_, b.type_);

            compiler
                .memory
                .read(compiler.instructions, b.memory_addr, b.type_.miden_width());
            compiler.memory.write(
                compiler.instructions,
                a.memory_addr,
                &vec![ValueSource::Stack; b.type_.miden_width() as usize],
            );

            a
        }
        Expression::Dot(a, b) => {
            let a = compile_expression(a, compiler, scope);

            struct_field(&a, b).unwrap()
        }
        Expression::GreaterThanOrEqual(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_gte(compiler, &a, &b)
        }
        e => unimplemented!("{:?}", e),
    };

    comment!(
        compiler,
        "Compiled expression {expr:?} to symbol {symbol:?}",
    );

    symbol
}

fn compile_statement(
    statement: &Statement,
    compiler: &mut Compiler,
    scope: &mut Scope,
    return_result: &mut Symbol,
) {
    match statement {
        Statement::Return(expr) => {
            let symbol = compile_expression(expr, compiler, scope);
            compiler.memory.read(
                &mut compiler.instructions,
                symbol.memory_addr,
                symbol.type_.miden_width(),
            );
            compiler.memory.write(
                &mut compiler.instructions,
                return_result.memory_addr,
                &vec![ValueSource::Stack; symbol.type_.miden_width() as usize],
            );
            compiler.instructions.push(encoder::Instruction::Abstract(
                encoder::AbstractInstruction::Return,
            ));
        }
        Statement::If(ast::If {
            condition,
            then_statements,
            else_statements,
        }) => {
            let mut condition_instructions = vec![];
            let mut condition_compiler =
                Compiler::new(&mut condition_instructions, compiler.memory);
            let condition_symbol = compile_expression(condition, &mut condition_compiler, scope);
            assert_eq!(
                condition_symbol.type_,
                Type::PrimitiveType(PrimitiveType::Boolean)
            );
            condition_compiler.memory.read(
                &mut condition_compiler.instructions,
                condition_symbol.memory_addr,
                condition_symbol.type_.miden_width(),
            );

            let mut body_instructions = vec![];
            let mut body_compiler = Compiler::new(&mut body_instructions, compiler.memory);
            for statement in then_statements {
                compile_statement(statement, &mut body_compiler, scope, return_result);
            }

            let mut else_body_instructions = vec![];
            let mut else_body_compiler =
                Compiler::new(&mut else_body_instructions, compiler.memory);
            for statement in else_statements {
                compile_statement(statement, &mut else_body_compiler, scope, return_result);
            }

            compiler.instructions.push(encoder::Instruction::If {
                condition: condition_instructions,
                then: body_instructions,
                else_: else_body_instructions,
            })
        }
        Statement::Expression(expr) => {
            compile_expression(expr, compiler, scope);
        }
        st => unimplemented!("{:?}", st),
    }
}

fn compile_ast_function_call(
    function: &ast::Function,
    compiler: &mut Compiler,
    scope: &mut Scope,
    args: &[Symbol],
) -> Symbol {
    let mut function_instructions = vec![];
    let mut function_compiler = Compiler::new(&mut function_instructions, compiler.memory);

    // let old_symbols = std::mem::replace(&mut scope.symbols, vec![]);
    let mut return_result = function_compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
    scope.with(|scope| {
        for (arg, param) in args.iter().zip(function.parameters.iter()) {
            scope.add_symbol(param.name.clone(), arg.clone());
        }

        for statement in &function.statements {
            compile_statement(statement, &mut function_compiler, scope, &mut return_result);
        }
    });

    compiler.instructions.push(encoder::Instruction::Abstract(
        encoder::AbstractInstruction::InlinedFunction(function_instructions),
    ));

    // scope.symbols = old_symbols;

    return_result
}

fn cast(compiler: &mut Compiler, from: &Symbol, to: &Symbol) {
    match (&from.type_, &to.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::cast_from_uint32(compiler, from, to),
        x => unimplemented!("{:?}", x),
    }
}

fn compile_add(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::add(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::add(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::add(compiler, a, &b_u64)
        }
        e => unimplemented!("{:?}", e),
    }
}

fn compile_sub(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::sub(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::sub(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::sub(compiler, a, &b_u64)
        }
        e => unimplemented!("{:?}", e),
    }
}

fn compile_eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::eq(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::eq(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::eq(compiler, a, &b_u64)
        }
        e => unimplemented!("{:?}", e),
    }
}

fn compile_gte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::gte(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::gte(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::gte(compiler, a, &b_u64)
        }
        e => unimplemented!("{:?}", e),
    }
}

fn prepare_scope(program: &ast::Program) -> Scope {
    let mut scope = Scope::new();

    scope.add_function(
        "assert".to_string(),
        Function::Builtin(Box::new(&|compiler, args| {
            let condition = args.get(0).unwrap();
            let message = args.get(1).unwrap();

            assert_eq!(condition.type_, Type::PrimitiveType(PrimitiveType::Boolean));
            assert_eq!(message.type_, Type::String);

            let mut failure_branch = vec![];
            let mut failure_compiler = Compiler::new(&mut failure_branch, compiler.memory);

            let str_len = string::length(&mut failure_compiler, message);
            let str_data_ptr = string::data_ptr(&mut failure_compiler, message);

            failure_compiler.memory.write(
                &mut failure_compiler.instructions,
                1,
                &vec![
                    ValueSource::Memory(str_len.memory_addr),
                    ValueSource::Memory(str_data_ptr.memory_addr),
                ],
            );

            failure_compiler
                .instructions
                .push(encoder::Instruction::Push(0));
            failure_compiler
                .instructions
                .push(encoder::Instruction::Assert);

            compiler.instructions.push(encoder::Instruction::If {
                condition: vec![encoder::Instruction::MemLoad(Some(condition.memory_addr))],
                then: vec![],
                // fail on purpose with assert(0)
                else_: failure_branch,
            });

            Symbol {
                type_: Type::PrimitiveType(PrimitiveType::Boolean),
                memory_addr: 0,
            }
        })),
    );

    for node in &program.nodes {
        match node {
            ast::RootNode::Contract(c) => {
                let mut contract = Contract {
                    name: c.name.clone(),
                    functions: vec![],
                    fields: vec![],
                };

                for item in &c.items {
                    match item {
                        ast::ContractItem::Field(f) => {
                            contract.fields.push((
                                f.name.clone(),
                                match f.type_ {
                                    ast::Type::String => todo!(),
                                    ast::Type::Number => Type::PrimitiveType(PrimitiveType::UInt32),
                                },
                            ));
                        }
                        ast::ContractItem::Function(f) => {
                            contract.functions.push((f.name.clone(), f));
                        }
                        ast::ContractItem::Index(_) => todo!(),
                    }
                }

                scope.add_contract(contract.name.clone(), contract);
            }
            ast::RootNode::Function(function) => scope
                .functions
                .push((function.name.clone(), Function::AST(function))),
        }
    }

    scope
}

pub enum CompileTimeArg {
    U32(u32),
    Record(HashMap<String, u32>),
}

pub fn compile(
    program: ast::Program,
    contract_name: Option<&str>,
    function_name: &str,
    args: &[CompileTimeArg],
    this: Option<HashMap<String, u32>>,
) -> String {
    let mut scope = prepare_scope(&program);
    let contract = contract_name.map(|name| scope.find_contract(name).unwrap());
    let contract_struct = contract.map(|contract| Struct {
        name: contract.name.clone(),
        fields: contract
            .fields
            .iter()
            .map(|(name, field)| {
                (
                    name.clone(),
                    match field {
                        Type::PrimitiveType(p) => p.clone(),
                        Type::String => todo!(),
                        Type::Struct(_) => todo!(),
                    },
                )
            })
            .collect(),
    });
    let function = contract
        .and_then(|c| {
            c.functions
                .iter()
                .find(|(name, _)| name == function_name)
                .map(|(_, f)| *f)
        })
        .or_else(|| match scope.find_function(function_name) {
            Some(Function::AST(f)) => Some(f),
            Some(Function::Builtin(_)) => todo!(),
            None => None,
        })
        .unwrap();

    let mut instructions = vec![];
    let mut memory = Memory::new();

    {
        let mut compiler = Compiler::new(&mut instructions, &mut memory);

        if let Some(contract) = contract {
            let this_symbol = new_struct(&mut compiler, contract_struct.as_ref().unwrap().clone());

            if let Some(this) = this {
                for (field, value) in this {
                    let field_symbol = struct_field(&this_symbol, &field).unwrap();
                    compiler.memory.write(
                        compiler.instructions,
                        field_symbol.memory_addr,
                        &[ValueSource::Immediate(value)],
                    );
                }
            }

            scope.add_symbol("this".to_string(), this_symbol);
        }

        let arg_symbols = args
            .iter()
            .map(|arg| match arg {
                CompileTimeArg::U32(n) => uint32::new(&mut compiler, *n),
                CompileTimeArg::Record(sv) => {
                    let struct_ = contract_struct.as_ref().unwrap().clone();

                    let symbol = new_struct(&mut compiler, struct_);
                    for (field_name, value) in sv {
                        let field_symbol = struct_field(&symbol, field_name).unwrap();
                        compiler.memory.write(
                            &mut compiler.instructions,
                            field_symbol.memory_addr,
                            &[ValueSource::Immediate(*value)],
                        );
                    }

                    symbol
                }
            })
            .collect::<Vec<_>>();

        let result = compile_ast_function_call(function, &mut compiler, &mut scope, &arg_symbols);
        compiler.memory.read(
            &mut compiler.instructions,
            result.memory_addr,
            result.type_.miden_width(),
        );

        if let Some(this) = scope.find_symbol("this") {
            compiler.memory.read(
                &mut compiler.instructions,
                this.memory_addr,
                this.type_.miden_width(),
            );
        }

        for symbol in arg_symbols {
            compiler.memory.read(
                &mut compiler.instructions,
                symbol.memory_addr,
                symbol.type_.miden_width(),
            )
        }
    }

    let instructions = encoder::unabstract(
        instructions,
        &mut |size| memory.allocate(size),
        &mut None,
        &mut None,
        false,
    );

    let mut miden_code = String::new();
    miden_code.push_str("use.std::math::u64\n");
    miden_code.push_str("begin\n");
    for instruction in instructions {
        miden_code.push_str("  ");
        instruction
            .encode(unsafe { miden_code.as_mut_vec() })
            .unwrap();
        miden_code.push_str("\n");
    }
    miden_code.push_str("end\n");

    miden_code
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_compile_function() {
        let main = ast::Function {
            name: "add_1_2".to_string(),
            parameters: vec![],
            statements: vec![ast::Statement::Return(ast::Expression::Add(
                Box::new(ast::Expression::Primitive(ast::Primitive::Number(1.0))),
                Box::new(ast::Expression::Primitive(ast::Primitive::Number(2.0))),
            ))],
            statements_code: String::new(),
        };

        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory);
        compile_ast_function_call(&main, &mut compiler, &mut Scope::new(), &[]);

        assert_eq!(
            compiler.instructions,
            &mut vec![encoder::Instruction::Abstract(
                encoder::AbstractInstruction::InlinedFunction(vec![
                    encoder::Instruction::Push(1),
                    encoder::Instruction::MemStore(Some(2)),
                    encoder::Instruction::Drop,
                    encoder::Instruction::Push(2),
                    encoder::Instruction::MemStore(Some(3)),
                    encoder::Instruction::Drop,
                    encoder::Instruction::MemLoad(Some(2)),
                    encoder::Instruction::MemLoad(Some(3)),
                    encoder::Instruction::U32CheckedAdd,
                    encoder::Instruction::MemStore(Some(4)),
                    encoder::Instruction::Drop,
                    encoder::Instruction::MemLoad(Some(4)),
                    encoder::Instruction::MemStore(Some(1)),
                    encoder::Instruction::Drop,
                    encoder::Instruction::Abstract(encoder::AbstractInstruction::Return),
                ])
            )]
        );
    }

    #[test]
    fn test_compile_add_u64_u32() {
        let mut instructions = Vec::new();
        let mut memory = Memory::new();
        let mut compiler = Compiler::new(&mut instructions, &mut memory);

        let a = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
        let b = compiler
            .memory
            .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

        compiler.memory.write(
            &mut compiler.instructions,
            a.memory_addr,
            &[ValueSource::Immediate(1)],
        );
        compiler.memory.write(
            &mut compiler.instructions,
            b.memory_addr,
            &[ValueSource::Immediate(2)],
        );

        let result = compile_add(&mut compiler, &a, &b);

        assert_eq!(
            compiler.instructions,
            &mut vec![
                encoder::Instruction::Push(1),
                encoder::Instruction::MemStore(Some(1)),
                encoder::Instruction::Drop,
                encoder::Instruction::Push(2),
                encoder::Instruction::MemStore(Some(3)),
                encoder::Instruction::Drop,
                encoder::Instruction::MemLoad(Some(3)),
                encoder::Instruction::Push(0),
                encoder::Instruction::MemStore(Some(4)),
                encoder::Instruction::Drop,
                encoder::Instruction::MemStore(Some(5)),
                encoder::Instruction::Drop,
                encoder::Instruction::MemLoad(Some(2)),
                encoder::Instruction::MemLoad(Some(1)),
                encoder::Instruction::MemLoad(Some(5)),
                encoder::Instruction::MemLoad(Some(4)),
                encoder::Instruction::Exec("u64::checked_add"),
                encoder::Instruction::MemStore(Some(6)),
                encoder::Instruction::Drop,
                encoder::Instruction::MemStore(Some(7)),
                encoder::Instruction::Drop,
            ]
        );
    }
}
