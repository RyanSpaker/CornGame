//! CLAUDE (claude-opus-5-5): tokens of the scene format. Pull-based, so the parser can hand a
//! script body to [`crate::js_scan`] and resume after it.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    /// identifier, `\` escapes resolved (`Cube\.002` -> `Cube.002`)
    Ident(String),
    Number { value: f64, unit: Option<String> },
    Str(String),
    /// `#abc`: an id in selectors, a hex color in values. `#"a b"` and `#a\ b` are `a b`
    Hash(String),
    /// `::`
    PathSep,
    Punct(char),
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    /// byte offset of the token's start
    pub at: usize,
    /// whitespace or a comment right before it
    pub space_before: bool,
}

/// line and column (1-based) of a byte offset, for messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Loc {
    pub line: u32,
    pub col: u32,
}

impl Loc {
    pub fn of(src: &str, at: usize) -> Loc {
        let before = &src[..at.min(src.len())];
        let line = before.matches('\n').count() as u32 + 1;
        let col = before.rsplit('\n').next().unwrap_or("").chars().count() as u32 + 1;
        Loc { line, col }
    }
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

pub struct Lexer<'a> {
    pub src: &'a str,
    pub pos: usize,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '-' || c == '\\' || !c.is_ascii()
}

fn is_ident_char(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { src, pos: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn peek_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    /// skips whitespace and comments, returns whether there were any
    fn skip_space(&mut self) -> Result<bool, (usize, String)> {
        let start = self.pos;
        loop {
            let rest = self.rest();
            if let Some(c) = rest.chars().next().filter(|c| c.is_whitespace()) {
                self.pos += c.len_utf8();
            } else if rest.starts_with("/*") {
                let end = rest[2..].find("*/").ok_or((self.pos, "unclosed comment".to_string()))?;
                self.pos += 2 + end + 2;
            } else if rest.starts_with("//") {
                self.pos += rest.find('\n').unwrap_or(rest.len());
            } else {
                return Ok(self.pos != start);
            }
        }
    }

    pub fn next(&mut self) -> Result<Token, (usize, String)> {
        let space_before = self.skip_space()?;
        let at = self.pos;
        let token = |tok| Ok(Token { tok, at, space_before });
        let Some(c) = self.peek_char() else { return token(Tok::Eof) };
        let rest = self.rest();

        if rest.starts_with("::") {
            self.pos += 2;
            return token(Tok::PathSep);
        }
        if c == '"' || c == '\'' {
            return token(Tok::Str(self.string(c)?));
        }
        if c == '#' {
            self.pos += 1;
            // `#"ceiling light"`: a name that isn't an identifier, as `#ceiling\ light` is too
            if let Some(quote @ ('"' | '\'')) = self.peek_char() {
                return token(Tok::Hash(self.string(quote)?));
            }
            return token(Tok::Hash(self.ident_chars()));
        }
        let digit_at = |i: usize| rest[i..].starts_with(|c: char| c.is_ascii_digit());
        if c.is_ascii_digit()
            || (c == '.' && digit_at(1))
            || (c == '-' && (digit_at(1) || (rest[1..].starts_with('.') && digit_at(2))))
        {
            return token(self.number());
        }
        // `-` alone (not starting an ident like `-x`) is punctuation
        if is_ident_start(c) && !(c == '-' && !rest[1..].starts_with(is_ident_start)) {
            return token(Tok::Ident(self.ident_chars()));
        }
        self.pos += c.len_utf8();
        token(Tok::Punct(c))
    }

    fn ident_chars(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek_char() {
            if c == '\\' {
                self.pos += 1;
                if let Some(e) = self.peek_char() {
                    out.push(e);
                    self.pos += e.len_utf8();
                }
            } else if is_ident_char(c) {
                out.push(c);
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        out
    }

    fn string(&mut self, quote: char) -> Result<String, (usize, String)> {
        let start = self.pos;
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek_char() else { return Err((start, "unclosed string".into())) };
            self.pos += c.len_utf8();
            match c {
                '\\' => {
                    let Some(e) = self.peek_char() else { continue };
                    self.pos += e.len_utf8();
                    out.push(match e {
                        'n' => '\n',
                        't' => '\t',
                        e => e,
                    });
                }
                '\n' => return Err((start, "unclosed string".into())),
                c if c == quote => return Ok(out),
                c => out.push(c),
            }
        }
    }

    fn number(&mut self) -> Tok {
        let rest = self.rest();
        let mut end = usize::from(rest.starts_with('-'));
        let mut seen_dot = false;
        for c in rest[end..].chars() {
            match c {
                '0'..='9' | '_' => {}
                '.' if !seen_dot && rest[end + 1..].starts_with(|c: char| c.is_ascii_digit()) => seen_dot = true,
                _ => break,
            }
            end += 1;
        }
        let value = rest[..end].replace('_', "").parse().unwrap_or(0.0);
        self.pos += end;
        let unit = match self.peek_char() {
            Some('%') => {
                self.pos += 1;
                Some("%".to_string())
            }
            Some(c) if c.is_alphabetic() => Some(self.ident_chars()),
            _ => None,
        };
        Tok::Number { value, unit }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        let mut lexer = Lexer::new(src);
        let mut out = vec![];
        loop {
            match lexer.next().unwrap().tok {
                Tok::Eof => return out,
                t => out.push(t),
            }
        }
    }

    #[test]
    fn tokens() {
        use Tok::*;
        let id = |s: &str| Ident(s.into());
        assert_eq!(
            toks(r#"avian3d::RigidBody: Static; #Cube\.002 #a\ b #"a b" /* c */ 40px -1.5 50% #ffffff64 "a\"b" .x-y"#),
            vec![
                id("avian3d"), PathSep, id("RigidBody"), Punct(':'), id("Static"), Punct(';'),
                Hash("Cube.002".into()),
                Hash("a b".into()),
                Hash("a b".into()),
                Number { value: 40.0, unit: Some("px".into()) },
                Number { value: -1.5, unit: None },
                Number { value: 50.0, unit: Some("%".into()) },
                Hash("ffffff64".into()),
                Str("a\"b".into()),
                Punct('.'), id("x-y"),
            ]
        );
    }

    #[test]
    fn space_before() {
        let mut lexer = Lexer::new("a:hover a: b");
        let spaces: Vec<bool> = std::iter::from_fn(|| {
            let t = lexer.next().unwrap();
            (t.tok != Tok::Eof).then_some(t.space_before)
        })
        .collect();
        assert_eq!(spaces, [false, false, false, true, false, true]);
    }
}
