use crate::ast;
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq)]
pub struct JSCollection {
    code: String,
}

pub fn generate_js_collection (collection_ast: &ast::Collection) -> JSCollection {
    let fns = collection_ast
        .items
        .iter()
        .filter_map(|item| 
            if let ast::CollectionItem::Function(f) = item { 
                let JSFunc{ name, code } = generate_js_function(&f);
                Some(format!("instance.{} = {}", &name, &code))
            } 
            else { None })
        .collect::<Vec<String>>()
        .join(";");

    JSCollection {
        code: format!(
            "function error(str) {{
                return new Error(str);
            }}
            
            const instance = $$__instance;
            {};",
            fns,
        ),
    }
}

#[derive(Debug, PartialEq)]
struct JSFunc {
    name: String,
    code: String,
}

fn generate_js_function(func_ast: &ast::Function) -> JSFunc {
    let parameters = func_ast
        .parameters
        .iter()
        .map(|p| format!("{}", p.name))
        .collect::<Vec<String>>()
        .join(", ");

    JSFunc {
        name: func_ast.name.clone(),
        code: format!(
            "function {} ({}) {{\n{}\n}}",
            func_ast.name, 
            parameters,
            func_ast.statements_code,
        ),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_js_function () {
        let func_ast = ast::Function{ 
            name: "HelloWorld".to_string(), 
            parameters: vec![
                ast::Parameter{ name: "a".to_string(), type_: ast::ParameterType::String, required: true },
                ast::Parameter{ name: "b".to_string(), type_: ast::ParameterType::Number, required: false },
            ],
            statements: vec![],
            statements_code: "return a".to_string(),
        };

        assert_eq!(
            generate_js_function(&func_ast),
            JSFunc{
                name: "HelloWorld".to_string(),
                code: "function HelloWorld (a, b) {\nreturn a\n}".to_string(),
            }
        )
    }

    #[test]
    fn test_generate_collection_function () {
        let collection_ast = ast::Collection{
            name: "CollectionName".to_string(),
            items: vec![
                ast::CollectionItem::Field(ast::Field{
                    name: "abc".to_string(),
                    type_: ast::Type::String,
                    required: true,
                }),
                ast::CollectionItem::Function(ast::Function{
                    name: "Hello".to_string(),
                    parameters: vec![
                        ast::Parameter{ name: "a".to_string(), type_: ast::ParameterType::String, required: true },
                        ast::Parameter{ name: "b".to_string(), type_: ast::ParameterType::Number, required: false },
                    ],
                    statements: vec![],
                    statements_code: "return a".to_string(),
                }),
                ast::CollectionItem::Function(ast::Function{
                    name: "World".to_string(),
                    parameters: vec![
                        ast::Parameter{ name: "c".to_string(), type_: ast::ParameterType::String, required: true },
                        ast::Parameter{ name: "d".to_string(), type_: ast::ParameterType::Number, required: false },
                    ],
                    statements: vec![],
                    statements_code: "return c".to_string(),
                })
            ]
        };

        assert_eq!(
            generate_js_collection(&collection_ast),
            JSCollection{
                code: "function error(str) {
                return new Error(str);
            }
            
            const instance = $$__instance;
            instance.Hello = function Hello (a, b) {\nreturn a\n};instance.World = function World (c, d) {\nreturn c\n};".to_string()
            }
        )
    }

}
