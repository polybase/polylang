mod ast;
mod bindings;
mod interpreter;
mod validation;

use serde::Serialize;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub polylang);

#[derive(Debug, Serialize)]
struct Error {
    message: String,
}

fn parse_error_to_error<T, E>(input: &str, error: lalrpop_util::ParseError<usize, T, E>) -> Error
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
        lalrpop_util::ParseError::InvalidToken { location } => {
            make_err(location, location, "Invalid token")
        }
        lalrpop_util::ParseError::UnrecognizedEOF {
            location,
            expected: _,
        } => make_err(location, location, "Unexpected end of file"),
        lalrpop_util::ParseError::UnrecognizedToken {
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
        lalrpop_util::ParseError::ExtraToken {
            token: (start_byte, token, end_byte),
        } => make_err(start_byte, end_byte, &format!("Extra token \"{}\"", token)),
        lalrpop_util::ParseError::User { error } => Error {
            message: format!("{:?}", error),
        },
    }
}

fn parse(input: &str) -> Result<ast::Program, Error> {
    polylang::ProgramParser::new()
        .parse(input)
        .map_err(|e| parse_error_to_error(input, e))
}

fn parse_out_json(input: &str) -> String {
    serde_json::to_string(&parse(input)).unwrap()
}

fn interpret(
    program: &str,
    contract_name: &str,
    func: &str,
    args: HashMap<String, Rc<RefCell<interpreter::Object>>>,
) -> Result<
    (
        interpreter::Object,
        HashMap<String, Rc<RefCell<interpreter::Object>>>,
    ),
    Error,
