use regex::Regex;
use serde::Deserialize;
use std::{cmp::Ordering, collections::HashMap};

use crate::ast;

const RESERVED_FUNCTIONS: [&str; 5] = ["min", "max", "readonly", "creator", "regex"];

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

pub(crate) fn validate_set_decorators(
    program: ast::Program,
    collection_name: &str,
    data: HashMap<&str, Value>,
    previous_data: HashMap<&str, Value>,
    public_key: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let collection = program
        .nodes
        .iter()
        .filter_map(|node| {
            if let ast::RootNode::Collection(collection) = node {
                Some(collection)
            } else {
                None
            }
        })
        .find(|collection| collection.name == collection_name)
        .ok_or(format!("Collection not found: {}", collection_name))?;

    let functions = collection.items.iter().filter_map(|item| {
        if let ast::CollectionItem::Function(function) = item {
            Some(&function.name)
        } else {
            None
        }
    });

    let collection_functions = collection.items.iter().filter_map(|item| {
        if let ast::CollectionItem::Function(function) = item {
            Some(&function.name)
        } else {
            None
        }
    });

    for function in functions.chain(collection_functions) {
        if RESERVED_FUNCTIONS.contains(&function.as_str()) {
            return Err(format!("Function name is reserved: {}", function).into());
        }
    }

    let fields = collection.items.iter().filter_map(|item| {
        if let ast::CollectionItem::Field(field) = item {
            Some(field)
        } else {
            None
        }
    });

    for field in fields {
        for decorator in &field.decorators {
            let validation_fn = match decorator.name.as_str() {
                "min" => min(decorator.arguments.as_slice())?,
                "max" => max(decorator.arguments.as_slice())?,
                "readonly" => readonly(decorator.arguments.as_slice())?,
                "creator" => creator(decorator.arguments.as_slice())?,
                "regex" => regex(decorator.arguments.as_slice())?,
                n => return Err(format!("Unknown decorator: {}", n).into()),
            };

            if let Err(err) = validation_fn(ValidationArgs {
                previous: previous_data.get(field.name.as_str()),
                new: data.get(field.name.as_str()),
                public_key,
            }) {
                return Err(format!(
                    "Invalid field: {}, decorator: {}, error: {}",
                    field.name,
                    decorator.name.as_str(),
                    err,
                )
                .into());
            }
        }
    }

    Ok(())
}

struct ValidationArgs<'a> {
    previous: Option<&'a Value>,
    new: Option<&'a Value>,
    public_key: Option<&'a str>,
}

type ValidationFn = Box<dyn Fn(ValidationArgs) -> Result<(), String>>;

fn min(args: &[ast::Primitive]) -> Result<ValidationFn, Box<dyn std::error::Error>> {
    if args.len() != 1 {
        return Err("min requires 1 argument".into());
    }

    let min_value = match args[0] {
        ast::Primitive::Number(min_value) => min_value,
        _ => return Err("Invalid type for min decorator".into()),
    };

    Ok(Box::new(move |vargs| match vargs.new {
        Some(Value::Number(v)) => match v.partial_cmp(&min_value) {
            None => Err("Invalid value".into()),
            Some(Ordering::Less) => Err(format!("Value is less than min: {}", min_value).into()),
            Some(_) => Ok(()),
        },
        Some(Value::String(s)) => match s.len().cmp(&(min_value.ceil() as usize)) {
            Ordering::Less => Err(format!("String is shorter than min: {}", min_value).into()),
            _ => Ok(()),
        },
        None => Ok(()),
    }))
}

fn max(args: &[ast::Primitive]) -> Result<ValidationFn, Box<dyn std::error::Error>> {
    if args.len() != 1 {
        return Err("max requires 1 argument".into());
    }

    let max_value = match args[0] {
        ast::Primitive::Number(max_value) => max_value,
        _ => return Err("Invalid type for max decorator".into()),
    };

    Ok(Box::new(move |vargs| match vargs.new {
        Some(Value::Number(v)) => match v.partial_cmp(&max_value) {
            None => Err("Invalid value".into()),
            Some(Ordering::Greater) => {
                Err(format!("Value is greater than max: {}", max_value).into())
            }
            Some(_) => Ok(()),
        },
        Some(Value::String(s)) => match s.len().cmp(&(max_value.ceil() as usize)) {
            Ordering::Greater => Err(format!("String is longer than max: {}", max_value).into()),
            _ => Ok(()),
        },
        None => Ok(()),
    }))
}

