pub type Spanned<Tok, Loc, Error> = Result<(Loc, Tok, Loc), Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok<'input> {
    NumberLiteral(f64),
    HexLiteral(&'input str),
    StringLiteral(&'input str),
    Identifier(&'input str),
    EthLiteralStart,
    FalconLiteralStart,
    Desc,
    Asc,
    True,
    False,
    Number,
    F32,
    F64,
    U32,
    U64,
    I32,
    I64,
    String,
    Boolean,
    Map,
    Record,
    PublicKey,
    Bytes,
    Let,
    Break,
    Return,
    Throw,
    If,
    Else,
    While,
    For,
    In,
    Of,
    Function,
    Index,
    Collection,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    ArrowRight, // >
    ArrowLeft,  // <
    Equal,      // =
    EqualEqual, // ==
    BangEqual,  // !=
    MinusEqual, // -=
    PlusEqual,  // +=
    Comma,
    Colon,
    Semicolon,
    Dot,
    Plus,
    Minus,
    Star,     // *
    StarStar, // **
    Slash,    // /
    Percent,
    Bang,
    Question,
    Tilde,
    Ampersand,
    AmpersandAmpersand,
    At, // @
    Caret,
    Pipe,
    PipePipe,
    Lte,
    Gte,
}

impl std::fmt::Display for Tok<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tok::NumberLiteral(n) => write!(f, "{}", n),
            Tok::HexLiteral(s) => write!(f, "{}", s),
            Tok::StringLiteral(s) => write!(f, "{}", s),
            Tok::Identifier(s) => write!(f, "{}", s),
            Tok::EthLiteralStart => write!(f, "eth#"),
            Tok::FalconLiteralStart => write!(f, "falcon#"),
            Tok::Desc => write!(f, "desc"),
            Tok::Asc => write!(f, "asc"),
            Tok::True => write!(f, "true"),
            Tok::False => write!(f, "false"),
            Tok::Number => write!(f, "number"),
            Tok::F32 => write!(f, "f32"),
            Tok::F64 => write!(f, "f64"),
            Tok::U32 => write!(f, "u32"),
            Tok::U64 => write!(f, "u64"),
            Tok::I32 => write!(f, "i32"),
            Tok::I64 => write!(f, "i64"),
            Tok::String => write!(f, "string"),
            Tok::Boolean => write!(f, "boolean"),
            Tok::Map => write!(f, "map"),
            Tok::Record => write!(f, "record"),
            Tok::PublicKey => write!(f, "PublicKey"),
            Tok::Bytes => write!(f, "bytes"),
            Tok::Let => write!(f, "let"),
            Tok::Break => write!(f, "break"),
            Tok::Return => write!(f, "return"),
            Tok::Throw => write!(f, "throw"),
            Tok::If => write!(f, "if"),
            Tok::Else => write!(f, "else"),
            Tok::While => write!(f, "while"),
            Tok::For => write!(f, "for"),
            Tok::In => write!(f, "in"),
            Tok::Of => write!(f, "of"),
            Tok::Function => write!(f, "function"),
            Tok::Index => write!(f, "index"),
            Tok::Collection => write!(f, "collection"),
            Tok::LBrace => write!(f, "{{"),
            Tok::RBrace => write!(f, "}}"),
            Tok::LBracket => write!(f, "["),
            Tok::RBracket => write!(f, "]"),
            Tok::LParen => write!(f, "("),
            Tok::RParen => write!(f, ")"),
            Tok::ArrowRight => write!(f, ">"),
            Tok::ArrowLeft => write!(f, "<"),
            Tok::Equal => write!(f, "="),
            Tok::EqualEqual => write!(f, "=="),
            Tok::BangEqual => write!(f, "!="),
            Tok::MinusEqual => write!(f, "-="),
            Tok::PlusEqual => write!(f, "+="),
            Tok::Comma => write!(f, ","),
            Tok::Colon => write!(f, ":"),
            Tok::Semicolon => write!(f, ";"),
            Tok::Dot => write!(f, "."),
            Tok::Plus => write!(f, "+"),
            Tok::Minus => write!(f, "-"),
            Tok::Star => write!(f, "*"),
            Tok::StarStar => write!(f, "**"),
            Tok::Slash => write!(f, "/"),
            Tok::Percent => write!(f, "%"),
            Tok::Bang => write!(f, "!"),
            Tok::Question => write!(f, "?"),
            Tok::Tilde => write!(f, "~"),
            Tok::Ampersand => write!(f, "&"),
            Tok::AmpersandAmpersand => write!(f, "&&"),
            Tok::At => write!(f, "@"),
            Tok::Caret => write!(f, "^"),
            Tok::Pipe => write!(f, "|"),
            Tok::PipePipe => write!(f, "||"),
            Tok::Lte => write!(f, "<="),
            Tok::Gte => write!(f, ">="),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum LexicalError {
    NumberParseError {
        start: usize,
        end: usize,
    },
    InvalidToken {
        start: usize,
        end: usize,
    },
    UnterminatedComment {
        start: usize,
        end: usize,
    },
    UnterminatedString {
        start: usize,
        end: usize,
    },
    UserError {
        start: usize,
        end: usize,
        message: String,
    },
}

impl std::fmt::Display for LexicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexicalError::NumberParseError { start, end } => {
                write!(f, "Failed to parse number at {}-{}", start, end)
            }
            LexicalError::InvalidToken { start, end } => {
                write!(f, "Invalid token at {}-{}", start, end)
            }
            LexicalError::UnterminatedComment { start, end } => {
                write!(f, "Unterminated comment at {}-{}", start, end)
            }
            LexicalError::UnterminatedString { start, end } => {
                write!(f, "Unterminated string at {}-{}", start, end)
            }
            LexicalError::UserError {
                start,
                end,
                message,
            } => {
                write!(f, "Error at {}-{}: {}", start, end, message)
            }
        }
    }
}

