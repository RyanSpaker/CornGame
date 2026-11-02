//! CLAUDE (claude-opus-5-5): finds where an inline JS script block ends, without parsing JS.
//!
//! The scanner keeps a stack of contexts. Code counts `{ }`; strings, comments and regex literals
//! are skipped; a template literal `` `...` `` ignores braces except `${`, which pushes code again,
//! so templates nest to any depth. The block ends at a `}` seen at depth 0 in the outermost code.
//!
//! Regex vs division: `/` starts a regex when a value is expected, i.e. not right after an
//! identifier, number, string, `)` or `]` (keywords like `return` expect a value). Postfix `++`/`--`
//! keep the previous state, so `a++ / 2` is division.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unclosed {
    /// the script block itself, or a `${` interpolation
    Block,
    String,
    Template,
    Comment,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanError {
    pub kind: Unclosed,
    /// byte offset in the scanned source where the unclosed construct starts
    pub start: usize,
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unclosed {:?} starting at byte {}", self.kind, self.start)
    }
}

impl std::error::Error for ScanError {}

enum Frame {
    Code { depth: u32, start: usize },
    Template { start: usize },
}

/// Keywords after which an expression (so possibly a regex) follows.
const EXPR_KEYWORDS: &[&str] = &[
    "return", "typeof", "instanceof", "in", "of", "new", "delete", "void", "throw", "case", "do",
    "else", "yield", "await",
];

/// `src` starts just after a script block's opening `{`. Returns the byte offset of its closing `}`.
pub fn script_end(src: &str) -> Result<usize, ScanError> {
    let b = src.as_bytes();
    let mut stack = vec![Frame::Code { depth: 0, start: 0 }];
    // true when a `/` here would start a regex
    let mut regex_ok = true;
    // first byte of the previous token (ignoring whitespace/comments), to spot `.prop` names
    let mut last = 0u8;
    let mut i = 0;

    loop {
        let outermost = stack.len() == 1;
        match stack.last_mut().unwrap() {
            Frame::Template { start } => match b.get(i) {
                None => return Err(ScanError { kind: Unclosed::Template, start: *start }),
                Some(b'\\') => i += 2,
                Some(b'`') => {
                    stack.pop();
                    i += 1;
                    regex_ok = false;
                    last = b'`';
                }
                Some(b'$') if b.get(i + 1) == Some(&b'{') => {
                    stack.push(Frame::Code { depth: 0, start: i });
                    i += 2;
                    regex_ok = true;
                    last = b'{';
                }
                Some(_) => i += 1,
            },
            Frame::Code { depth, start } => {
                let Some(&c) = b.get(i) else {
                    return Err(ScanError { kind: Unclosed::Block, start: *start });
                };
                match c {
                    _ if c.is_ascii_whitespace() => {
                        i += 1;
                        continue;
                    }
                    b'/' if b.get(i + 1) == Some(&b'/') => {
                        i = b[i..].iter().position(|&x| x == b'\n').map_or(b.len(), |p| i + p);
                        continue;
                    }
                    b'/' if b.get(i + 1) == Some(&b'*') => {
                        let end = src[i + 2..].find("*/").ok_or(ScanError { kind: Unclosed::Comment, start: i })?;
                        i += 2 + end + 2;
                        continue;
                    }
                    b'{' => {
                        *depth += 1;
                        i += 1;
                        regex_ok = true;
                    }
                    b'}' => {
                        if *depth == 0 {
                            if outermost {
                                return Ok(i);
                            }
                            // end of a `${ }`, back in the template
                            stack.pop();
                            i += 1;
                            continue;
                        }
                        *depth -= 1;
                        i += 1;
                        regex_ok = true;
                    }
                    b')' | b']' => {
                        i += 1;
                        regex_ok = false;
                    }
                    b'\'' | b'"' => {
                        i = skip_string(b, i)?;
                        regex_ok = false;
                    }
                    b'`' => {
                        stack.push(Frame::Template { start: i });
                        i += 1;
                    }
                    b'/' if regex_ok => {
                        i = skip_regex(b, i)?;
                        regex_ok = false;
                    }
                    b'+' | b'-' if b.get(i + 1) == Some(&c) => i += 2,
                    _ if is_word(c) || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) => {
                        let s = i;
                        i += 1;
                        while i < b.len() && (is_word(b[i]) || (b[i] == b'.' && b[s].is_ascii_digit())) {
                            i += 1;
                        }
                        // a property name after `.` is never a keyword
                        regex_ok = last != b'.' && EXPR_KEYWORDS.contains(&&src[s..i]);
                    }
                    _ => {
                        i += 1;
                        regex_ok = true;
                    }
                }
                last = c;
            }
        }
    }
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'#' || c >= 0x80
}

