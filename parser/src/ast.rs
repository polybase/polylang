use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Program {
    pub nodes: Vec<RootNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RootNode {
    Collection(Collection),
    Function(Function),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    pub items: Vec<CollectionItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CollectionItem {
    Field(Field),
    Function(Function),
    Index(Index),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub type_: Type,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FieldDecorator {
    pub name: String,
    pub arguments: Vec<Primitive>,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    String,
    Number,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterType {
    String,
    Number,
    Record,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub type_: ParameterType,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub statements: Vec<Statement>,
    pub statements_code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Index {
    pub unique: bool,
    pub fields: Vec<IndexField>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexField {
    pub name: String,
    pub order: Order,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Statement {
    Break,
    If(If),
    While(While),
    Return(Expression),
    Expression(Expression),
    Throw(Expression),
    Let(String, Expression),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    Primitive(Primitive),
    Ident(String),
    Boolean(bool),
    Assign(Box<Expression>, Box<Expression>),
    AssignSub(Box<Expression>, Box<Expression>),
    AssignAdd(Box<Expression>, Box<Expression>),
    Or(Box<Expression>, Box<Expression>),
    And(Box<Expression>, Box<Expression>),
    Equal(Box<Expression>, Box<Expression>),
    NotEqual(Box<Expression>, Box<Expression>),
    LessThan(Box<Expression>, Box<Expression>),
    LessThanOrEqual(Box<Expression>, Box<Expression>),
    GreaterThan(Box<Expression>, Box<Expression>),
    GreaterThanOrEqual(Box<Expression>, Box<Expression>),
    BitOr(Box<Expression>, Box<Expression>),
    BitXor(Box<Expression>, Box<Expression>),
    BitAnd(Box<Expression>, Box<Expression>),
    ShiftLeft(Box<Expression>, Box<Expression>),
    ShiftRight(Box<Expression>, Box<Expression>),
    Add(Box<Expression>, Box<Expression>),
    Subtract(Box<Expression>, Box<Expression>),
    Multiply(Box<Expression>, Box<Expression>),
    Divide(Box<Expression>, Box<Expression>),
    Modulo(Box<Expression>, Box<Expression>),
    Exponent(Box<Expression>, Box<Expression>),
    Not(Box<Expression>),
    BitNot(Box<Expression>),
    Negate(Box<Expression>),
    Dot(Box<Expression>, String),
    Index(Box<Expression>, Box<Expression>),
    Call(Box<Expression>, Vec<Expression>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct If {
    pub condition: Expression,
    pub then_statements: Vec<Statement>,
    pub else_statements: Vec<Statement>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct While {
    pub condition: Expression,
    pub statements: Vec<Statement>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Primitive {
    Number(f64),
    String(String),
}
