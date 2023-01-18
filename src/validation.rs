use base64::Engine;
use serde::Deserialize;
use std::{collections::HashMap, ops::Deref};

use crate::ast;

#[derive(Debug, PartialEq, Clone)]
enum PathPart<'a> {
    Field(&'a str),
    Index(usize),
}

impl std::fmt::Display for PathPart<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathPart::Field(field) => write!(f, "{}", field),
            PathPart::Index(index) => write!(f, "[{}]", index),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct PathParts<'a>(Vec<PathPart<'a>>);

impl std::fmt::Display for PathParts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Prints [Field("a"), Index(0), Field("b")] => "a[0].b"

        let mut parts = self.0.iter();
        if let Some(part) = parts.next() {
            write!(f, "{}", part)?;

            let mut last = part;
            for part in parts {
                match (last, part) {
                    (&PathPart::Field(_), PathPart::Field(_)) => write!(f, ".")?,
                    (&PathPart::Index(_), PathPart::Field(_)) => write!(f, ".")?,
                    (&PathPart::Field(_), PathPart::Index(_)) => write!(f, "")?,
                    (&PathPart::Index(_), PathPart::Index(_)) => write!(f, "")?,
                }
                write!(f, "{}", part)?;
                last = part;
            }
        }

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub enum ValidationError<'a> {
    InvalidType {
        path: PathParts<'a>,
        expected: ast::Type,
    },
    MissingField {
        path: PathParts<'a>,
    },
    ExtraField {
        path: PathParts<'a>,
    },
    Base64DecodeError {
        path: PathParts<'a>,
        error: base64::DecodeError,
    },
    Other {
        path: PathParts<'a>,
        message: String,
    },
}

impl std::fmt::Display for ValidationError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::InvalidType { path, expected } => {
                write!(
                    f,
                    "Invalid type at path {}, expected type {:?}",
                    path, expected,
                )
            }
            ValidationError::MissingField { path } => {
                write!(f, "Missing field at path {}", path)
            }
            ValidationError::ExtraField { path } => {
                write!(f, "Extra field at path {}", path)
            }
            ValidationError::Base64DecodeError { path, error } => {
                write!(f, "Base64 decode error at path {}: {}", path, error)
            }
            ValidationError::Other { path, message } => {
                write!(f, "Error at path {}: {}", path, message)
            }
        }
    }
}

impl std::error::Error for ValidationError<'_> {}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
}

