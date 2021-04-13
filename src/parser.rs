use crate::ast::Expr;

type Error = String;
type ParseResult<T> = Result<T, Error>;

struct Parser<'a> {
    text: &'a str,
}

pub fn parse(text: &str) -> ParseResult<Expr> {
    let parser = Parser::new(text);
    let expr = parser.parse_full_expr()?;
    log::debug!("parsed {:?} => {}", text, &expr);
    Ok(expr)
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self { text }
    }

    /// Error string with the given message and highlighted text range.
    pub fn error(&self, start: usize, mut end: usize, msg: &str) -> Error {
        if start >= end {
            end = start + 1;
        }
        let end = end.min(self.text.len());
        let before = &self.text[..start];
        let mut highlight = &self.text[start..end];
        let after = &self.text[end..];
        if after.is_empty() && highlight.is_empty() {
            highlight = " ";
        };
        format!(
            "{}: {}\x1b[97;41m{}\x1b[0m{}",
            msg, before, highlight, after
        )
    }

    // Parse a quoted string.
    //  "....."
    //   |    ^ end (return value)
    //   ^ start
    fn parse_quoted_string(&self, start: usize, quote: char) -> ParseResult<(String, usize)> {
        let s = self.text;
        let mut out = String::new();
        let mut escaped = false;
        for (i, ch) in s[start..].char_indices() {
            match (escaped, ch) {
                (false, ch) if ch == quote => {
                    return Ok((out, start + i));
                }
                (false, '\\') => {
                    escaped = true;
                }
                (false, ch) => {
                    out.push(ch);
                }
                (true, 'n') => {
                    out.push('\n');
                    escaped = false;
                }
                (true, 't') => {
                    out.push('\t');
                    escaped = false;
                }
                (true, '"') | (true, '\'') | (true, '\\') => {
                    out.push(ch);
                    escaped = false;
                }
                (true, _) => {
                    out.push('\\');
                    out.push(ch);
                    escaped = false;
                }
            }
        }
        Err(self.error(start, s.len(), "quoted string does not end"))
    }

    // Parse a string.
    //
    //  foobar(a,b,c)
    //  foobar,a,b,c
    //  "...."
    //  |     ^ end (return value)
    //  ^ start
    fn parse_string(&self, start: usize) -> ParseResult<(String, usize)> {
        let s = self.text;
        let mut out = String::new();
        for (i, ch) in s[start..].char_indices() {
            match ch {
                ch if ch.is_whitespace() && out.is_empty() => {
                    continue;
                }
                '"' | '\'' if out.is_empty() => {
                    let (out, end) = self.parse_quoted_string(start + i + 1, ch)?;
                    assert_eq!(s[end..].chars().next(), Some(ch));
                    return Ok((out, end + ch.len_utf8()));
                }
                '(' | ',' | ')' | '"' | '\'' => {
                    let out = out.trim().to_string();
                    return Ok((out, start + i));
                }
                ch => {
                    out.push(ch);
                }
            }
        }
        Ok((out, s.len()))
    }

    // Parse argument list.
    //
    //  (a,b,(c,d))
    //   |        ^ end (return value)
    //   ^ start
    fn parse_args(&self, mut start: usize) -> ParseResult<(Vec<Expr>, usize)> {
        let s = self.text;
        let mut out = Vec::new();
        let mut need_comma = false;
        'outer: loop {
            for (i, ch) in s[start..].char_indices() {
                match ch {
                    ch if ch.is_whitespace() => continue,
                    ',' => {
                        if need_comma {
                            need_comma = false;
                            continue;
                        } else {
                            return Err(self.error(start + i, start + i + 1, "unexpected comma"));
                        }
                    }
                    ')' => {
                        return Ok((out, start + i));
                    }
                    _ => {
                        if need_comma {
                            return Err(self.error(start, start + i + 1, "missing comma (',')"));
                        } else {
                            let (expr, end) = self.parse_expr(start + i)?;
                            out.push(expr);
                            need_comma = true;
                            start = end;
                            continue 'outer;
                        }
                    }
                }
            }
            return Err(self.error(start, s.len(), "missing ')' to end argument list"));
        }
    }

    // Parse an expression.
    fn parse_expr(&self, start: usize) -> ParseResult<(Expr, usize)> {
        let s = self.text;
        let (name, end) = self.parse_string(start)?;
        for (i, ch) in s[end..].char_indices() {
            if ch.is_whitespace() {
                continue;
            };
            if ch == '(' {
                // A function.
                if name.is_empty() {
                    return Err(self.error(start, start + i + 1, "function name cannot be empty"));
                }
                let (args, end) = self.parse_args(end + i + ch.len_utf8())?;
                assert_eq!(s[end..].chars().next(), Some(')'));
                let end = end + ')'.len_utf8();
                assert!(end > start);
                return Ok((Expr::Fn(name.into(), args), end));
            } else {
                break;
            }
        }
        if end == start {
            return Err(self.error(start, start + 1, "unquoted string cannot be empty"));
        }
        Ok((Expr::Name(name.into()), end))
    }

    fn parse_full_expr(&self) -> ParseResult<Expr> {
        let s = self.text;
        let (expr, end) = self.parse_expr(0)?;
        if s[end..].trim().is_empty() {
            Ok(expr)
        } else {
            Err(self.error(end, s.len(), "unexpected content"))
        }
    }
}
