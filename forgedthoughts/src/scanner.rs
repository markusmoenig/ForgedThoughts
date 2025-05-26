use rustc_hash::FxHashMap;

#[allow(dead_code)]
#[derive(Eq, PartialEq, Debug, Copy, Clone, Hash)]
pub enum TokenType {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,
    Dollar,
    Colon,
    Apostrophe,

    LineFeed,
    Space,
    Quotation,
    Unknown,
    SingeLineComment,
    HexColor,

    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals.
    Identifier,
    String,
    Number,

    // Keywords.
    And,
    Class,
    Else,
    False,
    For,
    Fn,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Let,
    While,
    CodeBlock,

    Error,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenType,
    pub line: usize,
    pub lexeme: String,
    pub indent: usize,
}

#[allow(dead_code)]
impl Token {
    pub fn synthetic(text: String) -> Token {
        Token {
            kind: TokenType::Error,
            lexeme: text,
            line: 0,
            indent: 0,
        }
    }
}

#[allow(dead_code)]
pub struct Scanner {
    keywords: FxHashMap<&'static str, TokenType>,
    code: String,
    start: usize,
    current: usize,
    line: usize,
    indent: usize,
}

#[allow(dead_code)]
impl Scanner {
    pub fn new(code: String) -> Scanner {
        let mut keywords = FxHashMap::default();
        keywords.insert("and", TokenType::And);
        keywords.insert("class", TokenType::Class);
        keywords.insert("else", TokenType::Else);
        keywords.insert("false", TokenType::False);
        keywords.insert("for", TokenType::For);
        keywords.insert("fn", TokenType::Fn);
        keywords.insert("if", TokenType::If);
        keywords.insert("nil", TokenType::Nil);
        keywords.insert("or", TokenType::Or);
        keywords.insert("print", TokenType::Print);
        keywords.insert("return", TokenType::Return);
        keywords.insert("super", TokenType::Super);
        keywords.insert("this", TokenType::This);
        keywords.insert("true", TokenType::True);
        keywords.insert("let", TokenType::Let);
        keywords.insert("while", TokenType::While);

        Scanner {
            keywords,
            code,
            start: 0,
            current: 0,
            line: 1,
            indent: 0,
        }
    }

    pub fn scan_token(&mut self, allow_whitespace: bool) -> Token {
        self.skip_whitespace(allow_whitespace);
        self.start = self.current;
        if self.is_at_end() {
            return self.make_token(TokenType::Eof);
        }

        match self.advance() {
            b' ' if allow_whitespace => self.make_token(TokenType::Space),
            b'\n' if allow_whitespace => self.make_token(TokenType::LineFeed),
            b'-' if self.matches(b'-') => self.make_token(TokenType::CodeBlock),
            b'(' => self.make_token(TokenType::LeftParen),
            b')' => self.make_token(TokenType::RightParen),
            b'{' => self.make_token(TokenType::LeftBrace),
            b'}' => self.make_token(TokenType::RightBrace),
            b'[' => self.make_token(TokenType::LeftBracket),
            b']' => self.make_token(TokenType::RightBracket),
            b'$' => self.make_token(TokenType::Dollar),
            b';' => self.make_token(TokenType::Semicolon),
            b',' => self.make_token(TokenType::Comma),
            b'.' => self.make_token(TokenType::Dot),
            b'-' => self.make_token(TokenType::Minus),
            b'+' => self.make_token(TokenType::Plus),
            b'\'' => self.make_token(TokenType::Apostrophe),
            b'/' if self.matches(b'/') => self.single_line_comment(),
            b'/' => self.make_token(TokenType::Slash),
            b'*' => self.make_token(TokenType::Star),
            b':' => self.make_token(TokenType::Colon),
            b'!' if self.matches(b'=') => self.make_token(TokenType::BangEqual),
            b'!' => self.make_token(TokenType::Bang),
            b'=' if self.matches(b'=') => self.make_token(TokenType::EqualEqual),
            b'=' => self.make_token(TokenType::Equal),
            b'<' if self.matches(b'=') => self.make_token(TokenType::LessEqual),
            b'<' => self.make_token(TokenType::Less),
            b'>' if self.matches(b'=') => self.make_token(TokenType::GreaterEqual),
            b'>' => self.make_token(TokenType::Greater),
            b'"' => self.string(),
            b'`' => self.string2(),
            b'#' => self.hex_color(),
            c if is_digit(c) => self.number(),
            c if is_alpha(c) => self.identifier(),
            _ => self.make_token(TokenType::Unknown), //self.error_token("Unexpected character."),
        }
    }

    fn is_at_end(&self) -> bool {
        self.current == self.code.len()
    }

    fn lexeme(&self) -> String {
        self.code[self.start..self.current].to_string()
    }

    fn make_token(&self, kind: TokenType) -> Token {
        Token {
            kind,
            lexeme: self.lexeme(),
            line: self.line,
            indent: self.indent,
        }
    }

