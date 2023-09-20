// If you're adding new fields to any struct,
// make sure to use #[serde(default)] on the field,
// so that it doesn't break the deserialization (missing field errors).

use std::{borrow::Cow, fmt::Display};

use polylang_parser::ast;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Root<'a>(#[serde(borrow)] pub Vec<RootNode<'a>>);

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum RootNode<'a> {
    #[serde(borrow, rename = "collection")]
    Collection(Collection<'a>),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Collection<'a> {
    pub namespace: Namespace<'a>,
    pub name: Cow<'a, str>,
    #[serde(borrow)]
    pub attributes: Vec<CollectionAttribute<'a>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename = "namespace")]
pub struct Namespace<'a> {
    pub value: Cow<'a, str>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum CollectionAttribute<'a> {
    #[serde(borrow, rename = "property")]
    Property(Property<'a>),
    #[serde(borrow, rename = "method")]
    Method(Method<'a>),
    #[serde(borrow, rename = "index")]
    Index(Index<'a>),
    #[serde(borrow, rename = "directive")]
    Directive(Directive<'a>),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Property<'a> {
    pub name: Cow<'a, str>,
    #[serde(rename = "type", borrow)]
    pub type_: Type<'a>,
    pub directives: Vec<Directive<'a>>,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Method<'a> {
    pub name: Cow<'a, str>,
    #[serde(borrow)]
    pub attributes: Vec<MethodAttribute<'a>>,
    pub code: Cow<'a, str>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Index<'a> {
    #[serde(rename = "fields", borrow)]
    pub fields: Vec<IndexField<'a>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct IndexField<'a> {
    pub direction: Direction,
    #[serde(rename = "fieldPath", borrow)]
    pub field_path: Vec<Cow<'a, str>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum Direction {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MethodAttribute<'a> {
    #[serde(borrow, rename = "directive")]
    Directive(Directive<'a>),
    #[serde(borrow, rename = "parameter")]
    Parameter(Parameter<'a>),
    #[serde(borrow, rename = "returnvalue")]
    ReturnValue(ReturnValue<'a>),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Directive<'a> {
    pub name: Cow<'a, str>,
    pub arguments: Vec<DirectiveArgument<'a>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum DirectiveArgument<'a> {
    #[serde(rename = "fieldreference")]
    FieldReference(FieldReference<'a>),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FieldReference<'a> {
    pub path: Vec<Cow<'a, str>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Parameter<'a> {
    pub name: Cow<'a, str>,
    #[serde(rename = "type", borrow)]
    pub type_: Type<'a>,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ReturnValue<'a> {
    pub name: Cow<'a, str>,
    #[serde(rename = "type", borrow)]
    pub type_: Type<'a>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "kind")]
pub enum Type<'a> {
    #[serde(rename = "primitive")]
    Primitive(Primitive),
    #[serde(borrow, rename = "array")]
    Array(Array<'a>),
    #[serde(borrow, rename = "map")]
    Map(Map<'a>),
    #[serde(borrow, rename = "object")]
    Object(Object<'a>),
    #[serde(rename = "record")]
    Record(Record),
    #[serde(borrow, rename = "foreignrecord")]
    ForeignRecord(ForeignRecord<'a>),
    #[serde(rename = "publickey")]
    PublicKey(PublicKey),
    #[serde(other)]
    Unknown,
}

impl Display for Type<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Primitive(p) => write!(f, "{}", p.value),
            Type::Array(a) => write!(f, "{}[]", a.value),
            Type::Map(m) => write!(f, "map<{}, {}>", m.key, m.value),
            Type::Object(o) => {
                write!(f, "{{ ")?;
                for (_i, field) in o.fields.iter().enumerate() {
                    write!(
                        f,
                        "{}{}: {}",
                        field.name,
                        if field.required { "" } else { "?" },
                        field.type_
                    )?;
                    write!(f, ";")?;
                    write!(f, " ")?;
                }
                write!(f, "}}")
            }
            Type::Record(_) => write!(f, "record"),
            Type::ForeignRecord(fr) => write!(f, "{}", fr.collection),
            Type::PublicKey(_) => write!(f, "PublicKey"),
            Type::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Primitive {
    pub value: PrimitiveType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum PrimitiveType {
    #[serde(rename = "string")]
    String,
    #[serde(rename = "number")]
    Number,
    #[serde(rename = "f32")]
    F32,
    #[serde(rename = "f64")]
    F64,
    #[serde(rename = "u32")]
    U32,
    #[serde(rename = "u64")]
    U64,
    #[serde(rename = "i32")]
    I32,
    #[serde(rename = "i64")]
    I64,
    #[serde(rename = "boolean")]
    Boolean,
    #[serde(rename = "bytes")]
    Bytes,
}

impl Display for PrimitiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimitiveType::String => write!(f, "string"),
            PrimitiveType::Number => write!(f, "number"),
            PrimitiveType::F32 => write!(f, "f32"),
            PrimitiveType::F64 => write!(f, "f64"),
            PrimitiveType::U32 => write!(f, "u32"),
            PrimitiveType::U64 => write!(f, "u64"),
            PrimitiveType::I32 => write!(f, "i32"),
            PrimitiveType::I64 => write!(f, "i64"),
            PrimitiveType::Boolean => write!(f, "boolean"),
            PrimitiveType::Bytes => write!(f, "bytes"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Array<'a> {
    #[serde(borrow)]
    pub value: Box<Type<'a>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Map<'a> {
    #[serde(borrow)]
    pub key: Box<Type<'a>>,
    #[serde(borrow)]
    pub value: Box<Type<'a>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Object<'a> {
    #[serde(borrow)]
    pub fields: Vec<ObjectField<'a>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ObjectField<'a> {
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    #[serde(rename = "type")]
    pub type_: Type<'a>,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Record {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct PublicKey {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ForeignRecord<'a> {
    pub collection: Cow<'a, str>,
}

impl<'a> Root<'a> {
    pub fn from_ast(namespace: &'a str, program: &'a ast::Program) -> Result<Self, String> {
        let mut root = Root(vec![]);
        for node in &program.nodes {
            root.0.push(match node {
                ast::RootNode::Contract(c) => RootNode::Collection(Collection {
                    namespace: Namespace {
                        value: Cow::Borrowed(namespace),
                    },
                    name: Cow::Borrowed(&c.name),
                    attributes: c
                        .items
                        .iter()
                        .map(|item| match item {
                            ast::ContractItem::Field(f) => {
                                CollectionAttribute::Property(Property {
                                    name: Cow::Borrowed(&f.name),
                                    type_: Type::from_ast_type(&f.type_),
                                    directives: f
                                        .decorators
                                        .iter()
                                        .map(Directive::from_decorator_ast)
                                        .collect(),
                                    required: f.required,
                                })
                            }
                            ast::ContractItem::Function(f) => CollectionAttribute::Method(Method {
                                name: Cow::Borrowed(&f.name),
                                code: Cow::Borrowed(&f.statements_code),
                                attributes: {
                                    let mut attributes = vec![];

                                    for param in &f.parameters {
                                        attributes.push(MethodAttribute::Parameter(Parameter {
                                            name: Cow::Borrowed(&param.name),
                                            type_: Type::from_ast_parameter_type(&param.type_),
                                            required: param.required,
                                        }));
                                    }

                                    if let Some(return_type) = &f.return_type {
                                        attributes.push(MethodAttribute::ReturnValue(
                                            ReturnValue {
                                                name: Cow::Borrowed("_"),
                                                type_: Type::from_ast_type(return_type),
                                            },
                                        ));
                                    }

                                    for decorator in &f.decorators {
                                        attributes.push(MethodAttribute::Directive(
                                            Directive::from_decorator_ast(decorator),
                                        ));
                                    }

                                    attributes
                                },
                            }),
                            ast::ContractItem::Index(i) => CollectionAttribute::Index(Index {
                                fields: i
                                    .fields
                                    .iter()
                                    .map(|f| IndexField {
                                        direction: match f.order {
                                            ast::Order::Asc => Direction::Asc,
                                            ast::Order::Desc => Direction::Desc,
                                        },
                                        field_path: f
                                            .path
                                            .iter()
                                            .map(|s| Cow::Borrowed(s.as_str()))
                                            .collect(),
                                    })
                                    .collect(),
                            }),
                        })
                        .chain(c.decorators.iter().map(|d| {
                            CollectionAttribute::Directive(Directive::from_decorator_ast(d))
                        }))
                        .collect(),
                }),
                ast::RootNode::Function(_) => Err("Functions are not supported at the root level")?,
            });
        }

        Ok(root)
    }
}

impl<'a> Type<'a> {
    fn from_ast_type(type_: &'a ast::Type) -> Self {
        match type_ {
            ast::Type::String => Type::Primitive(Primitive {
                value: PrimitiveType::String,
            }),
            ast::Type::Number => Type::Primitive(Primitive {
                value: PrimitiveType::Number,
            }),
            ast::Type::F32 => Type::Primitive(Primitive {
                value: PrimitiveType::F32,
            }),
            ast::Type::F64 => Type::Primitive(Primitive {
                value: PrimitiveType::F64,
            }),
            ast::Type::U32 => Type::Primitive(Primitive {
                value: PrimitiveType::U32,
            }),
            ast::Type::U64 => Type::Primitive(Primitive {
                value: PrimitiveType::U64,
            }),
            ast::Type::I32 => Type::Primitive(Primitive {
                value: PrimitiveType::I32,
            }),
            ast::Type::I64 => Type::Primitive(Primitive {
                value: PrimitiveType::I64,
            }),
            ast::Type::Boolean => Type::Primitive(Primitive {
                value: PrimitiveType::Boolean,
            }),
            ast::Type::Array(a) => Type::Array(Array {
                value: Box::new(Type::from_ast_type(a.as_ref())),
            }),
            ast::Type::Map(kt, vt) => Type::Map(Map {
                key: Box::new(Type::from_ast_type(kt)),
                value: Box::new(Type::from_ast_type(vt)),
            }),
            ast::Type::Object(fields) => Type::Object(Object {
                fields: fields
                    .iter()
                    .map(|f| ObjectField {
                        name: Cow::Borrowed(&f.name),
                        type_: Type::from_ast_type(&f.type_),
                        required: f.required,
                    })
                    .collect(),
            }),
            ast::Type::ForeignRecord { contract } => Type::ForeignRecord(ForeignRecord {
                collection: Cow::Borrowed(contract),
            }),
            ast::Type::PublicKey => Type::PublicKey(PublicKey {}),
            ast::Type::Bytes => Type::Primitive(Primitive {
                value: PrimitiveType::Bytes,
            }),
        }
    }

    fn from_ast_parameter_type(type_: &'a ast::ParameterType) -> Self {
        match type_ {
            ast::ParameterType::String => Type::Primitive(Primitive {
                value: PrimitiveType::String,
            }),
            ast::ParameterType::Number => Type::Primitive(Primitive {
                value: PrimitiveType::Number,
            }),
            ast::ParameterType::F32 => Type::Primitive(Primitive {
                value: PrimitiveType::F32,
            }),
            ast::ParameterType::F64 => Type::Primitive(Primitive {
                value: PrimitiveType::F64,
            }),
            ast::ParameterType::U32 => Type::Primitive(Primitive {
                value: PrimitiveType::U32,
            }),
            ast::ParameterType::U64 => Type::Primitive(Primitive {
                value: PrimitiveType::U64,
            }),
            ast::ParameterType::I32 => Type::Primitive(Primitive {
                value: PrimitiveType::I32,
            }),
            ast::ParameterType::I64 => Type::Primitive(Primitive {
                value: PrimitiveType::I64,
            }),
            ast::ParameterType::Boolean => Type::Primitive(Primitive {
                value: PrimitiveType::Boolean,
            }),
            ast::ParameterType::Array(a) => Type::Array(Array {
                value: Box::new(Type::from_ast_type(a)),
            }),
            ast::ParameterType::Map(kt, vt) => Type::Map(Map {
                key: Box::new(Type::from_ast_type(kt)),
                value: Box::new(Type::from_ast_type(vt)),
            }),
            ast::ParameterType::Object(fields) => Type::Object(Object {
                fields: fields
                    .iter()
                    .map(|f| ObjectField {
                        name: Cow::Borrowed(&f.0),
                        type_: Type::from_ast_type(&f.1),
                        required: true,
                    })
                    .collect(),
            }),
            ast::ParameterType::Record => Type::Record(Record {}),
            ast::ParameterType::ForeignRecord { contract } => Type::ForeignRecord(ForeignRecord {
                collection: Cow::Borrowed(contract.as_str()),
            }),
            ast::ParameterType::PublicKey => Type::PublicKey(PublicKey {}),
            ast::ParameterType::Bytes => Type::Primitive(Primitive {
                value: PrimitiveType::Bytes,
            }),
        }
    }
}

impl<'a> Directive<'a> {
    fn from_decorator_ast(ast: &'a ast::Decorator) -> Self {
        Directive {
            name: Cow::Borrowed(&ast.name),
            arguments: ast
                .arguments
                .iter()
                .map(DirectiveArgument::from_decorator_argument_ast)
                .collect(),
        }
    }
}

impl<'a> DirectiveArgument<'a> {
    fn from_decorator_argument_ast(ast: &'a ast::DecoratorArgument) -> Self {
        match ast {
            ast::DecoratorArgument::Identifier(f) => {
                DirectiveArgument::FieldReference(FieldReference {
                    path: f.split('.').map(Cow::Borrowed).collect(),
                })
            }
            ast::DecoratorArgument::Literal(_) => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn test() {
        let root = Root(vec![RootNode::Collection(Collection {
            namespace: Namespace {
                value: "foo".into(),
            },
            name: "Account".into(),
            attributes: vec![],
        })]);
        let json = serde_json::to_string(&root).unwrap();
        let _: Root = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_serialize() {
        let root = Root(vec![RootNode::Collection(Collection {
            namespace: Namespace {
                value: "abc/xyz".into(),
            },
            name: "Account".into(),
            attributes: vec![CollectionAttribute::Property(Property {
                name: "id".into(),
                type_: Type::Primitive(Primitive {
                    value: PrimitiveType::String,
                }),
                directives: vec![],
                required: true,
            })],
        })]);

        let actual = serde_json::to_string_pretty(&root).unwrap();
        let expected = expect![[r#"
            [
              {
                "kind": "collection",
                "namespace": {
                  "kind": "namespace",
                  "value": "abc/xyz"
                },
                "name": "Account",
                "attributes": [
                  {
                    "kind": "property",
                    "name": "id",
                    "type": {
                      "kind": "primitive",
                      "value": "string"
                    },
                    "directives": [],
                    "required": true
                  }
                ]
              }
            ]"#]];
        expected.assert_eq(&actual);
    }

    macro_rules! test_serialize_json {
        ($name:ident, $value:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let value = $value;
                let actual = serde_json::to_string_pretty(&value).unwrap();
                let expected = $expected;
                expected.assert_eq(&actual);
            }
        };
    }

    macro_rules! test_deserialize_json {
        ($name:ident, $type:ty, $value:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let value = $value;
                let actual: $type = serde_json::from_str(&value).unwrap();
                let expected = $expected;
                expected.assert_debug_eq(&actual);
            }
        };
    }

    test_serialize_json!(
        test_serialize_json_root,
        Root(vec![RootNode::Collection(Collection {
            namespace: Namespace {
                value: "abc/xyz".into()
            },
            name: "Account".into(),
            attributes: vec![
                CollectionAttribute::Property(Property {
                    name: "id".into(),
                    type_: Type::Primitive(Primitive {
                        value: PrimitiveType::String
                    }),
                    directives: vec![],
                    required: true,
                }),
                CollectionAttribute::Property(Property {
                    name: "balance".into(),
                    type_: Type::Primitive(Primitive {
                        value: PrimitiveType::Number
                    }),
                    directives: vec![],
                    required: true,
                }),
            ],
        })]),
        expect![[r#"
            [
              {
                "kind": "collection",
                "namespace": {
                  "kind": "namespace",
                  "value": "abc/xyz"
                },
                "name": "Account",
                "attributes": [
                  {
                    "kind": "property",
                    "name": "id",
                    "type": {
                      "kind": "primitive",
                      "value": "string"
                    },
                    "directives": [],
                    "required": true
                  },
                  {
                    "kind": "property",
                    "name": "balance",
                    "type": {
                      "kind": "primitive",
                      "value": "number"
                    },
                    "directives": [],
                    "required": true
                  }
                ]
              }
            ]"#]]
    );

    test_serialize_json!(
        test_serialize_json_attribute_property,
        CollectionAttribute::Property(Property {
            name: "id".into(),
            type_: Type::Primitive(Primitive {
                value: PrimitiveType::String
            }),
            directives: vec![],
            required: true,
        }),
        expect![[r#"
            {
              "kind": "property",
              "name": "id",
              "type": {
                "kind": "primitive",
                "value": "string"
              },
              "directives": [],
              "required": true
            }"#]]
    );

    test_serialize_json!(
        test_serialize_json_with_unknown_root_node,
        Root(vec![RootNode::Collection(Collection {
            namespace: Namespace {
                value: "abc/xyz".into()
            },
            name: "Account".into(),
            attributes: vec![CollectionAttribute::Property(Property {
                name: "id".into(),
                type_: Type::Primitive(Primitive {
                    value: PrimitiveType::String
                }),
                directives: vec![],
                required: true,
            })],
        })]),
        expect![[r#"
            [
              {
                "kind": "collection",
                "namespace": {
                  "kind": "namespace",
                  "value": "abc/xyz"
                },
                "name": "Account",
                "attributes": [
                  {
                    "kind": "property",
                    "name": "id",
                    "type": {
                      "kind": "primitive",
                      "value": "string"
                    },
                    "directives": [],
                    "required": true
                  }
                ]
              }
            ]"#]]
    );

    test_deserialize_json!(
        test_deserialize_collection,
        Root,
        r#"
            [
              {
                "kind": "collection",
                "namespace": {
                  "kind": "namespace",
                  "value": "abc/xyz"
                },
                "name": "Account",
                "attributes": [
                  {
                    "kind": "property",
                    "name": "id",
                    "type": {
                      "kind": "primitive",
                      "value": "string"
                    },
                    "directives": [],
                    "required": true
                  }
                ]
              }
            ]
        "#,
        expect![[r#"
            Root(
                [
                    Collection(
                        Collection {
                            namespace: Namespace {
                                value: "abc/xyz",
                            },
                            name: "Account",
                            attributes: [
                                Property(
                                    Property {
                                        name: "id",
                                        type_: Primitive(
                                            Primitive {
                                                value: String,
                                            },
                                        ),
                                        directives: [],
                                        required: true,
                                    },
                                ),
                            ],
                        },
                    ),
                ],
            )
        "#]]
    );

    test_deserialize_json!(
        test_deserialize_unknown_root_node,
        Root,
        r#"
            [
              {
                "kind": "some_new_kind",
                "unknown_field": ""
              }
            ]
        "#,
        expect![[r#"
            Root(
                [
                    Unknown,
                ],
            )
        "#]]
    );

    test_deserialize_json!(
        test_deserialize_unknown_attribute,
        CollectionAttribute,
        r#"
            {
              "kind": "some_new_kind",
              "unknown_field": ""
            }
        "#,
        expect![[r#"
            Unknown
        "#]]
    );

    test_deserialize_json!(
        test_deserialize_property_extra_field,
        Property,
        r#"
          {
            "kind": "property",
            "name": "id",
            "type": {
              "kind": "primitive",
              "value": "string"
            },
            "directives": [],
            "required": true,
            "unknown_field": ""
          }
        "#,
        expect![[r#"
            Property {
                name: "id",
                type_: Primitive(
                    Primitive {
                        value: String,
                    },
                ),
                directives: [],
                required: true,
            }
        "#]]
    );

    test_deserialize_json!(
        test_deserialize_method_attributes,
        Vec<MethodAttribute>,
        r#"
          [{
            "kind": "directive",
            "name": "read",
            "arguments": []
          }, {
            "kind": "parameter",
            "name": "from",
            "type": {
              "kind": "union",
              "value": []
            },
            "required": false
          }, {
            "kind": "returnvalue",
            "name": "from",
            "type": {
              "kind": "primitive",
              "value": "string"
            }
          }]"#,
        expect![[r#"
            [
                Directive(
                    Directive {
                        name: "read",
                        arguments: [],
                    },
                ),
                Parameter(
                    Parameter {
                        name: "from",
                        type_: Unknown,
                        required: false,
                    },
                ),
                ReturnValue(
                    ReturnValue {
                        name: "from",
                        type_: Primitive(
                            Primitive {
                                value: String,
                            },
                        ),
                    },
                ),
            ]
        "#]]
    );

    test_deserialize_json!(
        test_deserialize_directive,
        Directive,
        r#"
          {
            "kind": "directive",
            "name": "read",
            "arguments": []
          }
        "#,
        expect![[r#"
            Directive {
                name: "read",
                arguments: [],
            }
        "#]]
    );

    test_deserialize_json!(
        test_deserialize_parameter,
        Parameter,
        r#"
          {
            "kind": "parameter",
            "name": "from",
            "type": {
                "kind": "union",
                "value": []
            },
            "required": false
          }
        "#,
        expect![[r#"
            Parameter {
                name: "from",
                type_: Unknown,
                required: false,
            }
        "#]]
    );

    #[test]
    fn type_display_string() {
        let type_ = Type::Primitive(Primitive {
            value: PrimitiveType::String,
        });

        assert_eq!(type_.to_string(), "string");
    }

    #[test]
    fn type_display_object() {
        let type_ = Type::Object(Object {
            fields: vec![
                ObjectField {
                    name: Cow::Borrowed("b"),
                    type_: Type::Primitive(Primitive {
                        value: PrimitiveType::Boolean,
                    }),
                    required: true,
                },
                ObjectField {
                    name: Cow::Borrowed("num"),
                    type_: Type::Primitive(Primitive {
                        value: PrimitiveType::Number,
                    }),
                    required: false,
                },
                ObjectField {
                    name: Cow::Borrowed("emptyObject"),
                    type_: Type::Object(Object { fields: vec![] }),
                    required: true,
                },
                ObjectField {
                    name: Cow::Borrowed("col"),
                    type_: Type::ForeignRecord(ForeignRecord {
                        collection: Cow::Borrowed("Col"),
                    }),
                    required: true,
                },
                ObjectField {
                    name: Cow::Borrowed("stringArray"),
                    type_: Type::Array(Array {
                        value: Box::new(Type::Primitive(Primitive {
                            value: PrimitiveType::String,
                        })),
                    }),
                    required: true,
                },
                ObjectField {
                    name: Cow::Borrowed("mapStringToNumber"),
                    type_: Type::Map(Map {
                        key: Box::new(Type::Primitive(Primitive {
                            value: PrimitiveType::String,
                        })),
                        value: Box::new(Type::Primitive(Primitive {
                            value: PrimitiveType::Number,
                        })),
                    }),
                    required: true,
                },
                ObjectField {
                    name: Cow::Borrowed("publicKey"),
                    type_: Type::PublicKey(PublicKey {}),
                    required: true,
                },
            ],
        });

        assert_eq!(
            type_.to_string(),
            "{ b: boolean; num?: number; emptyObject: { }; col: Col; stringArray: string[]; mapStringToNumber: map<string, number>; publicKey: PublicKey; }"
        );
    }
}
