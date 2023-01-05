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
            items: vec![
                ast::CollectionItem::Field(ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
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
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Array(Box::new(ast::Type::String)),
                required: false,
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
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Array(Box::new(ast::Type::String)),
                required: false,
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
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Map(Box::new(ast::Type::String), Box::new(ast::Type::Number)),
                required: false,
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
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Map(Box::new(ast::Type::Number), Box::new(ast::Type::Number)),
                required: false,
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
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Map(Box::new(ast::Type::Number), Box::new(ast::Type::Number)),
                required: false,
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
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "tags".to_string(),
                type_: ast::Type::Map(Box::new(ast::Type::Number), Box::new(ast::Type::Number)),
                required: false,
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
                    items: vec![ast::CollectionItem::Field(ast::Field {
                        name: "info".to_string(),
                        type_: ast::Type::Object(vec![ast::Field {
                            name: "name".to_string(),
                            type_: ast::Type::String,
                            required: true,
                        }]),
                        required: true,
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
                    items: vec![ast::CollectionItem::Field(ast::Field {
                        name: "info".to_string(),
                        type_: ast::Type::Object(vec![ast::Field {
                            name: "name".to_string(),
                            type_: ast::Type::String,
                            required: false,
                        }]),
                        required: true,
                    })],
                },
                HashMap::from([("info".to_string(), Value::Map(HashMap::from([])))]),
            ),
            (
                ast::Collection {
                    name: "users".to_string(),
                    items: vec![ast::CollectionItem::Field(ast::Field {
                        name: "info".to_string(),
                        type_: ast::Type::Object(vec![ast::Field {
                            name: "name".to_string(),
                            type_: ast::Type::String,
                            required: true,
                        }]),
                        required: false,
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
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "info".to_string(),
                type_: ast::Type::Object(vec![ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                }]),
                required: true,
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
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "info".to_string(),
                type_: ast::Type::Object(vec![ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                }]),
                required: true,
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
            items: vec![
                ast::CollectionItem::Field(ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
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
            items: vec![
                ast::CollectionItem::Field(ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
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
            items: vec![
                ast::CollectionItem::Field(ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
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
            items: vec![ast::CollectionItem::Field(ast::Field {
                name: "is_admin".to_string(),
                type_: ast::Type::Boolean,
                required: true,
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
}
