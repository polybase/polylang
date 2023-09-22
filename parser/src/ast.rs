use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Program {
    pub nodes: Vec<RootNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RootNode {
    Contract(Contract),
    Function(Function),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Contract {
    pub name: String,
    pub decorators: Vec<Decorator>,
    pub items: Vec<ContractItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ContractItem {
    Field(Field),
    Function(Function),
    Index(Index),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub type_: Type,
    pub required: bool,
    pub decorators: Vec<Decorator>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecoratorNode {
    pub name: String,
    pub arguments: Vec<DecoratorArgument>,
}

pub type Decorator = MaybeSpanned<DecoratorNode>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DecoratorArgument {
    Identifier(String),
    Literal(Literal),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tag", content = "content")]
pub enum Type {
    String,
    Number,
    F32,
    F64,
    U32,
    U64,
    I32,
    I64,
    Boolean,
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Object(Vec<Field>),
    PublicKey,
    ForeignRecord { contract: String },
    Bytes,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tag", content = "content")]
pub enum ParameterType {
    String,
    Number,
    F32,
    F64,
    U32,
    U64,
    I32,
    I64,
    Boolean,
    Array(Type),
    Map(Type, Type),
    Object(Vec<(String, Type)>),
    Record,
    ForeignRecord { contract: String },
    PublicKey,
    Bytes,
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
    pub decorators: Vec<Decorator>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub statements: Vec<Statement>,
    pub statements_code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Index {
    pub fields: Vec<IndexField>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct IndexField {
    pub path: Vec<String>,
    pub order: Order,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl From<Span> for error::span::Span {
    fn from(val: Span) -> Self {
        error::span::Span::new(val.start, val.end)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Spanned<T> {
    pub span: Span,
    pub inner: T,
}

#[derive(Debug, Serialize, Deserialize, derive_more::From, Clone)]
pub enum MaybeSpanned<T> {
    T(T),
    Spanned(Spanned<T>),
}

impl<T> MaybeSpanned<T> {
    pub fn with_span(self, start: usize, end: usize) -> Self {
        let inner = match self {
            Self::T(inner) => inner,
            Self::Spanned(Spanned { inner, .. }) => inner,
        };
        Self::Spanned(Spanned {
            span: Span { start, end },
            inner,
        })
    }

    pub fn span(&self) -> Option<Span> {
        let Self::Spanned(spanned) = self else {
            return None;
        };
        Some(spanned.span)
    }

    pub fn into_inner(self) -> T {
        match self {
            Self::T(t) => t,
            Self::Spanned(spanned) => spanned.inner,
        }
    }
}

impl<T: PartialEq> PartialEq for MaybeSpanned<T> {
    fn eq(&self, other: &Self) -> bool {
        <T as PartialEq>::eq(&**self, &**other)
    }
}

impl<T> std::ops::Deref for MaybeSpanned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            MaybeSpanned::T(inner) => inner,
            MaybeSpanned::Spanned(s) => &s.inner,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StatementKind {
    Break,
    If(If),
    While(While),
    For(For),
    Return(Expression),
    Expression(Expression),
    Throw(Expression),
    Let(Let),
}

pub type Expression = MaybeSpanned<ExpressionKind>;
pub type Statement = MaybeSpanned<StatementKind>;

pub trait WithSpan: Sized {
    fn with_span(self, start: usize, end: usize) -> MaybeSpanned<Self> {
        MaybeSpanned::Spanned(Spanned {
            span: Span { start, end },
            inner: self,
        })
    }
}
impl<T> WithSpan for T {}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum ExpressionKind {
    Primitive(Primitive),
    Ident(String),
    Boolean(bool),
    Object(Object),
    Array(Vec<Expression>),
    Assign(Box<Expression>, Box<Expression>),
    AssignSub(Box<Expression>, Box<Expression>),
    AssignAdd(Box<Expression>, Box<Expression>),
    Increment(Box<Expression>),
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
pub struct Let {
    pub identifier: String,
    pub type_: Option<Type>,
    pub expression: Expression,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct For {
    pub for_kind: ForKind,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ForKind {
    Basic {
        initial_statement: ForInitialStatement,
        condition: Expression,
        post_statement: Expression,
    },
    ForEach {
        for_each_type: ForEachType,
        identifier: String,
        iterable: Expression,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ForInitialStatement {
    Let(Let),
    Expression(Expression),
}

#[derive(Debug, Serialize, Deserialize, derive_more::Display)]
pub enum ForEachType {
    // for .. in ..
    #[display(fmt = "in")]
    In,
    // for .. of ..
    #[display(fmt = "of")]
    Of,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum Primitive {
    // (value, has_decimal_point)
    Number(f64, bool),
    String(String),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Object {
    pub fields: Vec<(String, Expression)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Eth(Vec<u8>),
}
