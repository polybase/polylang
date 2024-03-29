use crate::stableast;
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq)]
pub struct JSContract {
    pub code: String,
}

pub fn generate_js_contract(contract_ast: &stableast::Contract) -> JSContract {
    let fns = contract_ast
        .attributes
        .iter()
        .filter_map(|item| {
            if let stableast::ContractAttribute::Method(m) = item {
                let JSFunc { name, code } = generate_js_function(m);
                Some(format!("instance.{} = {}", &name, &code))
            } else {
                None
            }
        })
        .collect::<Vec<String>>()
        .join(";");

    JSContract {
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

fn generate_js_function(func_ast: &stableast::Method) -> JSFunc {
    let parameters = func_ast
        .attributes
        .iter()
        .filter_map(|item| {
            if let stableast::MethodAttribute::Parameter(p) = item {
                Some(p)
            } else {
                None
            }
        })
        .map(|p| format!("{}", p.name))
        .collect::<Vec<String>>()
        .join(", ");

    JSFunc {
        name: func_ast.name.to_string(),
        code: format!(
            "function {} ({}) {{\n{}\n}}",
            func_ast.name, parameters, &func_ast.code,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_js_function() {
        let func_ast = stableast::Method {
            name: "HelloWorld".into(),
            attributes: vec![
                stableast::MethodAttribute::Parameter(stableast::Parameter {
                    name: "a".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::String,
                    }),
                    required: true,
                }),
                stableast::MethodAttribute::Parameter(stableast::Parameter {
                    name: "b".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::Number,
                    }),
                    required: false,
                }),
                stableast::MethodAttribute::ReturnValue(stableast::ReturnValue {
                    name: "_".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::String,
                    }),
                }),
            ],
            code: "return a".into(),
        };

        assert_eq!(
            generate_js_function(&func_ast),
            JSFunc {
                name: "HelloWorld".to_string(),
                code: "function HelloWorld (a, b) {\nreturn a\n}".to_string(),
            }
        )
    }

    #[test]
    fn test_generate_contract_function() {
        let contract_ast = stableast::Contract {
            namespace: stableast::Namespace { value: "".into() },
            name: "ContractName".into(),
            attributes: vec![
                stableast::ContractAttribute::Property(stableast::Property {
                    name: "abc".into(),
                    type_: stableast::Type::Primitive(stableast::Primitive {
                        value: stableast::PrimitiveType::String,
                    }),
                    directives: vec![],
                    required: true,
                }),
                stableast::ContractAttribute::Method(stableast::Method {
                    name: "Hello".into(),
                    attributes: vec![
                        stableast::MethodAttribute::Parameter(stableast::Parameter {
                            name: "a".into(),
                            type_: stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::String,
                            }),
                            required: true,
                        }),
                        stableast::MethodAttribute::Parameter(stableast::Parameter {
                            name: "b".into(),
                            type_: stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::Number,
                            }),
                            required: false,
                        }),
                        stableast::MethodAttribute::ReturnValue(stableast::ReturnValue {
                            name: "_".into(),
                            type_: stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::String,
                            }),
                        }),
                    ],
                    code: "return a".into(),
                }),
                stableast::ContractAttribute::Method(stableast::Method {
                    name: "World".into(),
                    attributes: vec![
                        stableast::MethodAttribute::Parameter(stableast::Parameter {
                            name: "c".into(),
                            type_: stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::String,
                            }),
                            required: true,
                        }),
                        stableast::MethodAttribute::Parameter(stableast::Parameter {
                            name: "d".into(),
                            type_: stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::Number,
                            }),
                            required: false,
                        }),
                        stableast::MethodAttribute::ReturnValue(stableast::ReturnValue {
                            name: "_".into(),
                            type_: stableast::Type::Primitive(stableast::Primitive {
                                value: stableast::PrimitiveType::String,
                            }),
                        }),
                    ],
                    code: "return c".into(),
                }),
            ],
        };

        assert_eq!(
            generate_js_contract(&contract_ast),
            JSContract{
                code: "function error(str) {
                return new Error(str);
            }
            
            const instance = $$__instance;
            instance.Hello = function Hello (a, b) {\nreturn a\n};instance.World = function World (c, d) {\nreturn c\n};".to_string()
            }
        )
    }
}