pub(crate) fn validate_value<'a>(
    path: &mut PathParts<'a>,
    value: &'a Value,
    expected_type: &'a ast::Type,
) -> Result<(), ValidationError<'a>> {
    match expected_type {
        ast::Type::String => {
            if let Value::String(_) = value {
                Ok(())
            } else {
                Err(ValidationError::InvalidType {
                    path: path.clone(),
                    expected: expected_type.clone(),
                })
            }
        }
        ast::Type::Number => {
            if let Value::Number(_) = value {
                Ok(())
            } else {
                Err(ValidationError::InvalidType {
                    path: path.clone(),
                    expected: expected_type.clone(),
                })
            }
        }
        ast::Type::Boolean => {
            if let Value::Boolean(_) = value {
                Ok(())
            } else {
                Err(ValidationError::InvalidType {
                    path: path.clone(),
                    expected: expected_type.clone(),
                })
            }
        }
        ast::Type::Array(el) => {
            if let Value::Array(arr) = value {
                for (i, item) in arr.iter().enumerate() {
                    path.0.push(PathPart::Index(i));
                    validate_value(path, item, el.deref())?;
                    path.0.pop();
                }

                Ok(())
            } else {
                Err(ValidationError::InvalidType {
                    path: path.clone(),
                    expected: expected_type.clone(),
                })
            }
        }
        ast::Type::Map(kt, vt) => {
            if let Value::Map(map) = value {
                for (key, value) in map {
                    path.0.push(PathPart::Field(key));
                    match kt.deref() {
                        ast::Type::String => return Ok(()),
                        ast::Type::Number => {
                            if key.parse::<f64>().is_err() {
                                return Err(ValidationError::InvalidType {
                                    path: path.clone(),
                                    expected: ast::Type::Number,
                                });
                            }
                        }
                        _ => {
                            return Err(ValidationError::InvalidType {
                                path: path.clone(),
                                expected: ast::Type::String,
                            })
                        }
                    }
                    validate_value(path, value, vt.deref())?;
                    path.0.pop();
                }

                Ok(())
            } else {
                Err(ValidationError::InvalidType {
                    path: path.clone(),
                    expected: expected_type.clone(),
                })
            }
        }
        ast::Type::Object(obj) => {
            for field in obj {
                if field.required
                    && matches!(value, Value::Map(map) if !map.contains_key(&field.name))
                {
                    path.0.push(PathPart::Field(&field.name));
                    return Err(ValidationError::MissingField { path: path.clone() });
                }
            }

            if let Value::Map(map) = value {
                for (key, value) in map {
                    path.0.push(PathPart::Field(key));
                    if let Some(field) = obj.iter().find(|f| &f.name == key) {
                        validate_value(path, value, &field.type_)?;
                    } else {
                        return Err(ValidationError::ExtraField { path: path.clone() });
                    }
                    path.0.pop();
                }

                Ok(())
            } else {
                return Err(ValidationError::InvalidType {
                    path: path.clone(),
                    expected: expected_type.clone(),
                });
            }
        }
        ast::Type::PublicKey => {
            if let Value::Map(map) = value {
                match (
                    map.get("kty"),
                    map.get("crv"),
                    map.get("alg"),
                    map.get("use"),
                    map.get("x"),
                    map.get("y"),
                ) {
                    (Some(kty), Some(crv), Some(alg), Some(use_), Some(x), Some(y)) => {
                        if let Some(extra_field) = map.iter().find(|(k, _)| {
                            !matches!(k.as_str(), "kty" | "crv" | "alg" | "use" | "x" | "y")
                        }) {
                            let mut path = path.clone();
                            path.0.push(PathPart::Field(extra_field.0));
                            return Err(ValidationError::ExtraField { path });
                        }

                        match kty {
                            Value::String(s) if s == "EC" => {}
                            _ => {
                                let mut path = path.clone();
                                path.0.push(PathPart::Field("kty"));
                                return Err(ValidationError::Other {
                                    path,
                                    message: "Invalid kty, should be EC".to_string(),
                                });
                            }
                        }

                        match crv {
                            Value::String(s) if s == "secp256k1" => {}
                            _ => {
                                let mut path = path.clone();
                                path.0.push(PathPart::Field("crv"));
                                return Err(ValidationError::Other {
                                    path,
                                    message: "Invalid crv, should be secp256k1".to_string(),
                                });
                            }
                        }

                        match alg {
                            Value::String(s) if s == "ES256K" => {}
                            _ => {
                                let mut path = path.clone();
                                path.0.push(PathPart::Field("alg"));
                                return Err(ValidationError::Other {
                                    path,
                                    message: "Invalid alg, should be ES256K".to_string(),
                                });
                            }
                        }

                        match use_ {
                            Value::String(s) if s == "sig" => {}
                            _ => {
                                let mut path = path.clone();
                                path.0.push(PathPart::Field("use"));
                                return Err(ValidationError::Other {
                                    path,
                                    message: "Invalid use, should be sig".to_string(),
                                });
                            }
                        }

                        let x = match x {
                            Value::String(s) => base64::engine::general_purpose::URL_SAFE
                                .decode(s.as_bytes())
                                .map_err(|err| {
                                    let mut path = path.clone();
                                    path.0.push(PathPart::Field("x"));
                                    ValidationError::Base64DecodeError { path, error: err }
                                })?,
                            _ => {
                                let mut path = path.clone();
                                path.0.push(PathPart::Field("x"));
                                return Err(ValidationError::InvalidType {
                                    path,
                                    expected: ast::Type::String,
                                });
                            }
                        };

                        let y = match y {
                            Value::String(s) => base64::engine::general_purpose::URL_SAFE
                                .decode(s.as_bytes())
                                .map_err(|err| {
                                    let mut path = path.clone();
                                    path.0.push(PathPart::Field("y"));
                                    ValidationError::Base64DecodeError { path, error: err }
                                })?,
                            _ => {
                                let mut path = path.clone();
                                path.0.push(PathPart::Field("y"));
                                return Err(ValidationError::InvalidType {
                                    path,
                                    expected: ast::Type::String,
                                });
                            }
                        };

                        if x.len() != 32 {
                            let mut path = path.clone();
                            path.0.push(PathPart::Field("x"));
                            return Err(ValidationError::Other {
                                path,
                                message: "Invalid length, expected 32 bytes".to_string(),
                            });
                        }

                        if y.len() != 32 {
                            let mut path = path.clone();
                            path.0.push(PathPart::Field("y"));
                            return Err(ValidationError::Other {
                                path,
                                message: "Invalid length, expected 32 bytes".to_string(),
                            });
                        }

                        Ok(())
                    }
                    (None, _, _, _, _, _) => {
                        let mut path = path.clone();
                        path.0.push(PathPart::Field("kty"));

                        Err(ValidationError::MissingField { path })
                    }
                    (_, None, _, _, _, _) => {
                        let mut path = path.clone();
                        path.0.push(PathPart::Field("crv"));

                        Err(ValidationError::MissingField { path })
                    }
                    (_, _, None, _, _, _) => {
                        let mut path = path.clone();
                        path.0.push(PathPart::Field("alg"));

                        Err(ValidationError::MissingField { path })
                    }
                    (_, _, _, None, _, _) => {
                        let mut path = path.clone();
                        path.0.push(PathPart::Field("use"));

                        Err(ValidationError::MissingField { path })
                    }
                    (_, _, _, _, None, _) => {
                        let mut path = path.clone();
                        path.0.push(PathPart::Field("x"));

                        Err(ValidationError::MissingField { path })
                    }
                    (_, _, _, _, _, None) => {
                        let mut path = path.clone();
                        path.0.push(PathPart::Field("y"));

                        Err(ValidationError::MissingField { path })
                    }
                }
            } else {
                Err(ValidationError::InvalidType {
                    path: path.clone(),
                    expected: expected_type.clone(),
                })
            }
        }
        ast::Type::ForeignRecord { collection } => {
            if let Value::Map(map) = value {
                if let Some(extra_field) = map.keys().filter(|k| *k != "id").nth(0) {
                    let mut path = path.clone();
                    path.0.push(PathPart::Field(extra_field));
                    return Err(ValidationError::ExtraField { path });
                }

                if let Some(id) = map.get("id") {
                    if let Value::String(_) = id {
                        Ok(())
                    } else {
                        let mut path = path.clone();
                        path.0.push(PathPart::Field("id"));
                        Err(ValidationError::InvalidType {
                            path,
                            expected: ast::Type::String,
                        })
                    }
                } else {
                    let mut path = path.clone();
                    path.0.push(PathPart::Field("id"));
                    Err(ValidationError::MissingField { path })
                }
            } else {
                Err(ValidationError::InvalidType {
                    path: path.clone(),
                    expected: expected_type.clone(),
                })
            }
        }
    }
}

