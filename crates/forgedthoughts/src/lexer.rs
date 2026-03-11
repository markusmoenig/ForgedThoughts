use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Let,
    Var,
    Fn,
    Return,
    Import,
    Export,
    Ident(String),
    String(String),
    HexColor(String),
    Number(f64),
    Equal,
    Semicolon,
    Colon,
    Comma,
    Dot,
    Plus,
    Minus,
    Amp,
    Star,
    Slash,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
}

#[derive(Debug, Error)]
pub enum LexError {
    #[error("unexpected character '{ch}' at byte {offset}")]
    UnexpectedChar { ch: char, offset: usize },
    #[error("invalid number '{lexeme}' at byte {offset}")]
    InvalidNumber { lexeme: String, offset: usize },
    #[error("invalid hex color '{lexeme}' at byte {offset}")]
    InvalidHexColor { lexeme: String, offset: usize },
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let mut chars = input.char_indices().peekable();
    let mut tokens = Vec::new();

    while let Some((offset, ch)) = chars.next() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {}
            '"' => {
                let mut value = String::new();
                let mut closed = false;
                while let Some((_, next)) = chars.next() {
                    match next {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => {
                            let Some((_, escaped)) = chars.next() else {
                                break;
                            };
                            let mapped = match escaped {
                                '"' => '"',
                                '\\' => '\\',
                                'n' => '\n',
                                'r' => '\r',
                                't' => '\t',
                                other => other,
                            };
                            value.push(mapped);
                        }
                        other => value.push(other),
                    }
                }
                if !closed {
                    return Err(LexError::UnexpectedChar { ch, offset });
                }
                tokens.push(Token {
                    kind: TokenKind::String(value),
                    start: offset,
                });
            }
            '#' => {
                let mut lexeme = String::from("#");
                while let Some((_, next)) = chars.peek() {
                    if next.is_ascii_hexdigit() {
                        lexeme.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let digits = &lexeme[1..];
                if !(digits.len() == 3 || digits.len() == 6) {
                    return Err(LexError::InvalidHexColor { lexeme, offset });
                }
                tokens.push(Token {
                    kind: TokenKind::HexColor(digits.to_ascii_lowercase()),
                    start: offset,
                });
            }
            '/' => {
                if let Some((_, '/')) = chars.peek() {
                    for (_, c) in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                } else {
                    tokens.push(simple(TokenKind::Slash, offset));
                }
            }
            '=' => tokens.push(simple(TokenKind::Equal, offset)),
            ';' => tokens.push(simple(TokenKind::Semicolon, offset)),
            ':' => tokens.push(simple(TokenKind::Colon, offset)),
            ',' => tokens.push(simple(TokenKind::Comma, offset)),
            '.' => tokens.push(simple(TokenKind::Dot, offset)),
            '+' => tokens.push(simple(TokenKind::Plus, offset)),
            '-' => tokens.push(simple(TokenKind::Minus, offset)),
            '&' => tokens.push(simple(TokenKind::Amp, offset)),
            '*' => tokens.push(simple(TokenKind::Star, offset)),
            '(' => tokens.push(simple(TokenKind::LParen, offset)),
            ')' => tokens.push(simple(TokenKind::RParen, offset)),
            '[' => tokens.push(simple(TokenKind::LBracket, offset)),
            ']' => tokens.push(simple(TokenKind::RBracket, offset)),
            '{' => tokens.push(simple(TokenKind::LBrace, offset)),
            '}' => tokens.push(simple(TokenKind::RBrace, offset)),
            c if is_ident_start(c) => {
                let mut lexeme = String::from(c);
                while let Some((_, next)) = chars.peek() {
                    if is_ident_continue(*next) {
                        lexeme.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }

                let kind = match lexeme.as_str() {
                    "let" => TokenKind::Let,
                    "var" => TokenKind::Var,
                    "fn" => TokenKind::Fn,
                    "return" => TokenKind::Return,
                    "import" => TokenKind::Import,
                    "export" => TokenKind::Export,
                    _ => TokenKind::Ident(lexeme),
                };

                tokens.push(Token {
                    kind,
                    start: offset,
                });
            }
            c if c.is_ascii_digit() => {
                let mut lexeme = String::from(c);
                while let Some((_, next)) = chars.peek() {
                    if next.is_ascii_digit() || *next == '.' {
                        lexeme.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }

                let value: f64 = lexeme.parse().map_err(|_| LexError::InvalidNumber {
                    lexeme: lexeme.clone(),
                    offset,
                })?;

                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    start: offset,
                });
            }
            _ => return Err(LexError::UnexpectedChar { ch, offset }),
        }
    }

    Ok(tokens)
}

fn simple(kind: TokenKind, start: usize) -> Token {
    Token { kind, start }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