    pub fn peek(&self) -> u8 {
        if self.is_at_end() {
            0
        } else {
            self.code.as_bytes()[self.current]
        }
    }
    pub fn peek_next(&self) -> u8 {
        if self.current > self.code.len() - 2 {
            b'\0'
        } else {
            self.code.as_bytes()[self.current + 1]
        }
    }

    fn error_token(&self, message: String) -> Token {
        Token {
            kind: TokenType::Error,
            lexeme: message,
            line: self.line,
            indent: self.indent,
        }
    }

    fn advance(&mut self) -> u8 {
        let char = self.peek();
        self.current += 1;
        char
    }

    fn matches(&mut self, expected: u8) -> bool {
        if self.is_at_end() || self.peek() != expected {
            false
        } else {
            self.current += 1;
            true
        }
    }

    fn skip_whitespace(&mut self, allow_whitespace: bool) {
        let mut after_lf = false;
        while !self.is_at_end() {
            match self.peek() {
                b' ' if !allow_whitespace => {
                    if after_lf {
                        self.indent += 1;
                    }
                    self.advance();
                }
                b'\r' | b'\t' => {
                    self.advance();
                }
                b'\n' => {
                    self.line += 1;
                    self.advance();
                    self.indent = 0;
                    after_lf = true;
                }
                b'/' if self.peek_next() == b'/' => {
                    while self.peek() != b'\n' && !self.is_at_end() {
                        self.advance();
                    }
                }
                _ => return,
            }
        }
    }

    pub fn scanline(&mut self, advance: usize) -> String {
        let mut string = "".to_string();

        let start = self.current + advance;
        while !self.is_at_end() {
            match self.peek() {
                b'\n' => {
                    string = self.code[start..self.current].to_string();
                    self.advance();
                    self.line += 1;
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
        string
    }

    pub fn scan_indention_block(
        &mut self,
        advance: usize,
        min_indent: usize,
    ) -> Result<String, String> {
        let mut string = "".to_string();

        let start = self.current + advance;

        let mut newline = false;
        let mut indent = min_indent + 1;
        while !self.is_at_end() {
            match self.peek() {
                b' ' => {
                    if newline {
                        indent += 1;
                    }
                    self.advance();
                }
                b'\n' => {
                    newline = true;
                    self.advance();
                    self.line += 1;
                    indent = 0;
                }
                _ => {
                    newline = false;
                    if indent <= min_indent {
                        //                        return Err("Indention of function block too small.".to_owned());
                        string = self.code[start..self.current].to_string();
                        self.current -= 1;
                        break;
                    }
                    self.advance();
                }
            }
        }
        Ok(string)
    }

    fn string(&mut self) -> Token {
        let b_current = self.current;

        while self.peek() != b'"' && !self.is_at_end() {
            if self.peek() == b'\n' {
                //self.line += 1;
                self.current = b_current;
                return self.make_token(TokenType::Quotation);
            }
            self.advance();
        }

        if self.is_at_end() {
            //self.error_token("Unterminated string.");
            self.current = b_current;
            self.make_token(TokenType::Quotation)
        } else {
            self.advance();
            self.make_token(TokenType::String)
        }
    }

    fn string2(&mut self) -> Token {
        let b_current = self.current;

        while self.peek() != b'`' && !self.is_at_end() {
            if self.peek() == b'\n' {
                //self.line += 1;
                self.current = b_current;
                return self.make_token(TokenType::Quotation);
            }
            self.advance();
        }

        if self.is_at_end() {
            //self.error_token("Unterminated string.")
            self.current = b_current;
            self.make_token(TokenType::Quotation)
        } else {
            self.advance();
            self.make_token(TokenType::String)
        }
    }

    fn number(&mut self) -> Token {
        while is_digit(self.peek()) {
            self.advance();
        }

        if self.peek() == b'.' && is_digit(self.peek_next()) {
            self.advance();
            while is_digit(self.peek()) {
                self.advance();
            }
        }

        self.make_token(TokenType::Number)
    }

    fn hex_color(&mut self) -> Token {
        while !self.is_at_end()
            && self.peek() != b'\n'
            && self.peek() != b','
            && self.peek() != b';'
        {
            self.advance();
        }
        self.make_token(TokenType::HexColor)
    }

    fn single_line_comment(&mut self) -> Token {
        while !self.is_at_end() && self.peek() != b'\n' {
            self.advance();
        }
        self.make_token(TokenType::SingeLineComment)
    }

    fn identifier(&mut self) -> Token {
        while is_alpha(self.peek()) || is_digit(self.peek()) {
            self.advance();
        }
        self.make_token(self.identifier_type())
    }

    fn identifier_type(&self) -> TokenType {
        self.keywords
            .get(self.lexeme().as_str())
            .cloned()
            .unwrap_or(TokenType::Identifier)
    }
}

fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

fn is_alpha(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}
