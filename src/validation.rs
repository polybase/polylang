use serde::Deserialize;
use std::{borrow::Cow, collections::HashMap, ops::Deref};

use crate::stableast;

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
        expected: stableast::Type<'a>,
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
    expected_type: &'a stableast::Type<'a>,
) -> Result<(), ValidationError<'a>> {
    match expected_type {
        stableast::Type::Primitive(p) => match p.value {
            stableast::PrimitiveType::String => {
                if let Value::String(_) = value {
                    Ok(())
                } else {
                    Err(ValidationError::InvalidType {
                        path: path.clone(),
                        expected: expected_type.clone(),
                    })
                }
            }
            stableast::PrimitiveType::Number => {
                if let Value::Number(_) = value {
                    Ok(())
                } else {
                    Err(ValidationError::InvalidType {
                        path: path.clone(),
                        expected: expected_type.clone(),
                    })
                }
            }
            stableast::PrimitiveType::Boolean => {
                if let Value::Boolean(_) = value {
                    Ok(())
                } else {
                    Err(ValidationError::InvalidType {
                        path: path.clone(),
                        expected: expected_type.clone(),
                    })
                }
            }
        },
        stableast::Type::Array(a) => {
            if let Value::Array(arr) = value {
                for (i, item) in arr.iter().enumerate() {
                    path.0.push(PathPart::Index(i));
                    validate_value(path, item, a.value.deref())?;
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
        stableast::Type::Map(m) => {
            let kt = m.key.as_ref();
            let vt = m.value.as_ref();

            if let Value::Map(map) = value {
                for (key, value) in map {
                    path.0.push(PathPart::Field(key));
                    match kt.deref() {
                        stableast::Type::Primitive(p) => match p.value {
                            stableast::PrimitiveType::String => return Ok(()),
                            stableast::PrimitiveType::Number => {
                                if key.parse::<f64>().is_err() {
                                    return Err(ValidationError::InvalidType {
                                        path: path.clone(),
                                        expected: stableast::Type::Primitive(
                                            stableast::Primitive {
                                                value: stableast::PrimitiveType::Number,
                                            },
                                        ),
                                    });
                                }
                            }
                            _ => {
                                return Err(ValidationError::InvalidType {
                                    path: path.clone(),
                                    expected: stableast::Type::Primitive(stableast::Primitive {
                                        value: stableast::PrimitiveType::String,
                                    }),
                                })
                            }
                        },
                        _ => {
                            return Err(ValidationError::InvalidType {
                                path: path.clone(),
                                expected: stableast::Type::Primitive(stableast::Primitive {
                                    value: stableast::PrimitiveType::String,
                                }),
                            })
                        }
                    }
                    validate_value(path, value, vt)?;
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
        stableast::Type::Object(obj) => {
            for field in &obj.fields {
                if field.required
                    && matches!(value, Value::Map(map) if !map.contains_key(field.name.as_ref()))
                {
                    path.0.push(PathPart::Field(&field.name));
                    return Err(ValidationError::MissingField { path: path.clone() });
                }
            }

            if let Value::Map(map) = value {
                for (key, value) in map {
                    path.0.push(PathPart::Field(key));
                    if let Some(field) = obj.fields.iter().find(|f| f.name == Cow::Borrowed(key)) {
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
        stableast::Type::Record(_) => {
            return Err(ValidationError::InvalidType {
                path: path.clone(),
                expected: expected_type.clone(),
            })
        }
        stableast::Type::ForeignRecord(_) => {
            return Err(ValidationError::InvalidType {
                path: path.clone(),
                expected: expected_type.clone(),
            })
        }
        stableast::Type::Unknown => {
            return Err(ValidationError::InvalidType {
                path: path.clone(),
                expected: expected_type.clone(),
            })
        }
    }
}

pub(crate) fn validate_set<'a>(
    collection: &'a stableast::Collection,
    data: &'a HashMap<String, Value>,
) -> Result<(), ValidationError<'a>> {
    let fields = collection
        .attributes
        .iter()
        .filter_map(|item| {
            if let stableast::CollectionAttribute::Property(prop) = item {
                Some(prop)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    for field in &fields {
        let value = data.get(field.name.as_ref());
        if field.required && value.is_none() {
            return Err(ValidationError::MissingField {
                path: PathParts(vec![PathPart::Field(&field.name)]),
            });
        }

        if let Some(value) = data.get(field.name.as_ref()) {
            validate_value(
                &mut PathParts(vec![PathPart::Field(&field.name)]),
                value,
                &field.type_,
            )?;
        }
    }

    for (key, _) in data {
        if !fields.iter().any(|item| item.name == key.as_str()) {
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![
                stableast::CollectionAttribute::Property(stableast::Property {
                    name: "name".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::String,
                    }),
                    required: true,
                }),
                stableast::CollectionAttribute::Property(stableast::Property {
                    name: "age".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::Number,
                    }),
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "tags".into(),
                    type_: stableast::Type::Array(stableast::Array {
                        value: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::String,
                        })),
                    }),
                    required: false,
                },
            )],
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "tags".into(),
                    type_: stableast::Type::Array(stableast::Array {
                        value: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::String,
                        })),
                    }),
                    required: false,
                },
            )],
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
                expected: stableast::Type::Primitive(stableast::Primitive {
                    value: stableast::PrimitiveType::String
                }),
            }
        );
    }

    #[test]
    fn test_validate_map() {
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "tags".into(),
                    type_: stableast::Type::Map(stableast::Map {
                        key: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::String,
                        })),
                        value: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::Number,
                        })),
                    }),
                    required: false,
                },
            )],
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "tags".into(),
                    type_: stableast::Type::Map(stableast::Map {
                        key: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::String,
                        })),
                        value: Box::new(stableast::Type::Map(stableast::Map {
                            key: Box::new(stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::String,
                            })),
                            value: Box::new(stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::Number,
                            })),
                        })),
                    }),
                    required: false,
                },
            )],
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "tags".into(),
                    type_: stableast::Type::Map(stableast::Map {
                        key: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::Number,
                        })),
                        value: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::Number,
                        })),
                    }),
                    required: false,
                },
            )],
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "tags".into(),
                    type_: stableast::Type::Map(stableast::Map {
                        key: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::Number,
                        })),
                        value: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::Number,
                        })),
                    }),
                    required: false,
                },
            )],
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
                expected: stableast::Type::Primitive(stableast::Primitive {
                    value: stableast::PrimitiveType::Number
                }),
            }
        );
    }

    #[test]
    fn test_validate_map_invalid_key() {
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "tags".into(),
                    type_: stableast::Type::Map(stableast::Map {
                        key: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::Number,
                        })),
                        value: Box::new(stableast::Type::Primitive(stableast::Primitive {
                            value: stableast::PrimitiveType::Number,
                        })),
                    }),
                    required: false,
                },
            )],
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
                expected: stableast::Type::Primitive(stableast::Primitive {
                    value: stableast::PrimitiveType::Number,
                }),
            }
        );
    }

    #[test]
    fn test_validate_object() {
        let cases = [
            (
                stableast::Collection {
                    namespace: stableast::Namespace { value: "ns".into() },
                    name: "users".into(),
                    attributes: vec![stableast::CollectionAttribute::Property(
                        stableast::Property {
                            name: "info".into(),
                            type_: stableast::Type::Object(stableast::Object {
                                fields: vec![stableast::ObjectField {
                                    name: "name".into(),
                                    type_: stableast::Type::Primitive(stableast::Primitive {
                                        value: stableast::PrimitiveType::String,
                                    }),
                                    required: true,
                                }],
                            }),
                            required: true,
                        },
                    )],
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
                stableast::Collection {
                    namespace: stableast::Namespace { value: "ns".into() },
                    name: "users".into(),
                    attributes: vec![stableast::CollectionAttribute::Property(
                        stableast::Property {
                            name: "info".into(),
                            type_: stableast::Type::Object(stableast::Object {
                                fields: vec![stableast::ObjectField {
                                    name: "name".into(),
                                    type_: stableast::Type::Primitive(stableast::Primitive {
                                        value: stableast::PrimitiveType::String,
                                    }),
                                    required: false,
                                }],
                            }),
                            required: true,
                        },
                    )],
                },
                HashMap::from([("info".to_string(), Value::Map(HashMap::from([])))]),
            ),
            (
                stableast::Collection {
                    namespace: stableast::Namespace { value: "ns".into() },
                    name: "users".into(),
                    attributes: vec![stableast::CollectionAttribute::Property(
                        stableast::Property {
                            name: "info".into(),
                            type_: stableast::Type::Object(stableast::Object {
                                fields: vec![stableast::ObjectField {
                                    name: "name".into(),
                                    type_: stableast::Type::Primitive(stableast::Primitive {
                                        value: stableast::PrimitiveType::String,
                                    }),
                                    required: true,
                                }],
                            }),
                            required: false,
                        },
                    )],
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "info".into(),
                    type_: stableast::Type::Object(stableast::Object {
                        fields: vec![stableast::ObjectField {
                            name: "name".into(),
                            type_: stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::String,
                            }),
                            required: true,
                        }],
                    }),
                    required: true,
                },
            )],
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "info".into(),
                    type_: stableast::Type::Object(stableast::Object {
                        fields: vec![stableast::ObjectField {
                            name: "name".into(),
                            type_: stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::String,
                            }),
                            required: true,
                        }],
                    }),
                    required: true,
                },
            )],
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![
                stableast::CollectionAttribute::Property(stableast::Property {
                    name: "name".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::String,
                    }),
                    required: true,
                }),
                stableast::CollectionAttribute::Property(stableast::Property {
                    name: "age".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::Number,
                    }),
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![
                stableast::CollectionAttribute::Property(stableast::Property {
                    name: "name".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::String,
                    }),
                    required: true,
                }),
                stableast::CollectionAttribute::Property(stableast::Property {
                    name: "age".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::Number,
                    }),
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
                expected: stableast::Type::Primitive(stableast::Primitive {
                    value: stableast::PrimitiveType::String,
                }),
            },
        );
    }

    #[test]
    fn test_validate_set_extra_field() {
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![
                stableast::CollectionAttribute::Property(stableast::Property {
                    name: "name".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::String,
                    }),
                    required: true,
                }),
                stableast::CollectionAttribute::Property(stableast::Property {
                    name: "age".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::Number,
                    }),
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
        let collection = stableast::Collection {
            namespace: stableast::Namespace { value: "ns".into() },
            name: "users".into(),
            attributes: vec![stableast::CollectionAttribute::Property(
                stableast::Property {
                    name: "is_admin".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::Boolean,
                    }),
                    required: true,
                },
            )],
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
                expected: stableast::Type::Primitive(stableast::Primitive {
                    value: stableast::PrimitiveType::Boolean,
                }),
            })
        );
    }
}
