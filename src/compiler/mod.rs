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

lazy_static::lazy_static! {
    // TODO: rewrite this in raw instructions for better performance
    static ref READ_ADVICE_INTO_STRING: ast::Function = crate::polylang::FunctionParser::new().parse(r#"
        function readAdviceIntoString(length: number, dataPtr: number): number {
            if (length == 0) return 0;
            let i = 0;
            let y = length - 1;
            while (y >= i) {
                writeMemory(dataPtr + i, readAdvice());
                i = i + 1;
            }

            return length;
        }
    "#).unwrap();
    static ref READ_ADVICE_STRING: ast::Function = crate::polylang::FunctionParser::new().parse(r#"
        function readAdviceString(): string {
            let length = readAdvice();
            let dataPtr = dynamicAlloc(length);
            readAdviceIntoString(length, dataPtr);
            return unsafeToString(length, dataPtr);
        }
    "#).unwrap();
    static ref UINT32_TO_STRING: ast::Function = crate::polylang::FunctionParser::new().parse(r#"
        function uint32ToString(value: number): string {
            if (value == 0) return '0';

            let length = 0;
            let i = 0;
            i = value;
            while (i >= 1) {
                i = i / 10;
                length = length + 1;
            }

            let dataPtr = dynamicAlloc(length); 

            let offset = 0;
            offset = length;
            while (value >= 1) {
                offset = offset - 1;
                let digit = value % 10;
                value = value / 10;
                writeMemory(dataPtr + offset, digit + 48);
            }

            return unsafeToString(length, dataPtr);
        }
    "#).unwrap();
    static ref BUILTINS_SCOPE: &'static Scope<'static, 'static> = {
        let mut scope = Scope::new();

        for function in HIDDEN_BUILTINS.iter() {
            scope.add_function(function.0.clone(), function.1.clone());
        }

        for function in USABLE_BUILTINS.iter() {
            scope.add_function(function.0.clone(), function.1.clone());
        }

        Box::leak(Box::new(scope))
    };
    static ref HIDDEN_BUILTINS: &'static [(String, Function<'static>)] = {
        let mut builtins = Vec::new();

        builtins.push((
            "dynamicAlloc".to_string(),
            Function::Builtin(Box::new(&dynamic_alloc)),
        ));

        builtins.push((
            "writeMemory".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let address = args.get(0).unwrap();
                let value = args.get(1).unwrap();

                assert_eq!(address.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                assert_eq!(value.type_, Type::PrimitiveType(PrimitiveType::UInt32));

                compiler.memory.read(
                    &mut compiler.instructions,
                    value.memory_addr,
                    value.type_.miden_width(),
                );
                compiler.memory.read(
                    &mut compiler.instructions,
                    address.memory_addr,
                    address.type_.miden_width(),
                );
                compiler
                    .instructions
                    .push(encoder::Instruction::MemStore(None));
                compiler
                    .instructions
                    .push(encoder::Instruction::Drop);

                Symbol {
                    type_: Type::PrimitiveType(PrimitiveType::UInt32),
                    memory_addr: 0,
                }
            })),
        ));

        builtins.push((
            "readAdvice".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let args: &[Symbol] = &[];
                let symbol = compiler
                    .memory
                    .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

                compiler.instructions.push(encoder::Instruction::AdvPush(1));
                compiler.memory.write(
                    &mut compiler.instructions,
                    symbol.memory_addr,
                    &vec![ValueSource::Stack],
                );

                symbol
            })),
        ));

        builtins.push((
            "readAdviceIntoString".to_string(),
            Function::AST(&READ_ADVICE_INTO_STRING),
        ));


        builtins.push((
            "unsafeToString".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let length = args.get(0).unwrap();
                let address_ptr = args.get(1).unwrap();

                assert_eq!(length.type_, Type::PrimitiveType(PrimitiveType::UInt32));
                assert_eq!(address_ptr.type_, Type::PrimitiveType(PrimitiveType::UInt32));

                let s = string::new(compiler, "");

                compiler.memory.read(
                    &mut compiler.instructions,
                    length.memory_addr,
                    length.type_.miden_width(),
                );
                compiler.memory.write(
                    &mut compiler.instructions,
                    string::length(&s).memory_addr,
                    &vec![ValueSource::Stack; length.type_.miden_width() as _],
                );

                compiler.memory.read(
                    &mut compiler.instructions,
                    address_ptr.memory_addr,
                    address_ptr.type_.miden_width(),
                );
                compiler.memory.write(
                    &mut compiler.instructions,
                    string::data_ptr(&s).memory_addr,
                    &vec![ValueSource::Stack; address_ptr.type_.miden_width() as _],
                );

                s
            })),
        ));

        Box::leak(Box::new(builtins))
    };
    static ref USABLE_BUILTINS: &'static [(String, Function<'static>)] = {
        let mut builtins = Vec::new();

        builtins.push((
            "assert".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let condition = args.get(0).unwrap();
                let message = args.get(1).unwrap();

                assert_eq!(condition.type_, Type::PrimitiveType(PrimitiveType::Boolean));
                assert_eq!(message.type_, Type::String);

                let mut failure_branch = vec![];
                let mut failure_compiler = Compiler::new(&mut failure_branch, compiler.memory, compiler.root_scope);

                let str_len = string::length(message);
                let str_data_ptr = string::data_ptr(message);

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
        ));

        builtins.push((
            "readAdviceString".to_string(),
            Function::Builtin(Box::new(&|compiler, _, args| {
                let old_root_scope = compiler.root_scope;
                compiler.root_scope = &BUILTINS_SCOPE;
                let result = compile_ast_function_call(&READ_ADVICE_STRING, compiler, args, None);
                compiler.root_scope = old_root_scope;
                result
            })),
        ));

        builtins.push(("readAdviceUInt32".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a u32
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));
            compiler.memory.write(&mut compiler.instructions, symbol.memory_addr, &vec![ValueSource::Stack]);
            symbol
        }))));

        builtins.push(("readAdviceBoolean".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            assert_eq!(args.len(), 0);

            compiler.instructions.push(encoder::Instruction::AdvPush(1));
            // TODO: assert that the number is actually a boolean
            let symbol = compiler.memory.allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));
            compiler.memory.write(&mut compiler.instructions, symbol.memory_addr, &vec![ValueSource::Stack]);
            symbol
        }))));

        builtins.push(("uint32ToString".to_string(), Function::Builtin(Box::new(&|compiler, _, args| {
            let old_root_scope = compiler.root_scope;
            compiler.root_scope = &BUILTINS_SCOPE;
            let result = compile_ast_function_call(&UINT32_TO_STRING, compiler, args, None);
            compiler.root_scope = old_root_scope;
            result
        }))));

        Box::leak(Box::new(builtins))
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
    fields: Vec<(String, Type)>,
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
                type_: field_type.clone(),
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
    Builtin(Box<&'static (dyn Fn(&mut Compiler, &mut Scope, &[Symbol]) -> Symbol + Sync)>),
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
struct Scope<'ast, 'b> {
    parent: Option<&'b Scope<'ast, 'b>>,
    symbols: Vec<(String, Symbol)>,
    functions: Vec<(String, Function<'ast>)>,
    contracts: Vec<(String, Contract<'ast>)>,
}

impl<'ast> Scope<'ast, '_> {
    fn new() -> Self {
        Scope {
            parent: None,
            symbols: vec![],
            functions: vec![],
            contracts: vec![],
        }
    }

    fn deeper<'b>(&'b self) -> Scope<'ast, 'b> {
        let scope = Scope {
            parent: Some(self),
            symbols: vec![],
            functions: vec![],
            contracts: vec![],
        };

        scope
    }

    fn add_symbol(&mut self, name: String, symbol: Symbol) {
        self.symbols.push((name, symbol));
    }

    fn find_symbol(&self, name: &str) -> Option<&Symbol> {
        if let Some(symbol) = self
            .symbols
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s)
        {
            return Some(symbol);
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.find_symbol(name);
        }

        None
    }

    fn add_function(&mut self, name: String, function: Function<'ast>) {
        self.functions.push((name, function));
    }

    fn find_function(&self, name: &str) -> Option<&Function<'ast>> {
        if let Some(func) = self
            .functions
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f)
        {
            return Some(func);
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.find_function(name);
        }

        None
    }

    fn add_contract(&mut self, name: String, contract: Contract<'ast>) {
        if self.find_contract(&name).is_some() {
            panic!("Contract {} already exists", name);
        }

        self.contracts.push((name, contract));
    }

    fn find_contract(&self, name: &str) -> Option<&Contract<'ast>> {
        if let Some(contract) = self
            .contracts
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c)
        {
            return Some(contract);
        }

        self.parent.and_then(|p| p.find_contract(name))
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
            // 3 is reserved for the dynamic allocation pointer
            static_alloc_ptr: 4,
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

    /// write(vec![], addr, &[ValueSource::Immediate(0), ValueSource::Immediate(1)])
    /// will set addr to 0 and addr + 1 to 1
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

    /// read reads the values from the memory starting at start_addr and pushes them to the stack.
    ///
    /// The top most stack item will be the value of start_addr.
    ///
    /// The bottom most stack item will be the value of start_addr + count - 1.
    fn read(&self, instructions: &mut Vec<encoder::Instruction>, start_addr: u32, count: u32) {
        for i in 1..=count {
            ValueSource::Memory(start_addr + count - i).load(instructions);
        }
    }
}

