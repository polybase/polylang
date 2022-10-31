pub mod ast;

use lalrpop_util::lalrpop_mod;
pub use lalrpop_util::ParseError;

lalrpop_mod!(pub polylang);
