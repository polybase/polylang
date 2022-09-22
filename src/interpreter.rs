// Interpreter interprets the AST and executes the program.

use crate::ast;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct Collection {
    name: String,
    fields: HashMap<String, ast::Type>,
    functions: HashMap<String, ast::Function>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Object {
    value: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum Value {
    None,
    Number(f64),
    Boolean(bool),
    String(String),
    Map(HashMap<String, Rc<RefCell<Object>>>),
    Error(Box<Rc<RefCell<Object>>>),
}

pub struct Interpreter {
    collections: HashMap<String, Collection>,
    variables: HashMap<String, Rc<RefCell<Object>>>,
    finished: bool,
    result: Rc<RefCell<Object>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            collections: HashMap::new(),
            variables: HashMap::new(),
            finished: false,
            result: Rc::new(RefCell::new(Object { value: Value::None })),
        }
    }

    pub fn load(&mut self, collection: ast::Collection) -> Result<(), Box<dyn std::error::Error>> {
        let mut fields = HashMap::new();
        let mut functions = HashMap::new();

        for item in collection.items {
            match item {
                ast::CollectionItem::Field(field) => {
                    fields.insert(field.name, field.type_);
                }
                ast::CollectionItem::Function(function) => {
                    functions.insert(function.name.clone(), function);
                }
                ast::CollectionItem::Index(_) => {}
            }
        }

        self.collections.insert(
            collection.name.clone(),
            Collection {
                name: collection.name,
                fields,
                functions,
            },
        );

        Ok(())
    }

    pub fn call(
        mut self,
        collection_name: &str,
        function_name: &str,
        variables: HashMap<String, Rc<RefCell<Object>>>,
    ) -> Result<(Object, HashMap<String, Rc<RefCell<Object>>>), Box<dyn std::error::Error>> {
        let collection = self.collections.remove(collection_name);
        if let None = collection {
            return Err(format!("Collection {} not found", collection_name).into());
        }
        let mut collection = collection.unwrap();

        let function = collection.functions.remove(function_name);
        if let None = function {
            return Err(format!("Function {} not found", function_name).into());
        }
        let function = function.unwrap();

        self.variables = variables;

        for statement in function.statements {
            self.visit_statement(&statement)?;
            if self.finished {
                return Ok((self.result.to_owned().borrow().clone(), self.variables));
            }
        }

        Ok((self.result.to_owned().borrow().clone(), self.variables))
    }

    fn visit_expression(
        &mut self,
        expression: &ast::Expression,
    ) -> Result<Rc<RefCell<Object>>, Box<dyn std::error::Error>> {
        if self.finished {
            return Ok(Rc::new(RefCell::new(Object { value: Value::None })));
        }

        match expression {
            ast::Expression::Number(number) => Ok(Rc::new(RefCell::new(Object {
                value: Value::Number(*number),
            }))),
            ast::Expression::String(string) => Ok(Rc::new(RefCell::new(Object {
                value: Value::String(string.clone()),
            }))),
            ast::Expression::Boolean(boolean) => Ok(Rc::new(RefCell::new(Object {
                value: Value::Boolean(*boolean),
            }))),
            ast::Expression::Ident(variable) => self
                .variables
                .get(variable)
                .cloned()
                .ok_or_else(|| format!("Variable {} not found", variable).into()),
            ast::Expression::Dot(left, right) => {
                let left = self.visit_expression(left)?;
                let left = left.borrow();

                match &left.value {
                    Value::Map(map) => map
                        .get(right)
                        .cloned()
                        .ok_or_else(|| format!("Field {} not found", right).into()),
                    _ => Err("Left side of dot operator must be a map".into()),
                }
            }
            ast::Expression::Assign(left, right) => {
                let right = self.visit_expression(right)?;

                if let ast::Expression::Ident(variable) = left.as_ref() {
                    self.variables.insert(variable.clone(), right.clone());
                    return Ok(right);
                }

                let left = self.visit_expression(left)?;
                left.borrow_mut().value = right.borrow().value.clone();

                Ok(left)
            }
            ast::Expression::AssignSub(left, right) => {
                let right = self.visit_expression(right)?;

                if let ast::Expression::Ident(variable) = left.as_ref() {
                    self.variables.insert(variable.clone(), right.clone());
                    return Ok(right);
                }

                let left = self.visit_expression(left)?;
                left.borrow_mut().value = match right.borrow().value {
                    Value::Number(number) => match left.borrow().value {
                        Value::Number(left_number) => Value::Number(left_number - number),
                        _ => return Err("Left side of minus operator must be a number".into()),
                    },
                    _ => return Err("Right side of minus operator must be a number".into()),
                };

                Ok(left)
            }
            ast::Expression::AssignAdd(left, right) => {
                let right = self.visit_expression(right)?;

                if let ast::Expression::Ident(variable) = left.as_ref() {
                    self.variables.insert(variable.clone(), right.clone());
                    return Ok(right);
                }

                let left = self.visit_expression(left)?;
                left.borrow_mut().value = match right.borrow().value {
                    Value::Number(number) => match left.borrow().value {
                        Value::Number(left_number) => Value::Number(left_number + number),
                        _ => return Err("Left side of add operator must be a number".into()),
                    },
                    _ => return Err("Right side of add operator must be a number".into()),
                };

                Ok(left)
            }
            ast::Expression::Equal(left, right) => {
                let left = self.visit_expression(left)?;
                let right = self.visit_expression(right)?;

                let left = left.borrow();
                let right = right.borrow();

                Ok(Rc::new(RefCell::new(Object {
                    value: Value::Boolean(&left.value == &right.value),
                })))
            }
            ast::Expression::NotEqual(left, right) => {
                let left = self.visit_expression(left)?;
                let right = self.visit_expression(right)?;

                let left = left.borrow();
                let right = right.borrow();

                Ok(Rc::new(RefCell::new(Object {
                    value: Value::Boolean(&left.value != &right.value),
                })))
            }
            ast::Expression::Add(left, right) => {
                let left = self.visit_expression(left)?;
                let right = self.visit_expression(right)?;

                let left = left.borrow();
                let right = right.borrow();

                Ok(Rc::new(RefCell::new(Object {
                    value: match (&left.value, &right.value) {
                        (Value::Number(left), Value::Number(right)) => Value::Number(left + right),
                        (Value::String(left), Value::String(right)) => {
                            Value::String(left.to_owned() + &right)
                        }
                        (x, y) => Err(format!("{:?} + {:?} is not implemented", x, y))?,
                    },
                })))
            }
            ast::Expression::Call(f, args) => {
                let f = match &**f {
                    ast::Expression::Ident(ident) if ident == "error" => ident,
                    x => Err(format!("calling {:?} is not implemented", x))?,
                };

                let arg = self.visit_expression(&args[0])?;

                Ok(Rc::new(RefCell::new(Object {
                    value: Value::Error(Box::new(arg)),
                })))
            }
            x => Err(format!("expression {:?} is not implemented", x))?,
        }
    }

    fn visit_statement(
        &mut self,
        statement: &ast::Statement,
    ) -> Result<Rc<RefCell<Object>>, Box<dyn std::error::Error>> {
        if self.finished {
            return Ok(Rc::new(RefCell::new(Object { value: Value::None })));
        }

        match statement {
            ast::Statement::Return(expression) => {
                self.result = self.visit_expression(expression)?;
                self.finished = true;
                Ok(Rc::new(RefCell::new(Object { value: Value::None })))
            }
            ast::Statement::Expression(expression) => self.visit_expression(expression),
            ast::Statement::If(if_statement) => {
                let condition = self.visit_expression(&if_statement.condition)?;
                if let Value::Boolean(true) = condition.borrow().value {
                    for statement in &if_statement.then_statements {
                        self.visit_statement(statement)?;
                    }
                } else {
                    for statement in &if_statement.else_statements {
                        self.visit_statement(statement)?;
                    }
                }

                Ok(Rc::new(RefCell::new(Object { value: Value::None })))
            }
            ast::Statement::Throw(expression) => {
                self.result = self.visit_expression(expression)?;
                self.finished = true;
                Ok(Rc::new(RefCell::new(Object { value: Value::None })))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_function() {
        let mut interpreter = Interpreter::new();
        interpreter
            .load(ast::Collection {
                name: "Test".to_string(),
                items: vec![ast::CollectionItem::Function(ast::Function {
                    name: "get_age".to_string(),
                    parameters: vec![],
                    statements: vec![ast::Statement::If(ast::If {
                        condition: ast::Expression::Equal(
                            Box::new(ast::Expression::Number(1.0)),
                            Box::new(ast::Expression::Number(1.0)),
                        ),
                        then_statements: vec![ast::Statement::Return(ast::Expression::Number(
                            42.0,
                        ))],
                        else_statements: vec![],
                    })],
                    statements_code: String::new(),
                })],
            })
            .unwrap();

        let result = interpreter
            .call("Test", "get_age", HashMap::from([]))
            .unwrap();
        assert_eq!(result.0.value, Value::Number(42.0));
    }

    #[test]
    fn test_call_function_with_parameters() {
        let mut interpreter = Interpreter::new();
        interpreter
            .load(ast::Collection {
                name: "Test".to_string(),
                items: vec![ast::CollectionItem::Function(ast::Function {
                    name: "get_age".to_string(),
                    parameters: vec![ast::Parameter {
                        name: "age".to_string(),
                        type_: ast::ParameterType::Number,
                    }],
                    statements: vec![ast::Statement::Return(ast::Expression::Ident(
                        "age".to_string(),
                    ))],
                    statements_code: String::new(),
                })],
            })
            .unwrap();

        let result = interpreter
            .call(
                "Test",
                "get_age",
                HashMap::from([(
                    "age".to_owned(),
                    Rc::new(RefCell::new(Object {
                        value: Value::Number(42.0),
                    })),
                )]),
            )
            .unwrap();

        assert_eq!(result.0.value, Value::Number(42.0));
    }

    #[test]
    fn test_throw() {
        let mut interpreter = Interpreter::new();
        interpreter
            .load(ast::Collection {
                name: "Test".to_string(),
                items: vec![ast::CollectionItem::Function(ast::Function {
                    name: "get_age".to_string(),
                    parameters: vec![],
                    statements: vec![ast::Statement::Throw(ast::Expression::Call(
                        Box::new(ast::Expression::Ident("error".to_string())),
                        vec![ast::Expression::String("Something went wrong".to_string())],
                    ))],
                    statements_code: String::new(),
                })],
            })
            .unwrap();

        let result = interpreter
            .call("Test", "get_age", HashMap::from([]))
            .unwrap();
        assert!(
            matches!(result.0.value, Value::Error(o) if o.borrow().value == Value::String("Something went wrong".to_string()))
        );
    }

    #[test]
    fn test_transfer() {
        /*
            collection Account {
               name: string;
               age: number!;
               balance: number;
               publicKey: string;

               @index([field, asc], field2);

               function transfer (a, b, amount) {
                   if (a.publicKey != auth.publicKey) throw error('invalid user');

                   a.balance -= amount;
                   b.balance += amount;
               }
           }
        */
        let mut interpreter = Interpreter::new();
        interpreter
            .load(ast::Collection {
                name: "Account".to_string(),
                items: vec![
                    ast::CollectionItem::Field(ast::Field {
                        name: "name".to_string(),
                        type_: ast::Type::String,
                        required: false,
                    }),
                    ast::CollectionItem::Field(ast::Field {
                        name: "age".to_string(),
                        type_: ast::Type::Number,
                        required: true,
                    }),
                    ast::CollectionItem::Field(ast::Field {
                        name: "balance".to_string(),
                        type_: ast::Type::Number,
                        required: false,
                    }),
                    ast::CollectionItem::Field(ast::Field {
                        name: "publicKey".to_string(),
                        type_: ast::Type::String,
                        required: false,
                    }),
                    ast::CollectionItem::Index(ast::Index {
                        unique: false,
                        fields: vec![
                            ast::IndexField {
                                name: "field".to_owned(),
                                order: ast::Order::Asc,
                            },
                            ast::IndexField {
                                name: "field2".to_owned(),
                                order: ast::Order::Asc,
                            },
                        ],
                    }),
                    ast::CollectionItem::Function(ast::Function {
                        name: "transfer".to_string(),
                        parameters: vec![
                            ast::Parameter {
                                name: "a".to_string(),
                                type_: ast::ParameterType::Record,
                            },
                            ast::Parameter {
                                name: "b".to_string(),
                                type_: ast::ParameterType::Record,
                            },
                            ast::Parameter {
                                name: "amount".to_string(),
                                type_: ast::ParameterType::Number,
                            },
                        ],
                        statements: vec![
                            ast::Statement::If(ast::If {
                                condition: ast::Expression::NotEqual(
                                    Box::new(ast::Expression::Dot(
                                        Box::new(ast::Expression::Ident("a".to_string())),
                                        "publicKey".to_string(),
                                    )),
                                    Box::new(ast::Expression::Dot(
                                        Box::new(ast::Expression::Ident("auth".to_string())),
                                        "publicKey".to_string(),
                                    )),
                                ),
                                then_statements: vec![ast::Statement::Throw(
                                    ast::Expression::Call(
                                        Box::new(ast::Expression::Ident("error".to_string())),
                                        vec![ast::Expression::String("invalid user".to_string())],
                                    ),
                                )],
                                else_statements: vec![],
                            }),
                            ast::Statement::Expression(ast::Expression::AssignSub(
                                Box::new(ast::Expression::Dot(
                                    Box::new(ast::Expression::Ident("a".to_string())),
                                    "balance".to_string(),
                                )),
                                Box::new(ast::Expression::Ident("amount".to_string())),
                            )),
                            ast::Statement::Expression(ast::Expression::AssignAdd(
                                Box::new(ast::Expression::Dot(
                                    Box::new(ast::Expression::Ident("b".to_string())),
                                    "balance".to_string(),
                                )),
                                Box::new(ast::Expression::Ident("amount".to_string())),
                            )),
                        ],
                        statements_code: String::new(),
                    }),
                ],
            })
            .unwrap();

        let result = interpreter
            .call(
                "Account",
                "transfer",
                HashMap::from([
                    (
                        "auth".to_owned(),
                        Rc::new(RefCell::new(Object {
                            value: Value::Map(HashMap::from([(
                                "publicKey".to_owned(),
                                Rc::new(RefCell::new(Object {
                                    value: Value::String("123".to_string()),
                                })),
                            )])),
                        })),
                    ),
                    (
                        "a".to_owned(),
                        Rc::new(RefCell::new(Object {
                            value: Value::Map(HashMap::from([
                                (
                                    "name".to_string(),
                                    Rc::new(RefCell::new(Object {
                                        value: Value::String("John".to_string()),
                                    })),
                                ),
                                (
                                    "age".to_string(),
                                    Rc::new(RefCell::new(Object {
                                        value: Value::Number(42.0),
                                    })),
                                ),
                                (
                                    "balance".to_string(),
                                    Rc::new(RefCell::new(Object {
                                        value: Value::Number(100.0),
                                    })),
                                ),
                                (
                                    "publicKey".to_string(),
                                    Rc::new(RefCell::new(Object {
                                        value: Value::String("123".to_string()),
                                    })),
                                ),
                            ])),
                        })),
                    ),
                    (
                        "b".to_owned(),
                        Rc::new(RefCell::new(Object {
                            value: Value::Map(HashMap::from([
                                (
                                    "name".to_string(),
                                    Rc::new(RefCell::new(Object {
                                        value: Value::String("Jane".to_string()),
                                    })),
                                ),
                                (
                                    "age".to_string(),
                                    Rc::new(RefCell::new(Object {
                                        value: Value::Number(42.0),
                                    })),
                                ),
                                (
                                    "balance".to_string(),
                                    Rc::new(RefCell::new(Object {
                                        value: Value::Number(100.0),
                                    })),
                                ),
                                (
                                    "publicKey".to_string(),
                                    Rc::new(RefCell::new(Object {
                                        value: Value::String("456".to_string()),
                                    })),
                                ),
                            ])),
                        })),
                    ),
                    (
                        "amount".to_owned(),
                        Rc::new(RefCell::new(Object {
                            value: Value::Number(10.0),
                        })),
                    ),
                ]),
            )
            .unwrap();

        assert_eq!(result.0, Object { value: Value::None });
    }
}