impl std::error::Error for LexicalError {}

const KEYWORDS: &[(Tok, &str)] = &[
    (Tok::Desc, "desc"),
    (Tok::Asc, "asc"),
    (Tok::True, "true"),
    (Tok::False, "false"),
    (Tok::Number, "number"),
    (Tok::F32, "f32"),
    (Tok::F64, "f64"),
    (Tok::U32, "u32"),
    (Tok::U64, "u64"),
    (Tok::I32, "i32"),
    (Tok::I64, "i64"),
    (Tok::String, "string"),
    (Tok::Boolean, "boolean"),
    (Tok::Map, "map"),
    (Tok::Record, "record"),
    (Tok::Let, "let"),
    (Tok::Break, "break"),
    (Tok::Return, "return"),
    (Tok::Throw, "throw"),
    (Tok::If, "if"),
    (Tok::Else, "else"),
    (Tok::While, "while"),
    (Tok::For, "for"),
    (Tok::In, "in"),
    (Tok::Of, "of"),
    (Tok::Function, "function"),
    (Tok::Index, "@index"),
    (Tok::Collection, "collection"),
    (Tok::PublicKey, "PublicKey"),
    (Tok::Bytes, "bytes"),
];

pub struct Lexer<'input> {
    input: &'input str,
    position: usize,
    errored: bool,
}