fn readonly(args: &[ast::Primitive]) -> Result<ValidationFn, Box<dyn std::error::Error>> {
    if args.len() != 0 {
        return Err("readonly does not take any arguments".into());
    }

    Ok(Box::new(move |vargs| match (vargs.previous, vargs.new) {
        (None, None) => Ok(()),
        (None, Some(_)) => Ok(()),
        (Some(_), None) => Err("Cannot delete a readonly field".into()),
        (Some(prev), Some(new)) if prev == new => Ok(()),
        (Some(_), Some(_)) => Err("Cannot update a readonly field".into()),
    }))
}

fn creator(args: &[ast::Primitive]) -> Result<ValidationFn, Box<dyn std::error::Error>> {
    if args.len() != 0 {
        return Err("creator does not take any arguments".into());
    }

    Ok(Box::new(move |vargs| {
        let public_key = if let Some(public_key) = vargs.public_key {
            public_key
        } else {
            return Err("Missing public key from auth".into());
        };

        match vargs.new {
            Some(Value::String(s)) => {
                if s == public_key {
                    Ok(())
                } else {
                    Err("Creator does not match public key from auth".into())
                }
            }
            _ => Err("Creator must be a string".into()),
        }
    }))
}

fn regex(args: &[ast::Primitive]) -> Result<ValidationFn, Box<dyn std::error::Error>> {
    if args.len() != 1 {
        return Err("regex requires 1 argument".into());
    }

    let regex = match args[0] {
        ast::Primitive::Regex(ref regex) => regex,
        _ => return Err("Invalid type for regex decorator".into()),
    };

    let regex = Regex::new(regex)?;

    Ok(Box::new(move |vargs| match vargs.new {
        Some(Value::String(s)) => {
            if regex.is_match(s) {
                Ok(())
            } else {
                Err(format!("String does not match regex: {}", regex).into())
            }
        }
        _ => Err("Regex can only be applied to strings".into()),
    }))
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
                    decorators: Vec::new(),
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
                    decorators: Vec::new(),
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
                    decorators: Vec::new(),
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
                    decorators: Vec::new(),
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
                    decorators: Vec::new(),
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
                    decorators: Vec::new(),
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
                    decorators: Vec::new(),
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
                    decorators: Vec::new(),
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

    #[test]
    fn test_validate_set_decorators() {
        let collection = || ast::Collection {
            name: "users".to_string(),
            items: vec![
                ast::CollectionItem::Field(ast::Field {
                    name: "name".to_string(),
                    type_: ast::Type::String,
                    required: true,
                    decorators: Vec::new(),
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "age".to_string(),
                    type_: ast::Type::Number,
                    required: false,
                    decorators: vec![ast::FieldDecorator {
                        name: "min".to_string(),
                        arguments: vec![ast::Primitive::Number(18.0)],
                    }],
                }),
                ast::CollectionItem::Field(ast::Field {
                    name: "$pk".to_string(),
                    type_: ast::Type::String,
                    required: true,
                    decorators: vec![ast::FieldDecorator {
                        name: "creator".to_string(),
                        arguments: vec![],
                    }],
                }),
            ],
        };

        validate_set_decorators(
            ast::Program {
                nodes: vec![ast::RootNode::Collection(collection())],
            },
            "users",
            HashMap::from([
                ("name", Value::String("John".to_string())),
                ("age", Value::Number(30.0)),
                ("$pk", Value::String("0x0".to_string())),
            ]),
            HashMap::new(),
            Some("0x0"),
        )
        .unwrap();

        let err = validate_set_decorators(
            ast::Program {
                nodes: vec![ast::RootNode::Collection(collection())],
            },
            "users",
            HashMap::from([
                ("name", Value::String("John".to_string())),
                ("age", Value::Number(17.0)),
                ("$pk", Value::String("0x0".to_string())),
            ]),
            HashMap::new(),
            Some("0x0"),
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "Invalid field: age, decorator: min, error: Value is less than min: 18"
        );

        // $pk doesn't match pk from auth
        let err = validate_set_decorators(
            ast::Program {
                nodes: vec![ast::RootNode::Collection(collection())],
            },
            "users",
            HashMap::from([
                ("name", Value::String("John".to_string())),
                ("age", Value::Number(30.0)),
                ("$pk", Value::String("0x1".to_string())),
            ]),
            HashMap::new(),
            Some("0x0"),
        )
        .unwrap_err();

        assert_eq!(err.to_string(), "Invalid field: $pk, decorator: creator, error: Creator does not match public key from auth");
    }

    #[test]
    fn test_readonly() {
        readonly(&[]).unwrap()(ValidationArgs {
            previous: Some(&Value::Number(123.0)),
            new: Some(&Value::Number(123.0)),
            public_key: None,
        })
        .unwrap();

        readonly(&[]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::Number(123.0)),
            public_key: None,
        })
        .unwrap();

        readonly(&[]).unwrap()(ValidationArgs {
            previous: Some(&Value::Number(123.0)),
            new: None,
            public_key: None,
        })
        .unwrap_err();

        readonly(&[]).unwrap()(ValidationArgs {
            previous: Some(&Value::Number(123.0)),
            new: Some(&Value::Number(456.0)),
            public_key: None,
        })
        .unwrap_err();
    }

    #[test]
    fn test_regex() {
        regex(&[ast::Primitive::Regex("^123$".to_string())]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("456".to_string())),
            public_key: None,
        })
        .unwrap_err();

        regex(&[ast::Primitive::Regex("^123$".to_string())]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("123".to_string())),
            public_key: None,
        })
        .unwrap();
    }

    #[test]
    fn test_min_number() {
        min(&[ast::Primitive::Number(123.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::Number(122.0)),
            public_key: None,
        })
        .unwrap_err();

        min(&[ast::Primitive::Number(123.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::Number(123.0)),
            public_key: None,
        })
        .unwrap();

        min(&[ast::Primitive::Number(123.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::Number(124.0)),
            public_key: None,
        })
        .unwrap();
    }

    #[test]
    fn test_min_string() {
        min(&[ast::Primitive::Number(1.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("".to_string())),
            public_key: None,
        })
        .unwrap_err();

        min(&[ast::Primitive::Number(1.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("a".to_string())),
            public_key: None,
        })
        .unwrap();

        min(&[ast::Primitive::Number(1.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("aa".to_string())),
            public_key: None,
        })
        .unwrap();
    }

    #[test]
    fn test_max() {
        max(&[ast::Primitive::Number(123.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::Number(122.0)),
            public_key: None,
        })
        .unwrap();

        max(&[ast::Primitive::Number(123.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::Number(123.0)),
            public_key: None,
        })
        .unwrap();

        max(&[ast::Primitive::Number(123.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::Number(124.0)),
            public_key: None,
        })
        .unwrap_err();
    }

    #[test]
    fn test_max_string() {
        max(&[ast::Primitive::Number(1.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("".to_string())),
            public_key: None,
        })
        .unwrap();

        max(&[ast::Primitive::Number(1.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("a".to_string())),
            public_key: None,
        })
        .unwrap();

        max(&[ast::Primitive::Number(1.0)]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("aa".to_string())),
            public_key: None,
        })
        .unwrap_err();
    }

    #[test]
    fn test_creator() {
        creator(&[]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("0x0".to_string())),
            public_key: Some("0x0"),
        })
        .unwrap();

        creator(&[]).unwrap()(ValidationArgs {
            previous: None,
            new: Some(&Value::String("0x1".to_string())),
            public_key: Some("0x0"),
        })
        .unwrap_err();
    }
}
