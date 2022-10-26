mod encoder;

use crate::ast::{self, Expression, Statement};

#[derive(Copy, Clone, Debug, PartialEq)]
enum PrimitiveType {
    U32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Type {
    PrimitiveType(PrimitiveType),
}

impl Type {
    fn miden_width(&self) -> u32 {
        match self {
            Type::PrimitiveType(PrimitiveType::U32) => 1,
        }
    }
}

#[derive(Debug, Clone)]
struct Symbol {
    type_: Type,
    memory_addr: u32,
}

#[derive(Debug)]
struct Scope {
    symbols: Vec<(String, Symbol)>,
}

impl Scope {
    fn new() -> Self {
        Scope { symbols: vec![] }
    }

    fn with(&mut self, body: impl FnOnce(&mut Self)) {
        let start_len = self.symbols.len();
        body(self);
        self.symbols.truncate(start_len);
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
}

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
            static_alloc_ptr: 0,
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

struct Compiler<'ast, 'b> {
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
}

fn compile_expression(expr: &Expression, compiler: &mut Compiler, scope: &Scope) -> Symbol {
    match expr {
        Expression::Ident(id) => {
            let symbol = scope.find_symbol(id).unwrap();
            symbol.clone()
        }
        Expression::Primitive(ast::Primitive::Number(n)) => {
            let symbol = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::U32));
            compiler.memory.write(
                &mut compiler.instructions,
                symbol.memory_addr,
                &[ValueSource::Immediate(*n as u32)],
            );
            symbol
        }
        Expression::Add(a, b) => {
            let a = compile_expression(a, compiler, scope);
            compiler.memory.read(
                &mut compiler.instructions,
                a.memory_addr,
                a.type_.miden_width(),
            );

            let b = compile_expression(b, compiler, scope);
            compiler.memory.read(
                &mut compiler.instructions,
                b.memory_addr,
                b.type_.miden_width(),
            );

            let result = compiler
                .memory
                .allocate_symbol(Type::PrimitiveType(PrimitiveType::U32));
            compiler
                .instructions
                .push(encoder::Instruction::U32CheckedAdd);
            compiler.memory.write(
                &mut compiler.instructions,
                result.memory_addr,
                &[ValueSource::Stack],
            );

            result
        }
        e => unimplemented!("{:?}", e),
    }
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
            let mut values = vec![];
            for _ in 0..symbol.type_.miden_width() {
                values.push(ValueSource::Stack);
            }
            compiler.memory.write(
                &mut compiler.instructions,
                return_result.memory_addr,
                &values,
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
                Type::PrimitiveType(PrimitiveType::U32)
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
        st => unimplemented!("{:?}", st),
    }
}

fn compile_function_call(
    function: &ast::Function,
    compiler: &mut Compiler,
    args: &[Symbol],
) -> Symbol {
    let mut function_instructions = vec![];
    let mut function_compiler = Compiler::new(&mut function_instructions, compiler.memory);

    let mut function_scope = Scope::new();
    for (arg, param) in args.iter().zip(function.parameters.iter()) {
        function_scope.add_symbol(param.name.clone(), arg.clone());
    }

    let mut return_result = function_compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::U32));

    for statement in &function.statements {
        compile_statement(
            statement,
            &mut function_compiler,
            &mut function_scope,
            &mut return_result,
        );
    }

    compiler.instructions.push(encoder::Instruction::Abstract(
        encoder::AbstractInstruction::InlinedFunction(function_instructions),
    ));

    return_result
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
        compile_function_call(&main, &mut compiler, &[]);

        assert_eq!(
            compiler.instructions,
            &mut vec![encoder::Instruction::Abstract(
                encoder::AbstractInstruction::InlinedFunction(vec![
                    encoder::Instruction::Push(1),
                    encoder::Instruction::MemStore(Some(1)),
                    encoder::Instruction::Drop,
                    encoder::Instruction::MemLoad(Some(1)),
                    encoder::Instruction::Push(2),
                    encoder::Instruction::MemStore(Some(2)),
                    encoder::Instruction::Drop,
                    encoder::Instruction::MemLoad(Some(2)),
                    encoder::Instruction::U32CheckedAdd,
                    encoder::Instruction::MemStore(Some(3)),
                    encoder::Instruction::Drop,
                    encoder::Instruction::MemLoad(Some(3)),
                    encoder::Instruction::MemStore(Some(0)),
                    encoder::Instruction::Drop,
                    encoder::Instruction::Abstract(encoder::AbstractInstruction::Return),
                ])
            )]
        );
    }
}