type LexerItem<'input> = Spanned<Tok<'input>, usize, LexicalError>;

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            input,
            position: 0,
            errored: false,
        }
    }

    fn peek_char(&self) -> Option<(usize, char)> {
        self.input[self.position..]
            .char_indices()
            .next()
            .map(|(i, c)| (i + self.position, c))
    }

    fn peek_char_nth(&self, nth: usize) -> Option<(usize, char)> {
        self.input[self.position..]
            .char_indices()
            .nth(nth)
            .map(|(i, c)| (i + self.position, c))
    }

    fn next_char(&mut self) -> Option<(usize, char)> {
        let next = self.peek_char();
        if let Some((i, c)) = next {
            self.position = i + c.len_utf8();
        }
        next
    }

    fn reset_if_none(
        &mut self,
        f: impl FnOnce(&mut Self) -> Option<LexerItem<'input>>,
    ) -> Option<LexerItem<'input>> {
        let start = self.position;
        let item = f(self);
        if item.is_none() {
            self.position = start;
        }
        item
    }

    fn eat_whitespace(&mut self) -> bool {
        match self.peek_char() {
            Some((_, c)) if c.is_whitespace() => {}
            _ => return false,
        }

        while let Some((_, c)) = self.peek_char() {
            if !c.is_whitespace() {
                break;
            }
            self.next_char();
        }

        true
    }

    /// Eats comments in the form of `// ...` or `/* ... */`
    fn eat_comments(&mut self) -> Result<bool, LexicalError> {
        let mut found = false;

        if let Some((_, '/')) = self.peek_char() {
            match self.peek_char_nth(1) {
                Some((_, '/')) => {
                    self.next_char();
                    self.next_char();

                    found = true;

                    while let Some((_, c)) = self.peek_char() {
                        if c == '\n' {
                            break;
                        }

                        self.next_char();
                    }
                }
                Some((i, '*')) => {
                    self.next_char();
                    self.next_char();

                    found = true;

                    let found_end = loop {
                        if let Some((_, '*')) = self.peek_char() {
                            if let Some((_, '/')) = self.peek_char_nth(1) {
                                self.next_char();
                                self.next_char();
                                break true;
                            }
                        }

                        if self.next_char().is_none() {
                            break false;
                        }
                    };

                    if !found_end {
                        return Err(LexicalError::UnterminatedComment {
                            start: i - 1, // start of `/*`
                            end: self.position,
                        });
                    }
                }
                None | Some(_) => {}
            }
        }

        Ok(found)
    }

    fn lex_keyword(&mut self) -> Option<LexerItem<'input>> {
        let (start, c) = self.peek_char()?;
        if !c.is_ascii_alphabetic() {
            return None;
        }

        let mut end = start;
        let mut keyword = String::new();
        while let Some((i, c)) = self.peek_char() {
            if !c.is_ascii_alphanumeric() && c != '_' {
                break;
            }
            end = i;
            keyword.push(c);
            self.next_char();
        }

        KEYWORDS
            .iter()
            .find(|(_, k)| k == &keyword)
            .map(|(tok, _)| Ok::<_, LexicalError>((start, tok.clone(), end + c.len_utf8())))
    }

    /// parses 'eth#', 'falcon#', etc.
    fn lex_literal_start(&mut self) -> Option<LexerItem<'input>> {
        let (start, c) = self.peek_char()?;
        if !c.is_ascii_alphabetic() {
            return None;
        }

        let mut end = start;
        let mut literal_start = String::new();
        while let Some((i, c)) = self.peek_char() {
            if !c.is_ascii_alphanumeric() && c != '_' {
                break;
            }
            end = i;
            literal_start.push(c);
            self.next_char();
        }

        if self.peek_char() != Some((end + c.len_utf8(), '#')) {
            return None;
        }
        self.next_char();
        literal_start += "#";

        [
            (Tok::EthLiteralStart, "eth#"),
            (Tok::FalconLiteralStart, "falcon#"),
        ]
        .iter()
        .find(|(_, k)| k == &&literal_start)
        .map(|(tok, _)| Ok::<_, LexicalError>((start, tok.clone(), end + c.len_utf8())))
    }

    fn lex_at_index(&mut self) -> Option<LexerItem<'input>> {
        match (
            self.peek_char(),
            self.peek_char_nth(1),
            self.peek_char_nth(2),
            self.peek_char_nth(3),
            self.peek_char_nth(4),
            self.peek_char_nth(5),
        ) {
            (
                Some((start, '@')),
                Some((_, 'i')),
                Some((_, 'n')),
                Some((_, 'd')),
                Some((_, 'e')),
                Some((end, 'x')),
            ) => {
                self.next_char();
                self.next_char();
                self.next_char();
                self.next_char();
                self.next_char();
                self.next_char();
                Some(Ok((start, Tok::Index, end + 'x'.len_utf8())))
            }
            _ => None,
        }
    }

    fn lex_number(&mut self) -> Option<LexerItem<'input>> {
        let (start, c) = self.peek_char()?;
        if !c.is_numeric() {
            return None;
        }

        let mut end = start;
        let mut number = String::new();
        while let Some((i, c)) = self.peek_char() {
            if !c.is_numeric() && c != '.' {
                break;
            }
            end = i;
            number.push(c);
            self.next_char();
        }

        number
            .parse::<f64>()
            .map_err(|_| LexicalError::NumberParseError {
                start,
                end: end + c.len_utf8(),
            })
            .map(|n| Ok((start, Tok::NumberLiteral(n), end + c.len_utf8())))
            .ok()
    }

    /// parses 'hello' as Tok::String("'hello'")
    /// or "hello" as Tok::String("\"hello\"")
    fn lex_string(&mut self) -> Option<LexerItem<'input>> {
        let (start, c) = self.peek_char()?;
        let is_double_quoted = match c {
            '\'' => false,
            '"' => true,
            _ => return None,
        };
        self.next_char();

        let mut end = start;
        let mut string = String::new();
        let terminated = loop {
            let Some((i, c)) = self.peek_char() else {
                break false;
            };

            match (is_double_quoted, c) {
                (true, '"') | (false, '\'') => {
                    end = i;
                    self.next_char();
                    break true;
                }
                _ => {}
            }

            end = i;
            string.push(c);
            self.next_char();
        };

        if !terminated {
            return Some(Err(LexicalError::UnterminatedString {
                start,
                end: self.position,
            }));
        }

        Some(Ok((
            start,
            Tok::StringLiteral(&self.input[start..end + c.len_utf8()]),
            end + c.len_utf8(),
        )))
    }

    fn lex_hex_literal(&mut self) -> Option<LexerItem<'input>> {
        let (start, c) = self.peek_char()?;
        if c != '0' {
            return None;
        }
        self.next_char();

        let (x_pos, c) = self.peek_char()?;
        if c != 'x' {
            return None;
        }
        self.next_char();

        let mut end = x_pos;
        let mut hex = String::new();
        while let Some((i, c)) = self.peek_char() {
            if !c.is_ascii_hexdigit() {
                break;
            }
            end = i;
            hex.push(c);
            self.next_char();
        }

        Some(Ok((
            start,
            Tok::HexLiteral(&self.input[start..end + c.len_utf8()]),
            end + c.len_utf8(),
        )))
    }

    fn lex_identifier(&mut self) -> Option<LexerItem<'input>> {
        let (start, c) = self.peek_char()?;
        if !(c.is_ascii_alphabetic() || c == '_' || c == '$') {
            return None;
        }

        self.next_char();

        let mut end = start;
        let mut identifier = String::new();
        while let Some((i, c)) = self.peek_char() {
            if !(c.is_ascii_alphanumeric() || c == '_') {
                break;
            }
            end = i;
            identifier.push(c);
            self.next_char();
        }

        Some(Ok((
            start,
            Tok::Identifier(&self.input[start..end + c.len_utf8()]),
            end + c.len_utf8(),
        )))
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = LexerItem<'input>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.errored {
            return None;
        }

        loop {
            let found_whitespace = self.eat_whitespace();
            let found_comment = match self.eat_comments() {
                Ok(b) => b,
                Err(e) => {
                    return Some(Err(e));
                }
            };

            if !found_whitespace && !found_comment {
                break;
            }
        }

        let result = self
            .reset_if_none(Self::lex_keyword)
            .or_else(|| self.reset_if_none(Self::lex_literal_start))
            .or_else(|| self.reset_if_none(Self::lex_at_index))
            .or_else(|| self.reset_if_none(Self::lex_hex_literal))
            .or_else(|| self.reset_if_none(Self::lex_number))
            .or_else(|| self.reset_if_none(Self::lex_string))
            .or_else(|| self.reset_if_none(Self::lex_identifier))
            .or_else(|| match self.peek_char()? {
                (i, '{') => {
                    self.next_char();
                    Some(Ok((i, Tok::LBrace, i + 1)))
                }
                (i, '}') => {
                    self.next_char();
                    Some(Ok((i, Tok::RBrace, i + 1)))
                }
                (i, '[') => {
                    self.next_char();
                    Some(Ok((i, Tok::LBracket, i + 1)))
                }
                (i, ']') => {
                    self.next_char();
                    Some(Ok((i, Tok::RBracket, i + 1)))
                }
                (i, '(') => {
                    self.next_char();
                    Some(Ok((i, Tok::LParen, i + 1)))
                }
                (i, ')') => {
                    self.next_char();
                    Some(Ok((i, Tok::RParen, i + 1)))
                }
                (i, '>') => {
                    self.next_char();
                    match self.peek_char() {
                        Some((_, '=')) => {
                            self.next_char();
                            Some(Ok((i, Tok::Gte, i + 2)))
                        }
                        _ => Some(Ok((i, Tok::ArrowRight, i + 1))),
                    }
                }
                (i, '<') => {
                    self.next_char();
                    match self.peek_char() {
                        Some((_, '=')) => {
                            self.next_char();
                            Some(Ok((i, Tok::Lte, i + 2)))
                        }
                        _ => Some(Ok((i, Tok::ArrowLeft, i + 1))),
                    }
                }
                (i, '=') => {
                    self.next_char();
                    match self.peek_char() {
                        Some((_, '=')) => {
                            self.next_char();
                            Some(Ok((i, Tok::EqualEqual, i + 2)))
                        }
                        _ => Some(Ok((i, Tok::Equal, i + 1))),
                    }
                }
                (i, ',') => {
                    self.next_char();
                    Some(Ok((i, Tok::Comma, i + 1)))
                }
                (i, ';') => {
                    self.next_char();
                    Some(Ok((i, Tok::Semicolon, i + 1)))
                }
                (i, ':') => {
                    self.next_char();
                    Some(Ok((i, Tok::Colon, i + 1)))
                }
                (i, '.') => {
                    self.next_char();
                    Some(Ok((i, Tok::Dot, i + 1)))
                }
                (i, '+') => {
                    self.next_char();

                    match self.peek_char() {
                        Some((_, '=')) => {
                            self.next_char();
                            Some(Ok((i, Tok::PlusEqual, i + 2)))
                        }
                        _ => Some(Ok((i, Tok::Plus, i + 1))),
                    }
                }
                (i, '-') => {
                    self.next_char();

                    match self.peek_char() {
                        Some((_, '=')) => {
                            self.next_char();
                            Some(Ok((i, Tok::MinusEqual, i + 2)))
                        }
                        _ => Some(Ok((i, Tok::Minus, i + 1))),
                    }
                }
                (i, '/') => {
                    self.next_char();
                    Some(Ok((i, Tok::Slash, i + 1)))
                }
                (i, '%') => {
                    self.next_char();
                    Some(Ok((i, Tok::Percent, i + 1)))
                }
                (i, '!') => {
                    self.next_char();
                    match self.peek_char() {
                        Some((_, '=')) => {
                            self.next_char();
                            Some(Ok((i, Tok::BangEqual, i + 2)))
                        }
                        _ => Some(Ok((i, Tok::Bang, i + 1))),
                    }
                }
                (i, '?') => {
                    self.next_char();
                    Some(Ok((i, Tok::Question, i + 1)))
                }
                (i, '~') => {
                    self.next_char();
                    Some(Ok((i, Tok::Tilde, i + 1)))
                }
                (i, '*') => {
                    self.next_char();
                    match self.peek_char() {
                        Some((_, '*')) => {
                            self.next_char();
                            Some(Ok((i, Tok::StarStar, i + 2)))
                        }
                        _ => Some(Ok((i, Tok::Star, i + 1))),
                    }
                }
                (i, '&') => {
                    self.next_char();
                    match self.peek_char() {
                        Some((_, '&')) => {
                            self.next_char();
                            Some(Ok((i, Tok::AmpersandAmpersand, i + 2)))
                        }
                        _ => Some(Ok((i, Tok::Ampersand, i + 1))),
                    }
                }
                (i, '@') => {
                    self.next_char();
                    Some(Ok((i, Tok::At, i + 1)))
                }
                (i, '^') => {
                    self.next_char();
                    Some(Ok((i, Tok::Caret, i + 1)))
                }
                (i, '|') => {
                    self.next_char();
                    match self.peek_char() {
                        Some((_, '|')) => {
                            self.next_char();
                            Some(Ok((i, Tok::PipePipe, i + 2)))
                        }
                        _ => Some(Ok((i, Tok::Pipe, i + 1))),
                    }
                }
                _ => None,
            })
            .or_else(|| {
                if let Some((i, _)) = self.peek_char() {
                    self.next_char();
                    Some(Err(LexicalError::InvalidToken { start: i, end: i }))
                } else {
                    None
                }
            });

        if let Some(Err(_)) = result {
            self.errored = true;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_whitespace() {
        let mut lexer = Lexer::new("  ");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_whitespace_2() {
        let mut lexer = Lexer::new("  asc");
        assert_eq!(lexer.next(), Some(Ok((2, Tok::Asc, 5))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_keyword() {
        let input = "desc asc";
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next(), Some(Ok((0, Tok::Desc, 4))));
        assert_eq!(&input[0..4], "desc");
        assert_eq!(lexer.next(), Some(Ok((5, Tok::Asc, 8))));
        assert_eq!(&input[5..8], "asc");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_number() {
        let mut lexer = Lexer::new("123.456 987");
        assert_eq!(lexer.next(), Some(Ok((0, Tok::NumberLiteral(123.456), 7))));
        assert_eq!(lexer.next(), Some(Ok((8, Tok::NumberLiteral(987.0), 11))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_number_error() {
        let mut lexer = Lexer::new("123.456.789");
        assert_eq!(
            lexer.next(),
            Some(Err(LexicalError::InvalidToken { start: 0, end: 0 }))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_string() {
        let input = "'hello' 'world'";
        let mut lexer = Lexer::new("'hello' 'world'");
        assert_eq!(
            lexer.next(),
            Some(Ok((0, Tok::StringLiteral("'hello'"), 7)))
        );
        assert_eq!(&input[0..7], "'hello'");
        assert_eq!(
            lexer.next(),
            Some(Ok((8, Tok::StringLiteral("'world'"), 15)))
        );
        assert_eq!(&input[8..15], "'world'");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_double_quote_string() {
        let input = "\"hello\" \"world\"";
        let mut lexer = Lexer::new("\"hello\" \"world\"");
        assert_eq!(
            lexer.next(),
            Some(Ok((0, Tok::StringLiteral("\"hello\""), 7)))
        );
        assert_eq!(&input[0..7], "\"hello\"");
        assert_eq!(
            lexer.next(),
            Some(Ok((8, Tok::StringLiteral("\"world\""), 15)))
        );
        assert_eq!(&input[8..15], "\"world\"");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_string_unterminated() {
        let mut lexer = Lexer::new("'hello");
        assert_eq!(
            lexer.next(),
            Some(Err(LexicalError::UnterminatedString { start: 0, end: 6 }))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_string_unterminated_mismatch() {
        let mut lexer = Lexer::new("'hello\"");
        assert_eq!(
            lexer.next(),
            Some(Err(LexicalError::UnterminatedString { start: 0, end: 7 }))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_identifier() {
        let input = "hello world";
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next(), Some(Ok((0, Tok::Identifier("hello"), 5))));
        assert_eq!(&input[0..5], "hello");
        assert_eq!(lexer.next(), Some(Ok((6, Tok::Identifier("world"), 11))));
        assert_eq!(&input[6..11], "world");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_identifier_dollar() {
        let input = "$hello $world";
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next(), Some(Ok((0, Tok::Identifier("$hello"), 6))));
        assert_eq!(&input[0..6], "$hello");
        assert_eq!(lexer.next(), Some(Ok((7, Tok::Identifier("$world"), 13))));
        assert_eq!(&input[7..13], "$world");
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_identifier_unicode_invalid() {
        let input = "ą";
        let mut lexer = Lexer::new(input);
        assert_eq!(
            lexer.next(),
            Some(Err(LexicalError::InvalidToken { start: 0, end: 0 }))
        );
    }

    #[test]
    fn test_lex_symbols() {
        let cases = [
            ("(", Tok::LParen),
            (")", Tok::RParen),
            ("[", Tok::LBracket),
            ("]", Tok::RBracket),
            ("{", Tok::LBrace),
            ("}", Tok::RBrace),
            ("+", Tok::Plus),
            ("-", Tok::Minus),
            ("*", Tok::Star),
            ("**", Tok::StarStar),
            ("/", Tok::Slash),
            ("%", Tok::Percent),
            ("!", Tok::Bang),
            ("?", Tok::Question),
            ("~", Tok::Tilde),
            ("&", Tok::Ampersand),
            ("&&", Tok::AmpersandAmpersand),
            ("@", Tok::At),
            ("^", Tok::Caret),
            ("|", Tok::Pipe),
            ("||", Tok::PipePipe),
            ("=", Tok::Equal),
            ("==", Tok::EqualEqual),
            ("!=", Tok::BangEqual),
            ("-=", Tok::MinusEqual),
            ("+=", Tok::PlusEqual),
            (",", Tok::Comma),
            (":", Tok::Colon),
            (";", Tok::Semicolon),
            (".", Tok::Dot),
            ("<", Tok::ArrowLeft),
            (">", Tok::ArrowRight),
            ("<=", Tok::Lte),
            (">=", Tok::Gte),
        ];

        for (input, expected) in cases.into_iter() {
            let mut lexer = Lexer::new(input);
            assert_eq!(lexer.next(), Some(Ok((0, expected, input.len()))));
            assert_eq!(lexer.next(), None);
        }
    }

    #[test]
    fn test_comments() {
        let input = "/* comment */";
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_comments_2() {
        let input = "/* comment */ 123";
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next(), Some(Ok((14, Tok::NumberLiteral(123.0), 17))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_comments_single_line() {
        let input = "// comment";
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_comments_mixed() {
        let input = r#"// comment
            123
            /* comment */
            456
            /*
                multi-
                line
            */
        "#;

        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next(), Some(Ok((23, Tok::NumberLiteral(123.0), 26))));
        assert_eq!(lexer.next(), Some(Ok((65, Tok::NumberLiteral(456.0), 68))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_comments_error() {
        let input = "/* comment";
        let mut lexer = Lexer::new(input);
        assert_eq!(
            lexer.next(),
            Some(Err(LexicalError::UnterminatedComment { start: 0, end: 10 }))
        );
    }
}
