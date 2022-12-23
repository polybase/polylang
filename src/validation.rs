use serde::Deserialize;
use std::{collections::HashMap, ops::Deref};

use crate::ast;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum Value {
    String(String),
    Number(f64),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
}

pub(crate) fn validate_value(value: &Value, expected_type: &ast::Type) -> bool {
    let matches = match expected_type {
        ast::Type::String => {
            if let Value::String(_) = value {
                true
            } else {
                false
            }
        }
        ast::Type::Number => {
            if let Value::Number(_) = value {
                true
            } else {
                false
            }
        }
        ast::Type::Array(el) => {
            if let Value::Array(arr) = value {
                for item in arr {
                    if !validate_value(item, el.deref()) {
                        return false;
                    }
                }
                true
            } else {
                false
            }
        }
        ast::Type::Map(kt, vt) => {
            if let Value::Map(map) = value {
                for (key, value) in map {
                    match kt.deref() {
                        ast::Type::String => return true,
                        ast::Type::Number => {
                            if key.parse::<f64>().is_err() {
                                return false;
                            }
                        }
                        _ => return false,
                    }
                    if !validate_value(value, vt.deref()) {
                        return false;
                    }
                }

                true
            } else {
                false
            }
        }
    };

    matches
}

pub(crate) fn validate_set(
    collection: ast::Collection,
    data: HashMap<String, Value>,
) -> Result<(), Box<dyn std::error::Error>> {
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
            return Err(format!("Missing required field: {}", field.name).into());
        }

        if let Some(value) = data.get(&field.name) {
            if !validate_value(value, &field.type_) {
                return Err(format!(
                    "Invalid type for field {}, expected type {:?}",
                    field.name, &field.type_
                )
                .into());
            }
        }
    }

    for (key, _) in data {
        if !fields.iter().any(|item| item.name == key) {
            return Err(format!("Unexpected extraneous field: {}", key).into());
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

        assert!(validate_set(collection, data).is_ok());
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

        assert!(validate_set(collection, data).is_ok());
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

        assert!(validate_set(collection, data).is_err());
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

        assert!(validate_set(collection, data).is_ok());
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

        assert!(validate_set(collection, data).is_ok());
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

        assert!(validate_set(collection, data).is_ok());
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

        assert!(validate_set(collection, data).is_err());
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
                ("tag2".to_string(), Value::Number(2.0)),
            ])),
        )]);

        assert!(validate_set(collection, data).is_err());
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

        assert!(validate_set(collection, data).is_err());
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

        assert!(validate_set(collection, data).is_err());
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

        assert!(validate_set(collection, data).is_err());
    }
}