pub(crate) fn validate_set<'a>(
    collection: &'a ast::Collection,
    data: &'a HashMap<String, Value>,
) -> Result<(), ValidationError<'a>> {
    let fields = collection
        .items
        .iter()
        .filter_map(|item| {
            if let ast::CollectionItem::Field(field) = item {
                Some(field)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    for field in &fields {
        let value = data.get(&field.name);
        if field.required && value.is_none() {
            return Err(ValidationError::MissingField {
                path: PathParts(vec![PathPart::Field(&field.name)]),
            });
        }

        if let Some(value) = data.get(&field.name) {
            validate_value(
                &mut PathParts(vec![PathPart::Field(&field.name)]),
                value,
                &field.type_,
            )?;
        }
    }

    for (key, _) in data {
        if !fields.iter().any(|item| item.name.as_str() == key.as_str()) {
            return Err(ValidationError::ExtraField {
                path: PathParts(vec![PathPart::Field(key)]),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_set() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![
                ast::CollectionItem::Field(ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                    decorators: vec![],
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
                    decorators: vec![],
                }),
            ],
        };

        let data = HashMap::from([
            ("name".to_string(), Value::String("John".to_string())),
            ("age".to_string(), Value::Number(30.0)),
        ]);

        assert!(validate_set(&collection, &data).is_ok());
    }

    #[test]
    fn test_validate_set_array() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Array(Box::new(ast::Type::String)),
                required: false,
                decorators: vec![],
            })],
        };

        let data = HashMap::from([(
            "tags".to_string(),
            Value::Array(vec![
                Value::String("tag1".to_string()),
                Value::String("tag2".to_string()),
            ]),
        )]);

        assert!(validate_set(&collection, &data).is_ok());
    }

    #[test]
    fn test_validate_set_array_invalid_array_value() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Array(Box::new(ast::Type::String)),
                required: false,
                decorators: vec![],
            })],
        };

        let data = HashMap::from([(
            "tags".to_string(),
            Value::Array(vec![Value::String("tag1".to_string()), Value::Number(2.0)]),
        )]);

        let result = validate_set(&collection, &data);
        assert!(result.is_err());

        assert_eq!(
            result.unwrap_err(),
            ValidationError::InvalidType {
                path: PathParts(vec![PathPart::Field("tags"), PathPart::Index(1)]),
                expected: ast::Type::String,
            }
        );
    }

    #[test]
    fn test_validate_map() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Map(Box::new(ast::Type::String), Box::new(ast::Type::Number)),
                required: false,
                decorators: vec![],
            })],
        };

        let data = HashMap::from([(
            "tags".to_string(),
            Value::Map(HashMap::from([
                ("tag1".to_string(), Value::Number(1.0)),
                ("tag2".to_string(), Value::Number(2.0)),
            ])),
        )]);

        assert!(validate_set(&collection, &data).is_ok());
    }

    #[test]
    fn test_validate_nested_map() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Map(
                    Box::new(ast::Type::String),
                    Box::new(ast::Type::Map(
                        Box::new(ast::Type::String),
                        Box::new(ast::Type::Number),
                    )),
                ),
                required: false,
                decorators: vec![],
            })],
        };

        let data = HashMap::from([(
            "tags".to_string(),
            Value::Map(HashMap::from([
                (
                    "tag1".to_string(),
                    Value::Map(HashMap::from([
                        ("tag1.1".to_string(), Value::Number(1.0)),
                        ("tag1.2".to_string(), Value::Number(2.0)),
                    ])),
                ),
                (
                    "tag2".to_string(),
                    Value::Map(HashMap::from([
                        ("tag2.1".to_string(), Value::Number(1.0)),
                        ("tag2.2".to_string(), Value::Number(2.0)),
                    ])),
                ),
            ])),
        )]);

        assert!(validate_set(&collection, &data).is_ok());
    }

    #[test]
    fn test_validate_map_number_key() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Map(Box::new(ast::Type::Number), Box::new(ast::Type::Number)),
                required: false,
                decorators: vec![],
            })],
        };

        let data = HashMap::from([(
            "tags".to_string(),
            Value::Map(HashMap::from([
                ("1".to_string(), Value::Number(1.0)),
                ("2.3".to_string(), Value::Number(2.0)),
            ])),
        )]);

        assert!(validate_set(&collection, &data).is_ok());
    }

    #[test]
    fn test_validate_map_number_key_invalid() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Map(Box::new(ast::Type::Number), Box::new(ast::Type::Number)),
                required: false,
                decorators: vec![],
            })],
        };

        let data = HashMap::from([(
            "tags".to_string(),
            Value::Map(HashMap::from([
                ("1".to_string(), Value::Number(1.0)),
                ("str".to_string(), Value::Number(2.0)),
            ])),
        )]);

        let result = validate_set(&collection, &data);
        assert!(result.is_err());

        assert_eq!(
            result.unwrap_err(),
            ValidationError::InvalidType {
                path: PathParts(vec![PathPart::Field("tags"), PathPart::Field("str")]),
                expected: ast::Type::Number,
            }
        );
    }

    #[test]
    fn test_validate_map_invalid_key() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Map(Box::new(ast::Type::Number), Box::new(ast::Type::Number)),
                required: false,
                decorators: vec![],
            })],
        };

        let data = HashMap::from([(
            "tags".to_string(),
            Value::Map(HashMap::from([
                ("tag1".to_string(), Value::Number(1.0)),
                ("2".to_string(), Value::Number(2.0)),
            ])),
        )]);

        let result = validate_set(&collection, &data);
        assert!(result.is_err());

        assert_eq!(
            result.unwrap_err(),
            ValidationError::InvalidType {
                path: PathParts(vec![PathPart::Field("tags"), PathPart::Field("tag1")]),
                expected: ast::Type::Number,
            }
        );
    }

    #[test]
    fn test_validate_object() {
        let cases = [
            (
                ast::Collection {
                    name: "users".to_string(),
                    decorators: vec![],
                    items: vec![ast::CollectionItem::Field(ast::Field {
                        name: "info".to_string(),
                        type_: ast::Type::Object(vec![ast::Field {
                            name: "name".to_string(),
                            type_: ast::Type::String,
                            required: true,
                            decorators: vec![],
                        }]),
                        required: true,
                        decorators: vec![],
                    })],
                },
                HashMap::from([(
                    "info".to_string(),
                    Value::Map(HashMap::from([(
                        "name".to_string(),
                        Value::String("John".to_string()),
                    )])),
                )]),
            ),
            (
                ast::Collection {
                    name: "users".to_string(),
                    decorators: vec![],
                    items: vec![ast::CollectionItem::Field(ast::Field {
                        name: "info".to_string(),
                        type_: ast::Type::Object(vec![ast::Field {
                            name: "name".to_string(),
                            type_: ast::Type::String,
                            required: false,
                            decorators: vec![],
                        }]),
                        required: true,
                        decorators: vec![],
                    })],
                },
                HashMap::from([("info".to_string(), Value::Map(HashMap::from([])))]),
            ),
            (
                ast::Collection {
                    name: "users".to_string(),
                    decorators: vec![],
                    items: vec![ast::CollectionItem::Field(ast::Field {
                        name: "info".to_string(),
                        type_: ast::Type::Object(vec![ast::Field {
                            name: "name".to_string(),
                            type_: ast::Type::String,
                            required: true,
                            decorators: vec![],
                        }]),
                        required: false,
                        decorators: vec![],
                    })],
                },
                HashMap::from([]),
            ),
        ];

        for (collection, data) in cases.into_iter() {
            assert!(
                validate_set(&collection, &data).is_ok(),
                "failed to validate: {:?}",
                data
            );
        }
    }

    #[test]
    fn test_validate_object_missing_field() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "info".to_string(),
                type_: ast::Type::Object(vec![ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                    decorators: vec![],
                }]),
                required: true,
                decorators: vec![],
            })],
        };

        let data = HashMap::from([("info".to_string(), Value::Map(HashMap::from([])))]);

        let result = validate_set(&collection, &data);
        assert!(result.is_err());

        assert_eq!(
            result.unwrap_err(),
            ValidationError::MissingField {
                path: PathParts(vec![PathPart::Field("info"), PathPart::Field("name")]),
            }
        );
    }

    #[test]
    fn test_validate_object_extra_field() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "info".to_string(),
                type_: ast::Type::Object(vec![ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                    decorators: vec![],
                }]),
                required: true,
                decorators: vec![],
            })],
        };

        let data = HashMap::from([(
            "info".to_string(),
            Value::Map(HashMap::from([
                ("name".to_string(), Value::String("John".to_string())),
                ("age".to_string(), Value::Number(30.0)),
            ])),
        )]);

        let result = validate_set(&collection, &data);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(
            error,
            ValidationError::ExtraField {
                path: PathParts(vec![PathPart::Field("info"), PathPart::Field("age")]),
            },
        );
    }

    #[test]
    fn test_validate_set_missing_required_field() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![
                ast::CollectionItem::Field(ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                    decorators: vec![],
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
                    decorators: vec![],
                }),
            ],
        };

        let data = HashMap::from([("age".to_string(), Value::Number(30.0))]);

        let result = validate_set(&collection, &data);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(
            error,
            ValidationError::MissingField {
                path: PathParts(vec![PathPart::Field("name")]),
            },
        );
    }

    #[test]
    fn test_validate_set_invalid_type() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![
                ast::CollectionItem::Field(ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                    decorators: vec![],
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
                    decorators: vec![],
                }),
            ],
        };

        let data = HashMap::from([
            ("name".to_string(), Value::Number(30.0)),
            ("age".to_string(), Value::String("30".to_string())),
        ]);

        let result = validate_set(&collection, &data);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(
            error,
            ValidationError::InvalidType {
                path: PathParts(vec![PathPart::Field("name")]),
                expected: ast::Type::String,
            },
        );
    }

    #[test]
    fn test_validate_set_extra_field() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![
                ast::CollectionItem::Field(ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                    decorators: vec![],
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
                    decorators: vec![],
                }),
            ],
        };

        let data = HashMap::from([
            ("name".to_string(), Value::String("John".to_string())),
            ("age".to_string(), Value::Number(30.0)),
            ("extra".to_string(), Value::String("extra".to_string())),
        ]);

        let result = validate_set(&collection, &data);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(
            error,
            ValidationError::ExtraField {
                path: PathParts(vec![PathPart::Field("extra")]),
            },
        );
    }

    #[test]
    fn test_validate_boolean() {
        let collection = ast::Collection {
            name: "users".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "is_admin".to_string(),
                type_: ast::Type::Boolean,
                required: true,
                decorators: vec![],
            })],
        };

        assert!(validate_set(
            &collection,
            &HashMap::from([("is_admin".to_string(), Value::Boolean(true))])
        )
        .is_ok());

        assert!(validate_set(
            &collection,
            &HashMap::from([("is_admin".to_string(), Value::Boolean(false))])
        )
        .is_ok());

        assert_eq!(
            validate_set(
                &collection,
                &HashMap::from([("is_admin".to_string(), Value::Number(1.0))])
            ),
            Err(ValidationError::InvalidType {
                path: PathParts(vec![PathPart::Field("is_admin")]),
                expected: ast::Type::Boolean,
            })
        );
    }

    macro_rules! test_validate_public_key {
        ($name:ident, $data:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let collection = ast::Collection {
                    name: "users".to_string(),
                    decorators: vec![],
                    items: vec![ast::CollectionItem::Field(ast::Field {
                        name: "public_key".to_string(),
                        type_: ast::Type::PublicKey,
                        required: true,
                        decorators: vec![],
                    })],
                };
                let data = $data;
                let result = validate_set(&collection, &data);

                assert_eq!(result, $expected, "{:?}", result);
            }
        };
    }

    test_validate_public_key!(
        test_validate_public_key_correct,
        HashMap::from([(
            "public_key".to_string(),
            Value::Map(HashMap::from([
                ("kty".to_string(), Value::String("EC".to_string())),
                ("crv".to_string(), Value::String("secp256k1".to_string())),
                ("alg".to_string(), Value::String("ES256K".to_string())),
                ("use".to_string(), Value::String("sig".to_string())),
                (
                    "x".to_string(),
                    Value::String(
                        base64::engine::general_purpose::URL_SAFE
                            .encode(&rand::random::<[u8; 32]>())
                    )
                ),
                (
                    "y".to_string(),
                    Value::String(
                        base64::engine::general_purpose::URL_SAFE
                            .encode(&rand::random::<[u8; 32]>())
                    )
                ),
            ])),
        )]),
        Ok(())
    );

    test_validate_public_key!(
        test_validate_public_key_invalid_x,
        HashMap::from([(
            "public_key".to_string(),
            Value::Map(HashMap::from([
                ("kty".to_string(), Value::String("EC".to_string())),
                ("crv".to_string(), Value::String("secp256k1".to_string())),
                ("alg".to_string(), Value::String("ES256K".to_string())),
                ("use".to_string(), Value::String("sig".to_string())),
                (
                    "x".to_string(),
                    Value::String(
                        base64::engine::general_purpose::URL_SAFE
                            .encode(&rand::random::<[u8; 16]>())
                    )
                ),
                (
                    "y".to_string(),
                    Value::String(
                        base64::engine::general_purpose::URL_SAFE
                            .encode(&rand::random::<[u8; 32]>())
                    )
                ),
            ])),
        )]),
        Err(ValidationError::Other {
            path: PathParts(vec![PathPart::Field("public_key"), PathPart::Field("x")]),
            message: "Invalid length, expected 32 bytes".to_string(),
        })
    );

    test_validate_public_key!(
        test_validate_public_key_invalid_y,
        HashMap::from([(
            "public_key".to_string(),
            Value::Map(HashMap::from([
                ("kty".to_string(), Value::String("EC".to_string())),
                ("crv".to_string(), Value::String("secp256k1".to_string())),
                ("alg".to_string(), Value::String("ES256K".to_string())),
                ("use".to_string(), Value::String("sig".to_string())),
                (
                    "x".to_string(),
                    Value::String(
                        base64::engine::general_purpose::URL_SAFE
                            .encode(&rand::random::<[u8; 32]>())
                    )
                ),
                (
                    "y".to_string(),
                    Value::String(
                        base64::engine::general_purpose::URL_SAFE
                            .encode(&rand::random::<[u8; 16]>())
                    )
                ),
            ])),
        )]),
        Err(ValidationError::Other {
            path: PathParts(vec![PathPart::Field("public_key"), PathPart::Field("y")]),
            message: "Invalid length, expected 32 bytes".to_string(),
        })
    );

    test_validate_public_key!(
        test_validate_public_key_missing_kty,
        HashMap::from([("public_key".to_string(), Value::Map(HashMap::from([])),)]),
        Err(ValidationError::MissingField {
            path: PathParts(vec![PathPart::Field("public_key"), PathPart::Field("kty")]),
        })
    );

    test_validate_public_key!(
        test_validate_public_key_missing_crv,
        HashMap::from([(
            "public_key".to_string(),
            Value::Map(HashMap::from([(
                "kty".to_string(),
                Value::String("EC".to_string())
            ),])),
        )]),
        Err(ValidationError::MissingField {
            path: PathParts(vec![PathPart::Field("public_key"), PathPart::Field("crv")]),
        })
    );

    test_validate_public_key!(
        test_validate_public_key_missing_alg,
        HashMap::from([(
            "public_key".to_string(),
            Value::Map(HashMap::from([
                ("kty".to_string(), Value::String("EC".to_string())),
                ("crv".to_string(), Value::String("secp256k1".to_string()))
            ])),
        )]),
        Err(ValidationError::MissingField {
            path: PathParts(vec![PathPart::Field("public_key"), PathPart::Field("alg")]),
        })
    );

    test_validate_public_key!(
        test_validate_public_key_missing_use,
        HashMap::from([(
            "public_key".to_string(),
            Value::Map(HashMap::from([
                ("kty".to_string(), Value::String("EC".to_string())),
                ("crv".to_string(), Value::String("secp256k1".to_string())),
                ("alg".to_string(), Value::String("ES256K".to_string()))
            ])),
        )]),
        Err(ValidationError::MissingField {
            path: PathParts(vec![PathPart::Field("public_key"), PathPart::Field("use")]),
        })
    );

    test_validate_public_key!(
        test_validate_public_key_missing_x,
        HashMap::from([(
            "public_key".to_string(),
            Value::Map(HashMap::from([
                ("kty".to_string(), Value::String("EC".to_string())),
                ("crv".to_string(), Value::String("secp256k1".to_string())),
                ("alg".to_string(), Value::String("ES256K".to_string())),
                ("use".to_string(), Value::String("sig".to_string()))
            ])),
        )]),
        Err(ValidationError::MissingField {
            path: PathParts(vec![PathPart::Field("public_key"), PathPart::Field("x")]),
        })
    );

    test_validate_public_key!(
        test_validate_public_key_missing_y,
        HashMap::from([(
            "public_key".to_string(),
            Value::Map(HashMap::from([
                ("kty".to_string(), Value::String("EC".to_string())),
                ("crv".to_string(), Value::String("secp256k1".to_string())),
                ("alg".to_string(), Value::String("ES256K".to_string())),
                ("use".to_string(), Value::String("sig".to_string())),
                (
                    "x".to_string(),
                    Value::String(
                        base64::engine::general_purpose::URL_SAFE
                            .encode(&rand::random::<[u8; 32]>())
                    )
                )
            ])),
        )]),
        Err(ValidationError::MissingField {
            path: PathParts(vec![PathPart::Field("public_key"), PathPart::Field("y")]),
        })
    );

    test_validate_public_key!(
        test_validate_public_key_extra_field,
        HashMap::from([(
            "public_key".to_string(),
            Value::Map(HashMap::from([
                ("kty".to_string(), Value::String("RSA".to_string())),
                ("crv".to_string(), Value::String("secp256k1".to_string())),
                ("alg".to_string(), Value::String("ES256K".to_string())),
                ("use".to_string(), Value::String("sig".to_string())),
                (
                    "x".to_string(),
                    Value::String(
                        base64::engine::general_purpose::URL_SAFE
                            .encode(&rand::random::<[u8; 32]>())
                    )
                ),
                (
                    "y".to_string(),
                    Value::String(
                        base64::engine::general_purpose::URL_SAFE
                            .encode(&rand::random::<[u8; 32]>())
                    )
                ),
                ("extra".to_string(), Value::String("extra".to_string()))
            ])),
        )]),
        Err(ValidationError::ExtraField {
            path: PathParts(vec![
                PathPart::Field("public_key"),
                PathPart::Field("extra")
            ]),
        })
    );

    #[test]
    fn test_validate_public_key_optional() {
        let collection = ast::Collection {
            name: "Collection".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "public_key".to_string(),
                type_: ast::Type::PublicKey,
                required: false,
                decorators: vec![],
            })],
        };

        let data = HashMap::new();

        let result = validate_set(&collection, &data);
        assert_eq!(result, Ok(()));
    }

    macro_rules! test_validate_foreign_record {
        ($name:ident, $data:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let collection = ast::Collection {
                    name: "Collection".to_string(),
                    decorators: vec![],
                    items: vec![ast::CollectionItem::Field(ast::Field {
                        name: "foreign_record".to_string(),
                        type_: ast::Type::ForeignRecord {
                            collection: "ForeignCollection".to_string(),
                        },
                        required: true,
                        decorators: vec![],
                    })],
                };

                let data = $data;
                let result = validate_set(&collection, &data);
                assert_eq!(result, $expected);
            }
        };
    }

    test_validate_foreign_record!(
        test_validate_foreign_record,
        HashMap::from([(
            "foreign_record".to_string(),
            Value::Map(HashMap::from([(
                "id".to_string(),
                Value::String("id".to_string())
            )])),
        )]),
        Ok(())
    );

    test_validate_foreign_record!(
        test_validate_foreign_record_missing_id,
        HashMap::from([("foreign_record".to_string(), Value::Map(HashMap::from([])))]),
        Err(ValidationError::MissingField {
            path: PathParts(vec![
                PathPart::Field("foreign_record"),
                PathPart::Field("id")
            ]),
        })
    );

    test_validate_foreign_record!(
        test_validate_foreign_record_extra_field,
        HashMap::from([(
            "foreign_record".to_string(),
            Value::Map(HashMap::from([
                ("id".to_string(), Value::String("id".to_string())),
                ("extra".to_string(), Value::String("extra".to_string()))
            ])),
        )]),
        Err(ValidationError::ExtraField {
            path: PathParts(vec![
                PathPart::Field("foreign_record"),
                PathPart::Field("extra")
            ]),
        })
    );

    #[test]
    fn test_validate_foreign_record_optional() {
        let collection = ast::Collection {
            name: "Collection".to_string(),
            decorators: vec![],
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "foreign_record".to_string(),
                type_: ast::Type::ForeignRecord {
                    collection: "ForeignCollection".to_string(),
                },
                required: false,
                decorators: vec![],
            })],
        };

        let data = HashMap::new();

        let result = validate_set(&collection, &data);
        assert_eq!(result, Ok(()));
    }
}