/// `b[i]` is the opening quote. Returns the offset after the closing quote.
fn skip_string(b: &[u8], start: usize) -> Result<usize, ScanError> {
    let quote = b[start];
    let mut i = start + 1;
    loop {
        match b.get(i) {
            None | Some(b'\n') => return Err(ScanError { kind: Unclosed::String, start }),
            Some(b'\\') => i += 2,
            Some(&c) if c == quote => return Ok(i + 1),
            Some(_) => i += 1,
        }
    }
}

/// `b[i]` is the opening `/`. Returns the offset after the flags.
fn skip_regex(b: &[u8], start: usize) -> Result<usize, ScanError> {
    let mut i = start + 1;
    let mut class = false;
    loop {
        match b.get(i) {
            None | Some(b'\n') => return Err(ScanError { kind: Unclosed::Regex, start }),
            Some(b'\\') => i += 1,
            Some(b'[') => class = true,
            Some(b']') => class = false,
            Some(b'/') if !class => break,
            Some(_) => {}
        }
        i += 1;
    }
    i += 1;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    Ok(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// script body followed by the closing brace and trailing scene text
    fn end(body: &str) -> usize {
        let src = format!("{body}}} .after {{}}");
        let e = script_end(&src).unwrap_or_else(|e| panic!("{e} in {body:?}"));
        assert_eq!(e, body.len(), "wrong end in {body:?}");
        e
    }

    fn err(body: &str) -> ScanError {
        script_end(body).expect_err(body)
    }

    #[test]
    fn braces() {
        end("");
        end(" function f() { if (x) { return {a: 1}; } } ");
    }

    #[test]
    fn strings_and_comments() {
        end(r#" let a = "}"; let b = '{'; let c = "\"}"; "#);
        end(" // don't } close\n x = 1; ");
        end(" /* } { */ x = 1; ");
    }

    #[test]
    fn templates() {
        end(" `a { } ${ b } }` ");
        end(" `a { ${ items.map(x => `<${x.tag}>{${x.body}}</${x.tag}>`).join(\"\") } }` ");
        end(" `${ f({ a: 1 }) }` ");
        end(" `${ `${ `${ 1 }` }` }` ");
        end(r" `\${ not code } \` still template` ");
    }

    #[test]
    fn regex_vs_division() {
        end(" let r = /[{]/g; ");
        end(" let r = /\\}/; ");
        end(" if (x) return /}/.test(s); ");
        end(" let a = b / c / d; ");
        end(" let a = f(x) / 2 + arr[0] / 3; ");
        end(" let a = i++ / 2; ");
        end(" let a = x.return / 2 / 3; ");
        end(" let a = 1.5 / 2; ");
        end(" if (a) {} /}/.test(s); ");
    }

    #[test]
    fn unclosed() {
        assert_eq!(err(" { ").kind, Unclosed::Block);
        assert_eq!(err(" `${ a ").kind, Unclosed::Block);
        assert_eq!(err(" ` ").kind, Unclosed::Template);
        assert_eq!(err(" \"abc\n } ").kind, Unclosed::String);
        assert_eq!(err(" /* } ").kind, Unclosed::Comment);
        assert_eq!(err(" x = /abc\n } ").kind, Unclosed::Regex);
        assert_eq!(err(" a `b ${ c ").start, 6);
    }
}
