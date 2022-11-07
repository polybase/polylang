mod bindings;
pub mod compiler;
mod js;
mod validation;

use polylang_parser::{ast, polylang, ParseError};
use serde::Serialize;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(Debug, Serialize)]
pub struct Error {
    pub message: String,
}

fn parse_error_to_error<T, E>(input: &str, error: ParseError<usize, T, E>) -> Error
where
    T: std::fmt::Display + std::fmt::Debug,
    E: std::fmt::Display + std::fmt::Debug,
{
    let get_line_start = |start_byte| input[..start_byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let get_line_end = |end_byte| {
        input[end_byte..]
            .find('\n')
            .map(|i| i + end_byte)
            .unwrap_or_else(|| input.len())
    };

    let make_err = |start_byte, end_byte, message: &str| {
        let line_start = get_line_start(start_byte);
        let line_end = get_line_end(end_byte);
        let line = &input[line_start..line_end];
        let line_number = input[..start_byte].matches('\n').count() + 1;
        let column = start_byte - line_start;
        let mut message = format!(
            "Error found at line {}, column {}: {}\n",
            line_number, column, message
        );

        // deindent the line
        let line_len_before_trim = line.len();
        let line = line.trim_start();
        let column = column - (line_len_before_trim - line.len());

        message.push_str(line);
        message.push_str("\n");
        message.push_str(&" ".repeat(column));
        message.push_str(&"^".repeat(if start_byte == end_byte {
            1
        } else {
            end_byte - start_byte
        }));
        Error { message }
    };

    match error {
        ParseError::InvalidToken { location } => make_err(location, location, "Invalid token"),
        ParseError::UnrecognizedEOF {
            location,
            expected: _,
        } => make_err(location, location, "Unexpected end of file"),
        ParseError::UnrecognizedToken {
            token: (start_byte, token, end_byte),
            expected,
        } => make_err(
            start_byte,
            end_byte,
            &format!(
                "Unrecognized token \"{}\". Expected one of: {}",
                token,
                expected.join(", "),
            ),
        ),
        ParseError::ExtraToken {
            token: (start_byte, token, end_byte),
        } => make_err(start_byte, end_byte, &format!("Extra token \"{}\"", token)),
        ParseError::User { error } => Error {
            message: format!("{:?}", error),
        },
    }
}

pub fn parse(input: &str) -> Result<ast::Program, Error> {
    polylang::ProgramParser::new()
        .parse(input)
        .map_err(|e| parse_error_to_error(input, e))
}

fn parse_out_json(input: &str) -> String {
    serde_json::to_string(&parse(input)).unwrap()
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

fn generate_collection_function(collection_ast: &str) -> Result<js::JSCollection, Error> {
    let collection_ast: ast::Collection =
        serde_json::from_str(collection_ast).map_err(|e| Error {
            message: e.to_string(),
        })?;

    Ok(js::generate_js_collection(&collection_ast))
}

fn generate_js_collection_out_json(collection_ast: &str) -> String {
    serde_json::to_string(&generate_collection_function(collection_ast)).unwrap()
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
        let program = polylang::ProgramParser::new().parse("collection Test {}");

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Collection(ast::Collection { name, items }) if name == "Test" && items.len() == 0)
        );
    }

    #[test]
    fn test_collection_with_fields() {
        let program = polylang::ProgramParser::new().parse(
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
            matches!(&collection.items[0], ast::CollectionItem::Field(ast::Field { name, type_, required: true }) if name == "name" && *type_ == ast::Type::String)
        );
        assert!(
            matches!(&collection.items[1], ast::CollectionItem::Field(ast::Field { name, type_, required: true }) if name == "age" && *type_ == ast::Type::Number)
        );
    }

    #[test]
    fn test_collection_with_asc_desc_fields() {
        let program = polylang::ProgramParser::new().parse(
            "
            collection Test {
                asc: string;
                desc: string;
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
            matches!(&collection.items[0], ast::CollectionItem::Field(ast::Field { name, type_, required: true }) if name == "asc" && *type_ == ast::Type::String),
        );
        assert!(
            matches!(&collection.items[1], ast::CollectionItem::Field(ast::Field { name, type_, required: true }) if name == "desc" && *type_ == ast::Type::String),
        );
    }

    #[test]
    fn test_collection_with_functions() {
        let program = polylang::ProgramParser::new().parse(
            "
            collection Test {
                function get_age(a: number, b?: string) {
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
            matches!(&collection.items[0], ast::CollectionItem::Function(ast::Function { name, parameters, statements, statements_code, return_type }) if name == "get_age" && parameters.len() == 2 && statements.len() == 1 && statements_code == "return 42;" && return_type.is_none())
        );

        let function = match &collection.items[0] {
            ast::CollectionItem::Function(function) => function,
            _ => panic!("Expected function"),
        };

        assert!(
            matches!(function.statements[0], ast::Statement::Return(ast::Expression::Primitive(ast::Primitive::Number(number))) if number == 42.0)
        );
        assert!(
            matches!(&function.parameters[0], ast::Parameter{ name, type_, required } if *required == true && name == "a" && *type_ == ast::ParameterType::Number)
        );
        assert!(
            matches!(&function.parameters[1], ast::Parameter{ name, type_, required } if *required == false && name == "b" && *type_ == ast::ParameterType::String)
        );
    }

    #[test]
    fn test_number() {
        let number = polylang::NumberParser::new().parse("42");

        assert!(number.is_ok());
        assert_eq!(number.unwrap(), 42.0);
    }

    #[test]
    fn test_string() {
        let string = polylang::StringParser::new().parse("'hello world'");

        assert!(string.is_ok());
        assert_eq!(string.unwrap(), "hello world");
    }

    #[test]
    fn test_comparison() {
        let comparison = polylang::ExpressionParser::new().parse("1 > 2");

        assert!(matches!(
            comparison.unwrap(),
            ast::Expression::GreaterThan(left, right) if *left == ast::Expression::Primitive(ast::Primitive::Number(1.0))
                && *right == ast::Expression::Primitive(ast::Primitive::Number(2.0)),
        ));
    }

    #[test]
    fn test_if() {
        let if_ = polylang::IfParser::new().parse(
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
            matches!(if_.condition, ast::Expression::Equal(n, m) if *n == ast::Expression::Primitive(ast::Primitive::Number(1.0)) && *m == ast::Expression::Primitive(ast::Primitive::Number(1.0)))
        );
        assert_eq!(if_.then_statements.len(), 1);
        assert_eq!(if_.else_statements.len(), 1);
    }

    #[test]
    fn test_call() {
        let call = polylang::ExpressionParser::new().parse("get_age(a, b, c)");

        assert!(matches!(
            call.unwrap(),
            ast::Expression::Call(f, args) if *f == ast::Expression::Ident("get_age".to_owned()) && args.len() == 3
        ));
    }

    #[test]
    fn test_dot() {
        let dot = polylang::ExpressionParser::new().parse("a.b").unwrap();

        assert!(matches!(
            dot,
            ast::Expression::Dot(left, right) if *left == ast::Expression::Ident("a".to_owned()) && right == "b".to_owned()
        ));
    }

    #[test]
    fn test_assign_sub() {
        let dot = polylang::ExpressionParser::new().parse("a -= b").unwrap();

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
                age?: number;
                balance: number;
                publicKey: string;
            
                @index([field, asc], field2);
            
                transfer (b: record, amount: number) {
                    if (this.publicKey != $auth.publicKey) throw error('invalid user');
                    
                    this.balance -= amount;
                    b.balance += amount;
                }
            }
        ";

        let collection = polylang::CollectionParser::new().parse(code).unwrap();
        assert_eq!(collection.name, "Account");
        assert_eq!(collection.items.len(), 6);

        assert!(matches!(
            &collection.items[0],
            ast::CollectionItem::Field(ast::Field { name, type_, required: true })
            if name == "name" && *type_ == ast::Type::String
        ));

        assert!(matches!(
            &collection.items[1],
            ast::CollectionItem::Field(ast::Field { name, type_, required: false })
            if name == "age" && *type_ == ast::Type::Number
        ));

        assert!(matches!(
            &collection.items[2],
            ast::CollectionItem::Field(ast::Field { name, type_, required: true })
            if name == "balance" && *type_ == ast::Type::Number
        ));

        assert!(matches!(
            &collection.items[3],
            ast::CollectionItem::Field(ast::Field { name, type_, required: true })
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
        dbg!(&function.statements);

        assert!(matches!(
            &function.statements[0],
            ast::Statement::If(ast::If {
                condition,
                then_statements,
                else_statements,
            }) if *condition == ast::Expression::NotEqual(
                Box::new(ast::Expression::Dot(
                    Box::new(ast::Expression::Ident("this".to_owned())),
                    "publicKey".to_owned(),
                )),
                Box::new(ast::Expression::Dot(
                    Box::new(ast::Expression::Ident("$auth".to_owned())),
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
                Box::new(ast::Expression::Ident("this".to_owned())),
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

    //     #[test]
    //     fn test_generate_js_function() {
    //         let func_code = "
    //             function transfer (a: record, b: record, amount: number) {
    //                 if (a.publicKey != $auth.publicKey) throw error('invalid user');

    //                 a.balance -= amount;
    //                 b.balance += amount;
    //             }
    //         ";

    //         let func = polylang::FunctionParser::new().parse(func_code).unwrap();
    //         let func_ast = serde_json::to_string(&func).unwrap();

    //         let eval_input = generate_js_function(&func_ast).unwrap();
    //         assert_eq!(
    //             eval_input,
    //             JSFunc {
    //                 code: "
    // function error(str) {
    //     return new Error(str);
    // }

    // const f = ($auth, args) => {
    // const a = args[0], b = args[1], amount = args[2];
    // if (a.publicKey != $auth.publicKey) throw error('invalid user');

    //                 a.balance -= amount;
    //                 b.balance += amount;
    // };
    // "
    //                 .to_string(),
    //             },
    //         );
    //     }

    #[test]
    fn test_error_unrecognized_token() {
        let code = "
            collection test-cities {}
        ";

        let collection = parse(code);
        assert!(collection.is_err());
        eprintln!("{}", collection.as_ref().unwrap_err().message);
        assert_eq!(
            collection.unwrap_err().message,
            r#"Error found at line 2, column 27: Unrecognized token "-". Expected one of: "{"
collection test-cities {}
               ^"#,
        );
    }

    #[test]
    fn test_error_invalid_token() {
        let code = "
            collection ą {}
        ";

        let collection = parse(code);
        assert!(collection.is_err());
        eprintln!("{}", collection.as_ref().unwrap_err().message);
        assert_eq!(
            collection.unwrap_err().message,
            r#"Error found at line 2, column 23: Invalid token
collection ą {}
           ^"#,
        );
    }

    #[test]
    fn test_error_unexpected_eof() {
        let code = "
            function x() {
        ";

        let collection = parse(code);
        assert!(collection.is_err());
        eprintln!("{}", collection.as_ref().unwrap_err().message);
        assert_eq!(
            collection.unwrap_err().message,
            r#"Error found at line 2, column 26: Unexpected end of file
function x() {
              ^"#,
        );
    }

    #[test]
    fn test_error_field_invalid_type() {
        let code = "
            collection test {
                name: object;
            }
        ";

        let collection = parse(code);
        assert!(collection.is_err());
        eprintln!("{}", collection.as_ref().unwrap_err().message);
        assert_eq!(
            collection.unwrap_err().message,
            r#"Error found at line 3, column 22: Unrecognized token "object". Expected one of: "number", "string"
name: object;
      ^^^^^^"#,
        );
    }
}