> {
    let program = parse(program)?;
    let mut interpreter = interpreter::Interpreter::new();

    let contract = program
        .nodes
        .into_iter()
        .find_map(|item| {
            if let ast::RootNode::Contract(c) = item {
                if c.name == contract_name {
                    Some(c)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .ok_or(Error {
            message: "contract not found".to_string(),
        })?;

    interpreter.load(contract).map_err(|e| Error {
        message: e.to_string(),
    })?;

    let (result, vars) = interpreter
        .call(contract_name, func, args)
        .map_err(|e| Error {
            message: e.to_string(),
        })?;

    Ok((result, vars))
}

fn interpret_out_json(
    program: &str,
    contract_name: &str,
    func: &str,
    args: HashMap<String, Rc<RefCell<interpreter::Object>>>,
) -> String {
    serde_json::to_string(&interpret(program, contract_name, func, args)).unwrap()
}

fn validate_set(contract_ast_json: &str, data_json: &str) -> Result<(), Error> {
    let contract_ast: ast::Contract = match serde_json::from_str(contract_ast_json) {
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

    validation::validate_set(contract_ast, data).map_err(|e| Error {
        message: e.to_string(),
    })
}

fn validate_set_out_json(contract_ast_json: &str, data_json: &str) -> String {
    serde_json::to_string(&validate_set(contract_ast_json, data_json)).unwrap()
}

fn validate_set_decorators(
    program_ast_json: &str,
    contract_name: &str,
    data_json: &str,
    previous_data_json: &str,
    public_key: &str,
) -> Result<(), Error> {
    let program_ast: ast::Program = match serde_json::from_str(program_ast_json) {
        Ok(ast) => ast,
        Err(err) => {
            return Err(Error {
                message: err.to_string(),
            })
        }
    };

    let data: HashMap<&str, validation::Value> = match serde_json::from_str(data_json) {
        Ok(data) => data,
        Err(err) => {
            return Err(Error {
                message: err.to_string(),
            })
        }
    };

    let previous_data: HashMap<&str, validation::Value> =
        match serde_json::from_str(previous_data_json) {
            Ok(data) => data,
            Err(err) => {
                return Err(Error {
                    message: err.to_string(),
                })
            }
        };

    validation::validate_set_decorators(
        program_ast,
        contract_name,
        data,
        previous_data,
        if public_key == "" {
            None
        } else {
            Some(public_key)
        },
    )
    .map_err(|e| Error {
        message: e.to_string(),
    })
}

fn validate_set_decorators_out_json(
    program_ast_json: &str,
    contract_name: &str,
    data_json: &str,
    previous_data_json: &str,
    public_key: &str,
) -> String {
    serde_json::to_string(&validate_set_decorators(
        program_ast_json,
        contract_name,
        data_json,
        previous_data_json,
        public_key,
    ))
    .unwrap()
}

#[derive(Debug, Serialize, PartialEq)]
struct JSFunc {
    code: String,
}

fn generate_js_function(func_ast: &str) -> Result<JSFunc, Error> {
    let func_ast: ast::Function = serde_json::from_str(func_ast).map_err(|e| Error {
        message: e.to_string(),
    })?;

    let arg_defs = func_ast
        .parameters
        .into_iter()
        .enumerate()
        .map(|(i, p)| format!("{} = args[{}]", p.name, i))
        .collect::<Vec<String>>()
        .join(", ");

    let arg_defs = if arg_defs.is_empty() {
        arg_defs
    } else {
        format!("const {};", arg_defs)
    };

    Ok(JSFunc {
        code: format!(
            "
function error(str) {{
    return new Error(str);
}}

const f = ($auth, args) => {{\n{}\n{}\n}};
",
            arg_defs, func_ast.statements_code,
        ),
    })
}

fn generate_js_function_out_json(func_ast: &str) -> String {
    serde_json::to_string(&generate_js_function(func_ast)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let input = "contract Test {}";
        let expected_output = r#"{"Ok":{"nodes":[{"Contract":{"name":"Test","items":[]}}]}}"#;

        let output = parse_out_json(input);
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_contract() {
        let program = polylang::ProgramParser::new().parse("contract Test {}");

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Contract(ast::Contract { name, items }) if name == "Test" && items.len() == 0)
        );
    }

    #[test]
    fn test_collection() {
        let program = polylang::ProgramParser::new().parse("collection Test {}");

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Contract(ast::Contract { name, items }) if name == "Test" && items.len() == 0)
        );
    }


    #[test]
    fn test_contract_with_fields() {
        let program = polylang::ProgramParser::new().parse(
            "
            contract Test {
                name: string;
                age: number;
            }
            ",
        );

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Contract(ast::Contract { name, items }) if name == "Test" && items.len() == 2)
        );

        let contract = match &program.nodes[0] {
            ast::RootNode::Contract(contract) => contract,
            _ => panic!("Expected contract"),
        };

        assert!(
            matches!(&contract.items[0], ast::ContractItem::Field(ast::Field { name, type_, required: true, decorators }) if name == "name" && *type_ == ast::Type::String && decorators.is_empty())
        );
        assert!(
            matches!(&contract.items[1], ast::ContractItem::Field(ast::Field { name, type_, required: true, decorators }) if name == "age" && *type_ == ast::Type::Number && decorators.is_empty())
        );
    }

    #[test]
    fn test_contract_with_asc_desc_fields() {
        let program = polylang::ProgramParser::new().parse(
            "
            contract Test {
                asc: string;
                desc: string;
            }
            ",
        );

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Contract(ast::Contract { name, items }) if name == "Test" && items.len() == 2)
        );

        let contract = match &program.nodes[0] {
            ast::RootNode::Contract(contract) => contract,
            _ => panic!("Expected contract"),
        };

        assert!(
            matches!(&contract.items[0], ast::ContractItem::Field(ast::Field { name, type_, required: true, decorators }) if name == "asc" && *type_ == ast::Type::String && decorators.is_empty()),
        );
        assert!(
            matches!(&contract.items[1], ast::ContractItem::Field(ast::Field { name, type_, required: true, decorators }) if name == "desc" && *type_ == ast::Type::String && decorators.is_empty()),
        );
    }

    #[test]
    fn test_fields_with_decorators() {
        let program = polylang::ProgramParser::new().parse(
            "
            contract Test {
                name: string @min(5) @readonly;
                age: number @min(18);
            }
            ",
        );

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);

        let contract = match &program.nodes[0] {
            ast::RootNode::Contract(contract) => contract,
            _ => panic!("Expected contract"),
        };

        let name_field = match &contract.items[0] {
            ast::ContractItem::Field(field) => field,
            _ => panic!("Expected field"),
        };

        assert_eq!(name_field.name, "name");
        assert_eq!(name_field.type_, ast::Type::String);
        assert_eq!(name_field.required, true);
        assert_eq!(name_field.decorators.len(), 2);
        assert_eq!(name_field.decorators[0].name, "min");
        assert_eq!(
            name_field.decorators[0].arguments,
            vec![ast::Primitive::Number(5.0)]
        );

        assert_eq!(name_field.decorators[1].name, "readonly");
        assert_eq!(name_field.decorators[1].arguments, vec![]);

        let age_field = match &contract.items[1] {
            ast::ContractItem::Field(field) => field,
            _ => panic!("Expected field"),
        };

        assert_eq!(age_field.name, "age");
        assert_eq!(age_field.type_, ast::Type::Number);
        assert_eq!(age_field.required, true);
        assert_eq!(age_field.decorators.len(), 1);
        assert_eq!(age_field.decorators[0].name, "min");
        assert_eq!(
            age_field.decorators[0].arguments,
            vec![ast::Primitive::Number(18.0)]
        );
    }

    #[test]
    fn test_contract_with_functions() {
        let program = polylang::ProgramParser::new().parse(
            "
            contract Test {
                function get_age() {
                    return 42;
                }
            }
            ",
        );

        let program = program.unwrap();
        assert_eq!(program.nodes.len(), 1);
        assert!(
            matches!(&program.nodes[0], ast::RootNode::Contract(ast::Contract { name, items }) if name == "Test" && items.len() == 1)
        );

        let contract = match &program.nodes[0] {
            ast::RootNode::Contract(contract) => contract,
            _ => panic!("Expected contract"),
        };

        assert!(
            matches!(&contract.items[0], ast::ContractItem::Function(ast::Function { name, parameters, statements, statements_code }) if name == "get_age" && parameters.len() == 0 && statements.len() == 1 && statements_code == "return 42;")
        );

        let function = match &contract.items[0] {
            ast::ContractItem::Function(function) => function,
            _ => panic!("Expected function"),
        };

        assert!(
            matches!(function.statements[0], ast::Statement::Return(ast::Expression::Primitive(ast::Primitive::Number(number))) if number == 42.0)
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
            contract Account {
                name: string;
                age?: number;
                balance: number;
                publicKey: string;
            
                @index([field, asc], field2);
            
                function transfer (a: record, b: record, amount: number) {
                    if (a.publicKey != $auth.publicKey) throw error('invalid user');
                    
                    a.balance -= amount;
                    b.balance += amount;
                }
            }
        ";

        let contract = polylang::ContractParser::new().parse(code).unwrap();
        assert_eq!(contract.name, "Account");
        assert_eq!(contract.items.len(), 6);

        assert!(matches!(
            &contract.items[0],
            ast::ContractItem::Field(ast::Field { name, type_, required: true, decorators })
            if name == "name" && *type_ == ast::Type::String && decorators.is_empty()
        ));

        assert!(matches!(
            &contract.items[1],
            ast::ContractItem::Field(ast::Field { name, type_, required: false, decorators })
            if name == "age" && *type_ == ast::Type::Number && decorators.is_empty()
        ));

        assert!(matches!(
            &contract.items[2],
            ast::ContractItem::Field(ast::Field { name, type_, required: true, decorators })
            if name == "balance" && *type_ == ast::Type::Number && decorators.is_empty()
        ));

        assert!(matches!(
            &contract.items[3],
            ast::ContractItem::Field(ast::Field { name, type_, required: true, decorators })
            if name == "publicKey" && *type_ == ast::Type::String && decorators.is_empty()
        ));

        assert!(matches!(
            &contract.items[4],
            ast::ContractItem::Index(ast::Index {
                unique,
                fields,
            }) if !unique && fields[0].name == "field" && fields[0].order == ast::Order::Asc
                && fields[1].name == "field2" && fields[1].order == ast::Order::Asc
        ));

        let function = match &contract.items[5] {
            ast::ContractItem::Function(f) => f,
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
                    Box::new(ast::Expression::Ident("a".to_owned())),
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

    #[test]
    fn test_generate_js_function() {
        let func_code = "
            function transfer (a: record, b: record, amount: number) {
                if (a.publicKey != $auth.publicKey) throw error('invalid user');
                
                a.balance -= amount;
                b.balance += amount;
            }
        ";

        let func = polylang::FunctionParser::new().parse(func_code).unwrap();
        let func_ast = serde_json::to_string(&func).unwrap();

        let eval_input = generate_js_function(&func_ast).unwrap();
        assert_eq!(
            eval_input,
            JSFunc {
                code: "
function error(str) {
    return new Error(str);
}

const f = ($auth, args) => {
const a = args[0], b = args[1], amount = args[2];
if (a.publicKey != $auth.publicKey) throw error('invalid user');
                
                a.balance -= amount;
                b.balance += amount;
};
"
                .to_string(),
            },
        );
    }

    #[test]
    fn test_error_unrecognized_token() {
        let code = "
            contract test-cities {}
        ";

        let contract = parse(code);
        assert!(contract.is_err());
        eprintln!("{}", contract.as_ref().unwrap_err().message);
        assert_eq!(
            contract.unwrap_err().message,
            r#"Error found at line 2, column 25: Unrecognized token "-". Expected one of: "{"
contract test-cities {}
             ^"#,
        );
    }

    #[test]
    fn test_error_invalid_token() {
        let code = "
            contract ą {}
        ";

        let contract = parse(code);
        assert!(contract.is_err());
        eprintln!("{}", contract.as_ref().unwrap_err().message);
        assert_eq!(
            contract.unwrap_err().message,
            r#"Error found at line 2, column 21: Invalid token
contract ą {}
         ^"#,
        );
    }

    #[test]
    fn test_error_unexpected_eof() {
        let code = "
            function x() {
        ";

        let contract = parse(code);
        assert!(contract.is_err());
        eprintln!("{}", contract.as_ref().unwrap_err().message);
        assert_eq!(
            contract.unwrap_err().message,
            r#"Error found at line 2, column 26: Unexpected end of file
function x() {
              ^"#,
        );
    }

    #[test]
    fn test_error_field_invalid_type() {
        let code = "
            contract test {
                name: object;
            }
        ";

        let contract = parse(code);
        assert!(contract.is_err());
        eprintln!("{}", contract.as_ref().unwrap_err().message);
        assert_eq!(
            contract.unwrap_err().message,
            r#"Error found at line 3, column 22: Unrecognized token "object". Expected one of: "number", "string"
name: object;
      ^^^^^^"#,
        );
    }
}
