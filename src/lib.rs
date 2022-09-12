mod ast;
mod interpreter;
mod validation;

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use lalrpop_util::lalrpop_mod;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

lalrpop_mod!(pub spacetime);

fn parse_out_json(input: &str) -> String {
    let program = spacetime::ProgramParser::new().parse(input);
    match program {
        Ok(program) => serde_json::to_string(&program).unwrap(),
        Err(err) => {
            serde_json::to_string(&serde_json::json!({ "error": err.to_string() })).unwrap()
        }
    }
}

fn interpret_out_json(
    program: &str,
    collection_name: &str,
    func: &str,
    args: HashMap<String, Rc<RefCell<interpreter::Object>>>,
) -> String {
    let program = spacetime::ProgramParser::new().parse(program);
    if let Err(err) = program {
        return serde_json::to_string(&serde_json::json!({ "error": err.to_string() })).unwrap();
    }
    let program = program.unwrap();

    let mut interpreter = interpreter::Interpreter::new();

    let mut collection: Option<ast::Collection> = None;
    for item in program.nodes {
        match item {
            ast::RootNode::Collection(c) => {
                if c.name == collection_name {
                    collection = Some(c);
                }
            }
            _ => {}
        }
    }
    if let None = collection {
        return serde_json::to_string(&serde_json::json!({ "error": "collection not found" }))
            .unwrap();
    }
    let collection = collection.unwrap();

    if let Err(err) = interpreter.load(collection) {
        return serde_json::to_string(&serde_json::json!({ "error": err.to_string() })).unwrap();
    }

    let obj = interpreter.call(collection_name, func, args);
    if let Err(err) = obj {
        return serde_json::to_string(&serde_json::json!({ "error": err.to_string() })).unwrap();
    }
    let obj = obj.unwrap();
    serde_json::to_string(&obj).unwrap()
}

fn validate_set_out_json(ast_json: &str, data_json: &str) -> String {
    let ast: ast::Collection = match serde_json::from_str(ast_json) {
        Ok(ast) => ast,
        Err(err) => {
            return serde_json::to_string(&serde_json::json!({ "error": err.to_string() })).unwrap()
        }
    };

    let data: HashMap<String, validation::Value> = match serde_json::from_str(data_json) {
        Ok(data) => data,
        Err(err) => {
            return serde_json::to_string(&serde_json::json!({ "error": err.to_string() })).unwrap()
        }
    };

    let result = validation::validate_set(ast, data);
    if let Err(err) = result {
        return serde_json::to_string(&serde_json::json!({ "error": err.to_string() })).unwrap();
    }

    "{}".to_string()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(msg: String);
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn init() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn parse(input: &str) -> String {
    parse_out_json(input)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn interpret(program: &str, collection_name: &str, func: &str, args: &str) -> String {
    let args = serde_json::from_str(args).unwrap();
    interpret_out_json(program, collection_name, func, args)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn validate_set(ast_json: &str, data_json: &str) -> String {
    validate_set_out_json(ast_json, data_json)
}

#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn parse(input: *const i8) -> *mut i8 {
    let input = unsafe { std::ffi::CStr::from_ptr(input) };
    let input = input.to_str().unwrap();

    let output = parse_out_json(input);
    let output = std::ffi::CString::new(output).unwrap();
    output.into_raw()
}

#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn interpret(
    program: *const i8,
    collection_name: *const i8,
    func: *const i8,
    args: *const i8,
) -> *mut i8 {
    let program = unsafe { std::ffi::CStr::from_ptr(program) };
    let program = program.to_str().unwrap();

    let collection_name = unsafe { std::ffi::CStr::from_ptr(collection_name) };
    let collection_name = collection_name.to_str().unwrap();

    let func = unsafe { std::ffi::CStr::from_ptr(func) };
    let func = func.to_str().unwrap();

    let args = unsafe { std::ffi::CStr::from_ptr(args) };
    let args = serde_json::from_str(args.to_str().unwrap()).unwrap();

    let output = interpret_out_json(program, collection_name, func, args);
    let output = std::ffi::CString::new(output).unwrap();
    output.into_raw()
}

#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn validate_set(ast_json: *const i8, data_json: *const i8) -> *mut i8 {
    let ast_json = unsafe { std::ffi::CStr::from_ptr(ast_json) };
    let ast_json = ast_json.to_str().unwrap();

    let data_json = unsafe { std::ffi::CStr::from_ptr(data_json) };
    let data_json = data_json.to_str().unwrap();

    let output = validate_set_out_json(ast_json, data_json);
    let output = std::ffi::CString::new(output).unwrap();
    output.into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let input = "collection Test {}";
        let expected_output = r#"{"nodes":[{"Collection":{"name":"Test","items":[]}}]}"#;

        let input_cstr = std::ffi::CString::new(input).unwrap();

        let output = parse(input_cstr.as_ptr());
        let output = unsafe { std::ffi::CStr::from_ptr(output) };
        let output = output.to_str().unwrap();

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
                fields,
            }) if fields[0].0 == "field" && fields[0].1 == ast::Order::Asc
                && fields[1].0 == "field2" && fields[1].1 == ast::Order::Asc
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
