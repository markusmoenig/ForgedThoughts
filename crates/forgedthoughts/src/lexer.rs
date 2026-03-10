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
    Ident(String),
    Number(f64),
    Equal,
    Semicolon,
    Colon,
    Comma,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    LBrace,
    RBrace,
}

#[derive(Debug, Error)]
pub enum LexError {
    #[error("unexpected character '{ch}' at byte {offset}")]
    UnexpectedChar { ch: char, offset: usize },
    #[error("invalid number '{lexeme}' at byte {offset}")]
    InvalidNumber { lexeme: String, offset: usize },
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let mut chars = input.char_indices().peekable();
    let mut tokens = Vec::new();

    while let Some((offset, ch)) = chars.next() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {}
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
            '*' => tokens.push(simple(TokenKind::Star, offset)),
            '(' => tokens.push(simple(TokenKind::LParen, offset)),
            ')' => tokens.push(simple(TokenKind::RParen, offset)),
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
