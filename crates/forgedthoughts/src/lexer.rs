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
    For,
    In,
    Ident(String),
    String(String),
    HexColor(String),
    Number(f64),
    Equal,
    Semicolon,
    Colon,
    Comma,
    Dot,
    DotDot,
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
    // Collect into a Vec so we can look two chars ahead.
    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let mut i = 0;
    let mut tokens = Vec::new();

    while i < chars.len() {
        let (offset, ch) = chars[i];
        i += 1;

        match ch {
            ' ' | '\t' | '\r' | '\n' => {}

            '"' => {
                let mut value = String::new();
                let mut closed = false;
                while i < chars.len() {
                    let (_, next) = chars[i];
                    i += 1;
                    match next {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => {
                            if i < chars.len() {
                                let (_, escaped) = chars[i];
                                i += 1;
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
                while i < chars.len() && chars[i].1.is_ascii_hexdigit() {
                    lexeme.push(chars[i].1);
                    i += 1;
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
                if i < chars.len() && chars[i].1 == '/' {
                    // Line comment — consume until newline.
                    while i < chars.len() && chars[i].1 != '\n' {
                        i += 1;
                    }
                } else {
                    tokens.push(simple(TokenKind::Slash, offset));
                }
            }

            '.' => {
                // Check for '..' (range operator).
                if i < chars.len() && chars[i].1 == '.' {
                    i += 1;
                    tokens.push(simple(TokenKind::DotDot, offset));
                } else {
                    tokens.push(simple(TokenKind::Dot, offset));
                }
            }

            '=' => tokens.push(simple(TokenKind::Equal, offset)),
            ';' => tokens.push(simple(TokenKind::Semicolon, offset)),
            ':' => tokens.push(simple(TokenKind::Colon, offset)),
            ',' => tokens.push(simple(TokenKind::Comma, offset)),
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
                while i < chars.len() && is_ident_continue(chars[i].1) {
                    lexeme.push(chars[i].1);
                    i += 1;
                }
                let kind = match lexeme.as_str() {
                    "let" => TokenKind::Let,
                    "var" => TokenKind::Var,
                    "fn" => TokenKind::Fn,
                    "return" => TokenKind::Return,
                    "import" => TokenKind::Import,
                    "export" => TokenKind::Export,
                    "for" => TokenKind::For,
                    "in" => TokenKind::In,
                    _ => TokenKind::Ident(lexeme),
                };
                tokens.push(Token { kind, start: offset });
            }

            c if c.is_ascii_digit() => {
                let mut lexeme = String::from(c);
                while i < chars.len() {
                    let next_ch = chars[i].1;
                    if next_ch.is_ascii_digit() {
                        lexeme.push(next_ch);
                        i += 1;
                    } else if next_ch == '.' {
                        // Only consume '.' as a decimal point if the character after it
                        // is a digit (not another '.', which would be the '..' range op).
                        let after = chars.get(i + 1).map(|&(_, c)| c);
                        if after.map_or(false, |c| c.is_ascii_digit()) {
                            lexeme.push(next_ch);
                            i += 1;
                        } else {
                            break;
                        }
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
