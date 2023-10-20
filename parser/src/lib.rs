pub mod ast;
mod lexer;

pub use lalrpop_util::ParseError;
pub use lexer::LexicalError;

pub mod polylang {
    #![allow(unused)]

    use lalrpop_util::lalrpop_mod;
    lalrpop_mod!(
        #[allow(dead_code, clippy::all)]
        polylang
    );
    pub use polylang::*;
}

pub fn parse(
    input: &str,
) -> Result<ast::Program, ParseError<usize, lexer::Tok, lexer::LexicalError>> {
    let lexer = lexer::Lexer::new(input);
    polylang::ProgramParser::new().parse(input, lexer)
}

pub fn parse_expression(
    input: &str,
) -> Result<ast::Expression, ParseError<usize, lexer::Tok, lexer::LexicalError>> {
    let lexer = lexer::Lexer::new(input);
    polylang::ExpressionParser::new().parse(input, lexer)
}

// temp for compiler
pub fn parse_function(
    input: &str,
) -> Result<ast::Function, ParseError<usize, lexer::Tok, lexer::LexicalError>> {
    let lexer = lexer::Lexer::new(input);
    polylang::FunctionParser::new().parse(input, lexer)
}
