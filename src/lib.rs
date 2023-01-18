mod bindings;
mod js;
mod validation;

use polylang_parser::{ast, LexicalError, ParseError};
use serde::Serialize;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(Debug, Serialize)]
struct Error {
    message: String,
}

fn parse_error_to_error<T>(input: &str, error: ParseError<usize, T, LexicalError>) -> Error
where
    T: std::fmt::Display + std::fmt::Debug,
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
        ParseError::User { error } => match error {
            LexicalError::NumberParseError { start, end } => {
                make_err(start, end, "Failed to parse number")
            }
            LexicalError::InvalidToken { start, end } => make_err(start, end, "Invalid token"),
            LexicalError::UnterminatedComment { start, end } => {
                make_err(start, end, "Unterminated comment")
            }
            LexicalError::UnterminatedString { start, end } => {
                make_err(start, end, "Unterminated string")
            }
            LexicalError::UserError {
                start,
                end,
                message,
            } => make_err(start, end, &message),
        },
    }
}

fn parse(input: &str) -> Result<ast::Program, Error> {
    polylang_parser::parse(input).map_err(|e| parse_error_to_error(input, e))
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

    validation::validate_set(&collection_ast, &data).map_err(|e| Error {
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
        let expected_output =
            r#"{"Ok":{"nodes":[{"Collection":{"name":"Test","decorators":[],"items":[]}}]}}"#;

        let output = parse_out_json(input);
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_collection() {
        let program = parse("collection Test {}");

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Collection(ast::Collection { name, decorators, items }) if name == "Test" && decorators.is_empty() && items.is_empty())
        );
    }

    #[test]
    fn test_collection_with_fields() {
        let program = parse(
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
            matches!(&program.nodes[0], ast::RootNode::Collection(ast::Collection { name, decorators, items }) if name == "Test" && decorators.is_empty() && items.len() == 2)
        );

        let collection = match &program.nodes[0] {
            ast::RootNode::Collection(collection) => collection,
            _ => panic!("Expected collection"),
        };

        assert!(
            matches!(&collection.items[0], ast::CollectionItem::Field(ast::Field { name, type_, required: true, decorators }) if name == "name" && *type_ == ast::Type::String && decorators.is_empty())
        );
        assert!(
            matches!(&collection.items[1], ast::CollectionItem::Field(ast::Field { name, type_, required: true, decorators }) if name == "age" && *type_ == ast::Type::Number && decorators.is_empty())
        );
    }

    #[test]
    fn test_collection_with_asc_desc_fields() {
        let program = parse(
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
            matches!(&program.nodes[0], ast::RootNode::Collection(ast::Collection { name, decorators, items }) if name == "Test" && decorators.is_empty() && items.len() == 2)
        );

        let collection = match &program.nodes[0] {
            ast::RootNode::Collection(collection) => collection,
            _ => panic!("Expected collection"),
        };

        assert!(
            matches!(&collection.items[0], ast::CollectionItem::Field(ast::Field { name, type_, required: true, decorators }) if name == "asc" && *type_ == ast::Type::String && decorators.is_empty()),
        );
        assert!(
            matches!(&collection.items[1], ast::CollectionItem::Field(ast::Field { name, type_, required: true, decorators }) if name == "desc" && *type_ == ast::Type::String && decorators.is_empty()),
        );
    }

    #[test]
    fn test_collection_with_functions() {
        let program = parse(
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
            matches!(&program.nodes[0], ast::RootNode::Collection(ast::Collection { name, decorators, items }) if name == "Test" && decorators.is_empty() && items.len() == 1)
        );

        let collection = match &program.nodes[0] {
            ast::RootNode::Collection(collection) => collection,
            _ => panic!("Expected collection"),
        };

        assert!(
            matches!(&collection.items[0], ast::CollectionItem::Function(ast::Function {
                name,
                decorators,
                parameters,
                statements,
                statements_code,
                return_type,
            }) if name == "get_age" && decorators.is_empty() && parameters.len() == 2 && statements.len() == 1 && statements_code == "return 42;" && return_type == &None)
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
        let number = polylang_parser::parse_expression("42");

        assert!(number.is_ok());
        assert_eq!(
            number.unwrap(),
            ast::Expression::Primitive(ast::Primitive::Number(42.0))
        );
    }

    #[test]
    fn test_string() {
        let string = polylang_parser::parse_expression("'hello world'");

        assert!(string.is_ok());
        assert_eq!(
            string.unwrap(),
            ast::Expression::Primitive(ast::Primitive::String("hello world".to_string()))
        );
    }

    #[test]
    fn test_comparison() {
        let comparison = polylang_parser::parse_expression("1 > 2");

        assert!(matches!(
            comparison.unwrap(),
            ast::Expression::GreaterThan(left, right) if *left == ast::Expression::Primitive(ast::Primitive::Number(1.0))
                && *right == ast::Expression::Primitive(ast::Primitive::Number(2.0)),
        ));
    }

    #[test]
    fn test_if() {
        let program = parse(
            "
            function x() {
                if (1 == 1) {
                    return 42;
                } else {
                    return 0;
                }
            }
            ",
        );

        let mut program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);

        let mut function = match program.nodes.pop().unwrap() {
            ast::RootNode::Function(function) => function,
            _ => panic!("Expected function"),
        };

        assert_eq!(function.statements.len(), 1);

        let if_ = match function.statements.pop().unwrap() {
            ast::Statement::If(if_) => if_,
            _ => panic!("Expected if"),
        };

        assert!(
            matches!(if_.condition, ast::Expression::Equal(n, m) if *n == ast::Expression::Primitive(ast::Primitive::Number(1.0)) && *m == ast::Expression::Primitive(ast::Primitive::Number(1.0)))
        );
        assert_eq!(if_.then_statements.len(), 1);
        assert_eq!(if_.else_statements.len(), 1);
    }

    #[test]
    fn test_call() {
        let call = polylang_parser::parse_expression("get_age(a, b, c)");

        assert!(matches!(
            call.unwrap(),
            ast::Expression::Call(f, args) if *f == ast::Expression::Ident("get_age".to_owned()) && args.len() == 3
        ));
    }

    #[test]
    fn test_dot() {
        let dot = polylang_parser::parse_expression("a.b").unwrap();

        assert!(matches!(
            dot,
            ast::Expression::Dot(left, right) if *left == ast::Expression::Ident("a".to_owned()) && right == "b".to_owned()
        ));
    }

    #[test]
    fn test_assign_sub() {
        let dot = polylang_parser::parse_expression("a -= b").unwrap();

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

        let program = parse(code).unwrap();

        assert_eq!(program.nodes.len(), 1);

        let collection = match &program.nodes[0] {
            ast::RootNode::Collection(collection) => collection,
            _ => panic!("Expected collection"),
        };

        assert_eq!(collection.name, "Account");
        assert_eq!(collection.items.len(), 6);

        assert!(matches!(
            &collection.items[0],
            ast::CollectionItem::Field(ast::Field { name, type_, required: true, decorators })
            if name == "name" && *type_ == ast::Type::String && decorators.is_empty()
        ));

        assert!(matches!(
            &collection.items[1],
            ast::CollectionItem::Field(ast::Field { name, type_, required: false, decorators })
            if name == "age" && *type_ == ast::Type::Number && decorators.is_empty()
        ));

        assert!(matches!(
            &collection.items[2],
            ast::CollectionItem::Field(ast::Field { name, type_, required: true, decorators })
            if name == "balance" && *type_ == ast::Type::Number && decorators.is_empty()
        ));

        assert!(matches!(
            &collection.items[3],
            ast::CollectionItem::Field(ast::Field { name, type_, required: true, decorators })
            if name == "publicKey" && *type_ == ast::Type::String && decorators.is_empty()
        ));

        assert!(matches!(
            &collection.items[4],
            ast::CollectionItem::Index(ast::Index {
                fields,
            }) if fields[0].path == ["field"] && fields[0].order == ast::Order::Asc
                && fields[1].path == ["field2"] && fields[1].order == ast::Order::Asc
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
    fn test_foreign_record_field() {
        let code = "
            collection test {
                account: Account;
            }
        ";

        let program = parse(code).unwrap();

        let collection = match &program.nodes[0] {
            ast::RootNode::Collection(c) => c,
            _ => panic!("expected collection"),
        };

        assert_eq!(collection.items.len(), 1);

        let field = match &collection.items[0] {
            ast::CollectionItem::Field(f) => f,
            _ => panic!("expected field"),
        };

        assert_eq!(field.name, "account");
        assert_eq!(
            field.type_,
            ast::Type::ForeignRecord {
                collection: "Account".to_string(),
            }
        );
    }

    #[test]
    fn test_array_map_field() {
        let cases = [
            (
                "collection test { numbers: number[]; }",
                vec![ast::Field {
                    name: "numbers".to_string(),
                    type_: ast::Type::Array(Box::new(ast::Type::Number)),
                    required: true,
                    decorators: vec![],
                }],
            ),
            (
                "collection test { strings: string[]; }",
                vec![ast::Field {
                    name: "strings".to_string(),
                    type_: ast::Type::Array(Box::new(ast::Type::String)),
                    required: true,
                    decorators: vec![],
                }],
            ),
            (
                "collection test { numToStr: map<number, string>; }",
                vec![ast::Field {
                    name: "numToStr".to_string(),
                    type_: ast::Type::Map(Box::new(ast::Type::Number), Box::new(ast::Type::String)),
                    required: true,
                    decorators: vec![],
                }],
            ),
            (
                "collection test { strToNum: map<string, number>; }",
                vec![ast::Field {
                    name: "strToNum".to_string(),
                    type_: ast::Type::Map(Box::new(ast::Type::String), Box::new(ast::Type::Number)),
                    required: true,
                    decorators: vec![],
                }],
            ),
        ];

        for (code, expected) in cases.iter() {
            let program = parse(code).unwrap();
            assert_eq!(program.nodes.len(), 1);
            let collection = match &program.nodes[0] {
                ast::RootNode::Collection(c) => c,
                _ => panic!("expected collection"),
            };
            assert_eq!(collection.items.len(), expected.len());

            for (i, item) in expected.iter().enumerate() {
                assert!(
                    matches!(
                        &collection.items[i],
                        ast::CollectionItem::Field(ast::Field {
                            name,
                            type_,
                            required,
                            decorators,
                        }) if name == &item.name && type_ == &item.type_ && required == &item.required && decorators == &item.decorators
                    ),
                    "expected: {:?}, got: {:?}",
                    item,
                    collection.items[i]
                );
            }
        }
    }

    #[test]
    fn test_object_field() {
        let cases = [
            (
                "collection test { person: { name: string; age: number; }; }",
                vec![ast::Field {
                    name: "person".to_string(),
                    type_: ast::Type::Object(vec![
                        ast::Field {
                            name: "name".to_string(),
                            type_: ast::Type::String,
                            required: true,
                            decorators: vec![],
                        },
                        ast::Field {
                            name: "age".to_string(),
                            type_: ast::Type::Number,
                            required: true,
                            decorators: vec![],
                        },
                    ]),
                    required: true,
                    decorators: vec![],
                }],
            ),
            (
                "collection test { person: { name?: string; }; }",
                vec![ast::Field {
                    name: "person".to_string(),
                    type_: ast::Type::Object(vec![ast::Field {
                        name: "name".to_string(),
                        type_: ast::Type::String,
                        required: false,
                        decorators: vec![],
                    }]),
                    required: true,
                    decorators: vec![],
                }],
            ),
            (
                "collection test { person: { info: { name: string; }; }; }",
                vec![ast::Field {
                    name: "person".to_string(),
                    type_: ast::Type::Object(vec![ast::Field {
                        name: "info".to_string(),
                        type_: ast::Type::Object(vec![ast::Field {
                            name: "name".to_string(),
                            type_: ast::Type::String,
                            required: true,
                            decorators: vec![],
                        }]),
                        required: true,
                        decorators: vec![],
                    }]),
                    required: true,
                    decorators: vec![],
                }],
            ),
        ];

        for (code, expected) in cases.iter() {
            let program = parse(code).unwrap();
            assert_eq!(program.nodes.len(), 1);
            let collection = match &program.nodes[0] {
                ast::RootNode::Collection(c) => c,
                _ => panic!("expected collection"),
            };
            assert_eq!(collection.items.len(), expected.len());

            for (i, item) in expected.iter().enumerate() {
                assert!(
                    matches!(
                        &collection.items[i],
                        ast::CollectionItem::Field(ast::Field {
                            name,
                            type_,
                            required,
                            decorators,
                        }) if name == &item.name && type_ == &item.type_ && required == &item.required
                    ),
                    "expected: {:?}, got: {:?}",
                    item,
                    collection.items[i]
                );
            }
        }
    }

    #[test]
    fn test_comments() {
        let code = "
            collection test {
                // This is a comment
                name: string;

                /*
                    This is a multiline comment
                */
                function test() {
                    return 1;
                }
            }
        ";

        assert!(parse(code).is_ok());
    }

    #[test]
    fn test_index_subfield() {
        let code = "
            collection test {
                person: {
                    name: string;
                };

                @index(person.name);
            }
        ";

        let program = parse(code).unwrap();
        assert_eq!(program.nodes.len(), 1);

        let collection = match &program.nodes[0] {
            ast::RootNode::Collection(c) => c,
            _ => panic!("expected collection"),
        };
        assert_eq!(collection.items.len(), 2);

        assert!(
            matches!(
                &collection.items[1],
                ast::CollectionItem::Index(ast::Index {
                    fields,
                }) if fields == &[ast::IndexField { path: vec!["person".to_string(), "name".to_string()], order: ast::Order::Asc }]
            ),
            "expected: {:?}, got: {:?}",
            &collection.items[1],
            &collection.items[1]
        );
    }

    #[test]
    fn test_decorators() {
        let code = "
            @public
            collection Account {
                @read
                owner: PublicKey;

                @call(owner)
                function noop() {}
            }
        ";

        let program = parse(code).unwrap();
        assert_eq!(program.nodes.len(), 1);

        let collection = match &program.nodes[0] {
            ast::RootNode::Collection(c) => c,
            _ => panic!("expected collection"),
        };

        assert_eq!(collection.decorators.len(), 1);
        assert_eq!(collection.decorators[0].name, "public");

        assert_eq!(collection.items.len(), 2);

        let field = match &collection.items[0] {
            ast::CollectionItem::Field(f) => f,
            _ => panic!("expected field"),
        };

        assert_eq!(field.decorators.len(), 1);
        assert_eq!(field.decorators[0].name, "read");

        let function = match &collection.items[1] {
            ast::CollectionItem::Function(f) => f,
            _ => panic!("expected function"),
        };

        assert_eq!(function.decorators.len(), 1);
        assert_eq!(function.decorators[0].name, "call");
        assert_eq!(function.decorators[0].arguments.len(), 1);
        assert_eq!(function.decorators[0].arguments[0], "owner");
    }

    /// Tests that collections from the filesystem directory 'test-collections' parse without an error
    #[test]
    fn test_fs_collections() {
        use std::path::Path;

        let dir = Path::new("test-collections");
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => panic!("Error reading directory: {}", e),
        };
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                continue;
            }

            let code = std::fs::read_to_string(&path).unwrap();
            let collection = parse(&code);
            if collection.is_err() {
                eprintln!("Error parsing collection: {}", path.display());
                eprintln!("{}", collection.as_ref().unwrap_err().message);
            }
            assert!(collection.is_ok());
        }
    }
}
