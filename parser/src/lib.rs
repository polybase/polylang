pub mod ast;
mod lexer;

use lalrpop_util::lalrpop_mod;
pub use lalrpop_util::ParseError;
pub use lexer::LexicalError;

lalrpop_mod!(polylang);

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