pub(crate) struct Compiler<'ast, 'c, 's> {
    instructions: &'c mut Vec<encoder::Instruction<'ast>>,
    memory: &'c mut Memory,
    root_scope: &'c Scope<'ast, 's>,
}

impl<'ast, 'c, 's> Compiler<'ast, 'c, 's> {
    fn new(
        instructions: &'c mut Vec<encoder::Instruction<'ast>>,
        memory: &'c mut Memory,
        root_scope: &'c Scope<'ast, 's>,
    ) -> Self {
        Compiler {
            instructions,
            memory,
            root_scope,
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
        Expression::Modulo(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_mod(compiler, &a, &b)
        }
        Expression::Divide(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_div(compiler, &a, &b)
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

            compile_function_call(compiler, func, &args_symbols, None)
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
        Expression::LessThanOrEqual(a, b) => {
            let a = compile_expression(a, compiler, scope);
            let b = compile_expression(b, compiler, scope);

            compile_lte(compiler, &a, &b)
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
            let mut scope = scope.deeper();
            let mut condition_instructions = vec![];
            let mut condition_compiler = Compiler::new(
                &mut condition_instructions,
                compiler.memory,
                compiler.root_scope,
            );
            let condition_symbol = compile_expression(condition, &mut condition_compiler, &scope);
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
            let mut body_compiler =
                Compiler::new(&mut body_instructions, compiler.memory, compiler.root_scope);
            for statement in then_statements {
                compile_statement(statement, &mut body_compiler, &mut scope, return_result);
            }

            let mut else_body_instructions = vec![];
            let mut else_body_compiler = Compiler::new(
                &mut else_body_instructions,
                compiler.memory,
                compiler.root_scope,
            );
            for statement in else_statements {
                compile_statement(
                    statement,
                    &mut else_body_compiler,
                    &mut scope,
                    return_result,
                );
            }

            compiler.instructions.push(encoder::Instruction::If {
                condition: condition_instructions,
                then: body_instructions,
                else_: else_body_instructions,
            })
        }
        Statement::While(ast::While {
            condition,
            statements,
        }) => {
            let mut scope = scope.deeper();
            let mut condition_instructions = vec![];
            let mut condition_compiler = Compiler::new(
                &mut condition_instructions,
                compiler.memory,
                compiler.root_scope,
            );
            let condition_symbol = compile_expression(condition, &mut condition_compiler, &scope);
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
            let mut body_compiler =
                Compiler::new(&mut body_instructions, compiler.memory, compiler.root_scope);
            for statement in statements {
                compile_statement(statement, &mut body_compiler, &mut scope, return_result);
            }

            compiler.instructions.push(encoder::Instruction::While {
                condition: condition_instructions,
                body: body_instructions,
            })
        }
        Statement::Let(name, expr) => {
            let symbol = compile_expression(expr, compiler, scope);
            scope.add_symbol(name.to_string(), symbol);
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
    args: &[Symbol],
    this: Option<Symbol>,
) -> Symbol {
    let mut function_instructions = vec![];
    let mut function_compiler = Compiler::new(
        &mut function_instructions,
        compiler.memory,
        compiler.root_scope,
    );

    let scope = &mut Scope::new();
    scope.parent = Some(compiler.root_scope);

    if let Some(this) = this {
        scope.add_symbol("this".to_string(), this);
    }

    let mut return_result = function_compiler
        .memory
        .allocate_symbol(match function.return_type {
            None => Type::PrimitiveType(PrimitiveType::Boolean),
            Some(ast::Type::Number) => Type::PrimitiveType(PrimitiveType::UInt32),
            Some(ast::Type::String) => Type::String,
        });
    for (arg, param) in args.iter().zip(function.parameters.iter()) {
        scope.add_symbol(param.name.clone(), arg.clone());
    }

    for statement in &function.statements {
        compile_statement(statement, &mut function_compiler, scope, &mut return_result);
    }

    compiler.instructions.push(encoder::Instruction::Abstract(
        encoder::AbstractInstruction::InlinedFunction(function_instructions),
    ));

    return_result
}

fn compile_function_call(
    compiler: &mut Compiler,
    function: &Function,
    args: &[Symbol],
    this: Option<Symbol>,
) -> Symbol {
    match function {
        Function::AST(a) => compile_ast_function_call(a, compiler, args, this),
        Function::Builtin(b) => b(compiler, &mut Scope::new(), args),
    }
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

fn compile_mod(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::modulo(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::modulo(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::modulo(compiler, a, &b_u64)
        }
        e => unimplemented!("{:?}", e),
    }
}

fn compile_div(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::div(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::div(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::div(compiler, a, &b_u64)
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

fn compile_lte(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    match (&a.type_, &b.type_) {
        (
            Type::PrimitiveType(PrimitiveType::UInt32),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => uint32::lte(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt64),
        ) => uint64::lte(compiler, a, b),
        (
            Type::PrimitiveType(PrimitiveType::UInt64),
            Type::PrimitiveType(PrimitiveType::UInt32),
        ) => {
            let b_u64 = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt64));
            cast(compiler, b, &b_u64);

            uint64::lte(compiler, a, &b_u64)
        }
        e => unimplemented!("{:?}", e),
    }
}

fn dynamic_alloc(compiler: &mut Compiler, scope: &mut Scope, args: &[Symbol]) -> Symbol {
    let size = &args[0];
    assert_eq!(size.type_, Type::PrimitiveType(PrimitiveType::UInt32));

    let addr = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::UInt32));

    compiler
        .instructions
        .push(encoder::Instruction::MemLoad(Some(3)));
    compiler.instructions.push(encoder::Instruction::Dup);
    compiler.memory.write(
        &mut compiler.instructions,
        addr.memory_addr,
        &vec![ValueSource::Stack],
    );
    compiler.memory.read(
        &mut compiler.instructions,
        size.memory_addr,
        size.type_.miden_width(),
    );
    // old addr + size
    compiler
        .instructions
        .push(encoder::Instruction::U32CheckedAdd);

    // store new addr
    compiler
        .instructions
        .push(encoder::Instruction::MemStore(Some(3)));

    // return old addr
    addr
}

fn read_struct_from_advice_tape(
    compiler: &mut Compiler,
    struct_symbol: &Symbol,
    struct_type: &Struct,
) {
    for (name, type_) in &struct_type.fields {
        let symbol = match type_ {
            Type::PrimitiveType(PrimitiveType::Boolean) => compile_function_call(
                compiler,
                BUILTINS_SCOPE.find_function("readAdviceBoolean").unwrap(),
                &[],
                None,
            ),
            Type::PrimitiveType(PrimitiveType::UInt32) => compile_function_call(
                compiler,
                BUILTINS_SCOPE.find_function("readAdviceUInt32").unwrap(),
                &[],
                None,
            ),
            Type::PrimitiveType(PrimitiveType::UInt64) => todo!(),
            Type::String => compile_function_call(
                compiler,
                BUILTINS_SCOPE.find_function("readAdviceString").unwrap(),
                &[],
                None,
            ),
            _ => unimplemented!("{:?}", type_),
        };

        let sf = struct_field(struct_symbol, name).unwrap();
        compiler.memory.read(
            &mut compiler.instructions,
            symbol.memory_addr,
            symbol.type_.miden_width(),
        );
        compiler.memory.write(
            &mut compiler.instructions,
            sf.memory_addr,
            &vec![ValueSource::Stack; symbol.type_.miden_width() as _],
        );
    }
}

fn read_contract_inputs(
    compiler: &mut Compiler,
    this_struct: Struct,
    args: &[Type],
) -> (Symbol, Vec<Symbol>) {
    let this = compiler.memory.allocate_symbol(Type::Struct(this_struct));
    let this_struct = if let Type::Struct(s) = &this.type_ {
        s
    } else {
        unreachable!();
    };

    read_struct_from_advice_tape(compiler, &this, this_struct);

    let mut args_symbols = Vec::new();
    for arg in args {
        let symbol = match arg {
            Type::PrimitiveType(PrimitiveType::Boolean) => compile_function_call(
                compiler,
                BUILTINS_SCOPE.find_function("readAdviceBoolean").unwrap(),
                &[],
                None,
            ),
            Type::PrimitiveType(PrimitiveType::UInt32) => compile_function_call(
                compiler,
                BUILTINS_SCOPE.find_function("readAdviceUInt32").unwrap(),
                &[],
                None,
            ),
            Type::PrimitiveType(PrimitiveType::UInt64) => todo!(),
            Type::Struct(struct_) => {
                let symbol = compiler.memory.allocate_symbol(arg.clone());
                read_struct_from_advice_tape(compiler, &symbol, struct_);
                symbol
            }
            _ => unimplemented!(),
        };

        args_symbols.push(symbol);
    }

    (this, args_symbols)
}

fn prepare_scope(program: &ast::Program) -> Scope {
    let mut scope = Scope::new();

    for func in USABLE_BUILTINS.iter() {
        scope.add_function(func.0.clone(), func.1.clone());
    }

    for node in &program.nodes {
        match node {
            ast::RootNode::Collection(c) => {
                let mut contract = Contract {
                    name: c.name.clone(),
                    functions: vec![],
                    fields: vec![],
                };

                for item in &c.items {
                    match item {
                        ast::CollectionItem::Field(f) => {
                            contract.fields.push((
                                f.name.clone(),
                                match f.type_ {
                                    ast::Type::String => Type::String,
                                    ast::Type::Number => Type::PrimitiveType(PrimitiveType::UInt32),
                                },
                            ));
                        }
                        ast::CollectionItem::Function(f) => {
                            contract.functions.push((f.name.clone(), &f));
                        }
                        ast::CollectionItem::Index(_) => todo!(),
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

pub fn compile(program: ast::Program, contract_name: Option<&str>, function_name: &str) -> String {
    let scope = prepare_scope(&program);
    let contract = contract_name.map(|name| scope.find_contract(name).unwrap());
    let contract_struct = contract.map(|contract| Struct {
        name: contract.name.clone(),
        fields: contract
            .fields
            .iter()
            .map(|(name, field)| (name.clone(), field.clone()))
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
        let mut compiler = Compiler::new(&mut instructions, &mut memory, &scope);

        let (this_symbol, arg_symbols) = read_contract_inputs(
            &mut compiler,
            contract_struct.clone().unwrap_or(Struct {
                name: "empty".to_string(),
                fields: vec![],
            }),
            &function
                .parameters
                .iter()
                .map(|p| match p.type_ {
                    ast::ParameterType::String => todo!(),
                    ast::ParameterType::Number => Type::PrimitiveType(PrimitiveType::UInt32),
                    ast::ParameterType::Record => Type::Struct(contract_struct.clone().unwrap()),
                })
                .collect::<Vec<_>>(),
        );

        let result =
            compile_ast_function_call(function, &mut compiler, &arg_symbols, Some(this_symbol));
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
    miden_code.push_str("  push.");
    miden_code.push_str(&memory.static_alloc_ptr.to_string());
    miden_code.push_str("\n  mem_store.3\n  drop\n"); // dynamic allocation pointer
    for instruction in instructions {
        instruction
            .encode(unsafe { miden_code.as_mut_vec() }, 1)
            .unwrap();
        miden_code.push_str("\n");
    }
    miden_code.push_str("end\n");

    miden_code
}
