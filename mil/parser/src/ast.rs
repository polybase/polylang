pub struct Node<T> {
    pub node: T,
    pub span: std::ops::Range<usize>,
}

impl<T> Node<T> {
    pub fn new(node: T, span: std::ops::Range<usize>) -> Self {
        Self { node, span }
    }
}

pub struct Identifier(pub String);

pub struct FunctionCall {
    pub name: Identifier,
    pub args: Vec<Node<Expression>>,
}

pub enum Expression {
    Number(u64),
    Identifier(Identifier),
    FunctionCall(FunctionCall),
}

pub struct Binding {
    pub names: Vec<Identifier>,
    pub expr: Node<Expression>,
}

pub struct If {
    pub condition: Node<Expression>,
    pub then: Vec<Statement>,
    pub otherwise: Vec<Statement>,
}

pub enum Statement {
    Binding(Binding),
    FunctionCall(Node<FunctionCall>),
    If(If),
}

pub struct Function {
    pub name: Identifier,
    pub args: Vec<Identifier>,
    pub outputs: Vec<Identifier>,
    pub body: Vec<Statement>,
}

pub struct Program {
    pub functions: Vec<Function>,
}
