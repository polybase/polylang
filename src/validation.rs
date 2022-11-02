use serde::Deserialize;
use std::{collections::HashMap};

use crate::ast;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum Value {
    String(String),
    Number(f64),
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
            match value {
                Value::String(_) => {
                    if field.type_ != ast::Type::String {
                        return Err(format!("Invalid type for field: {}", field.name).into());
                    }
                }
                Value::Number(_) => {
                    if field.type_ != ast::Type::Number {
                        return Err(format!("Invalid type for field: {}", field.name).into());
                    }
                }
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
