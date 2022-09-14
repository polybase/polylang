mod ast;
mod bindings;
mod interpreter;
mod validation;

use serde::Serialize;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub spacetime);

#[derive(Debug, Serialize)]
struct Error {
    message: String,
}

fn parse(input: &str) -> Result<ast::Program, Error> {
    spacetime::ProgramParser::new()
        .parse(input)
        .map_err(|e| Error {
            message: e.to_string(),
        })
}

fn parse_out_json(input: &str) -> String {
    serde_json::to_string(&parse(input)).unwrap()
}

fn interpret(
    program: &str,
    collection_name: &str,
    func: &str,
    args: HashMap<String, Rc<RefCell<interpreter::Object>>>,
) -> Result<
    (
        interpreter::Object,
        HashMap<String, Rc<RefCell<interpreter::Object>>>,
    ),
    Error,
> {
    let program = spacetime::ProgramParser::new().parse(program);
    if let Err(err) = program {
        return Err(Error {
            message: err.to_string(),
        });
    }
    let program = program.unwrap();

    let mut interpreter = interpreter::Interpreter::new();

    let collection = program
        .nodes
        .into_iter()
        .find_map(|item| {
            if let ast::RootNode::Collection(c) = item {
                if c.name == collection_name {
                    Some(c)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .ok_or(Error {
            message: "collection not found".to_string(),
        })?;

    interpreter.load(collection).map_err(|e| Error {
        message: e.to_string(),
    })?;

    let (result, vars) = interpreter
        .call(collection_name, func, args)
        .map_err(|e| Error {
            message: e.to_string(),
        })?;

    Ok((result, vars))
}

fn interpret_out_json(
    program: &str,
    collection_name: &str,
    func: &str,
    args: HashMap<String, Rc<RefCell<interpreter::Object>>>,
) -> String {
    serde_json::to_string(&interpret(program, collection_name, func, args)).unwrap()
}

fn validate_set(collection_ast_json: &str, data_json: &str) -> Result<(), Error> {
    let collection_ast: ast::Collection = match serde_json::from_str(collection_ast_json) {
        Ok(ast) => ast,
        Err(err) => {
            return Err(Error {
                message: err.to_string(),
            })
        }
    };

    let data: HashMap<String, validation::Value> = match serde_json::from_str(data_json) {
        Ok(data) => data,
        Err(err) => {
            return Err(Error {
                message: err.to_string(),
            })
        }
    };

    validation::validate_set(collection_ast, data).map_err(|e| Error {
        message: e.to_string(),
    })
}

fn validate_set_out_json(collection_ast_json: &str, data_json: &str) -> String {
    serde_json::to_string(&validate_set(collection_ast_json, data_json)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let input = "collection Test {}";
        let expected_output = r#"{"Ok":{"nodes":[{"Collection":{"name":"Test","items":[]}}]}}"#;

        let output = parse_out_json(input);
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_collection() {
        let program = spacetime::ProgramParser::new().parse("collection Test {}");

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Collection(ast::Collection { name, items }) if name == "Test" && items.len() == 0)
        );
    }

    #[test]
    fn test_collection_with_fields() {
        let program = spacetime::ProgramParser::new().parse(
            "
            collection Test {
                name: string;
                age: number;
            }
            ",
        );

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Collection(ast::Collection { name, items }) if name == "Test" && items.len() == 2)
        );

        let collection = match &program.nodes[0] {
            ast::RootNode::Collection(collection) => collection,
            _ => panic!("Expected collection"),
        };

        assert!(
            matches!(&collection.items[0], ast::CollectionItem::Field(ast::Field { name, type_, required: false }) if name == "name" && *type_ == ast::Type::String)
        );
    }

    #[test]
    fn test_collection_with_functions() {
        let program = spacetime::ProgramParser::new().parse(
            "
            collection Test {
                function get_age() {
                    return 42;
                }
            }
            ",
        );

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Collection(ast::Collection { name, items }) if name == "Test" && items.len() == 1)
        );

        let collection = match &program.nodes[0] {
            ast::RootNode::Collection(collection) => collection,
            _ => panic!("Expected collection"),
        };

        assert!(
            matches!(&collection.items[0], ast::CollectionItem::Function(ast::Function { name, parameters, statements }) if name == "get_age" && parameters.len() == 0 && statements.len() == 1)
        );

        let function = match &collection.items[0] {
            ast::CollectionItem::Function(function) => function,
            _ => panic!("Expected function"),
        };

        assert!(
            matches!(function.statements[0], ast::Statement::Return(ast::Expression::Number(number)) if number == 42.0)
        );
    }

    #[test]
    fn test_number() {
        let number = spacetime::NumberParser::new().parse("42");

        assert!(number.is_ok());
        assert_eq!(number.unwrap(), 42.0);
    }

    #[test]
    fn test_string() {
        let string = spacetime::StringParser::new().parse("'hello world'");

        assert!(string.is_ok());
        assert_eq!(string.unwrap(), "hello world");
    }

    #[test]
    fn test_comparison() {
        let comparison = spacetime::ExpressionParser::new().parse("1 > 2");

        assert!(matches!(
            comparison.unwrap(),
            ast::Expression::GreaterThan(left, right) if *left == ast::Expression::Number(1.0)
                && *right == ast::Expression::Number(2.0)
        ));
    }

    #[test]
    fn test_if() {
        let if_ = spacetime::IfParser::new().parse(
            "
            if (1 == 1) {
                return 42;
            } else {
                return 0;
            }
            ",
        );

        let if_ = if_.unwrap();
        assert!(
            matches!(if_.condition, ast::Expression::Equal(n, m) if *n == ast::Expression::Number(1.0) && *m == ast::Expression::Number(1.0))
        );
        assert_eq!(if_.then_statements.len(), 1);
        assert_eq!(if_.else_statements.len(), 1);
    }

    #[test]
    fn test_call() {
        let call = spacetime::ExpressionParser::new().parse("get_age(a, b, c)");

        assert!(matches!(
            call.unwrap(),
            ast::Expression::Call(f, args) if *f == ast::Expression::Ident("get_age".to_owned()) && args.len() == 3
        ));
    }

    #[test]
    fn test_dot() {
        let dot = spacetime::ExpressionParser::new().parse("a.b").unwrap();

        assert!(matches!(
            dot,
            ast::Expression::Dot(left, right) if *left == ast::Expression::Ident("a".to_owned()) && right == "b".to_owned()
        ));
    }

    #[test]
    fn test_assign_sub() {
        let dot = spacetime::ExpressionParser::new().parse("a -= b").unwrap();

        assert!(matches!(
            dot,
            ast::Expression::AssignSub(left, right) if *left == ast::Expression::Ident("a".to_owned()) && *right == ast::Expression::Ident("b".to_owned())
        ));
    }

    #[test]
    fn test_code_from_issue() {
        let code = "
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
        ";

        let collection = spacetime::CollectionParser::new().parse(code).unwrap();
        assert_eq!(collection.name, "Account");
        assert_eq!(collection.items.len(), 6);

        assert!(matches!(
            &collection.items[0],
            ast::CollectionItem::Field(ast::Field { name, type_, required: false })
            if name == "name" && *type_ == ast::Type::String
        ));

        assert!(matches!(
            &collection.items[1],
            ast::CollectionItem::Field(ast::Field { name, type_, required: true })
            if name == "age" && *type_ == ast::Type::Number
        ));

        assert!(matches!(
            &collection.items[2],
            ast::CollectionItem::Field(ast::Field { name, type_, required: false })
            if name == "balance" && *type_ == ast::Type::Number
        ));

        assert!(matches!(
            &collection.items[3],
            ast::CollectionItem::Field(ast::Field { name, type_, required: false })
            if name == "publicKey" && *type_ == ast::Type::String
        ));

        assert!(matches!(
            &collection.items[4],
            ast::CollectionItem::Index(ast::Index {
                unique,
                fields,
            }) if !unique && fields[0].name == "field" && fields[0].order == ast::Order::Asc
                && fields[1].name == "field2" && fields[1].order == ast::Order::Asc
        ));

        let function = match &collection.items[5] {
            ast::CollectionItem::Function(f) => f,
            _ => panic!("expected function"),
        };

        assert!(matches!(
            &function.statements[0],
            ast::Statement::If(ast::If {
                condition,
                then_statements,
                else_statements,
            }) if *condition == ast::Expression::NotEqual(
                Box::new(ast::Expression::Dot(
                    Box::new(ast::Expression::Ident("a".to_owned())),
                    "publicKey".to_owned(),
                )),
                Box::new(ast::Expression::Dot(
                    Box::new(ast::Expression::Ident("auth".to_owned())),
                    "publicKey".to_owned(),
                )),
            ) && then_statements.len() == 1 && else_statements.len() == 0
        ));

        assert!(matches!(
            &function.statements[1],
            ast::Statement::Expression(ast::Expression::AssignSub(
                left,
                right,
            )) if **left == ast::Expression::Dot(
                Box::new(ast::Expression::Ident("a".to_owned())),
                "balance".to_owned(),
            ) && **right == ast::Expression::Ident("amount".to_owned())
        ));

        assert!(matches!(
            &function.statements[2],
            ast::Statement::Expression(ast::Expression::AssignAdd(
                left,
                right,
            )) if **left == ast::Expression::Dot(
                Box::new(ast::Expression::Ident("b".to_owned())),
                "balance".to_owned(),
            ) && **right == ast::Expression::Ident("amount".to_owned())
        ));
    }
}
