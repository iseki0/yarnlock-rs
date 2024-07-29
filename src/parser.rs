use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::rc::Rc;

use crate::lexer::tokenize;
use crate::tokens::{Token, TokenWrapper};

const VERSION_LINE_TEXT: &str = "yarn lockfile v";

fn version_match(chars: &[u8]) -> Option<i32> {
    match std::str::from_utf8(chars) {
        Ok(s) => {
            let s = s.trim();
            if !s.starts_with(VERSION_LINE_TEXT) {
                return None;
            }
            s[VERSION_LINE_TEXT.len()..].parse::<i32>().ok()
        }
        Err(_) => None
    }
}

#[derive(Clone, Debug)]
pub enum Value {
    String(Rc<String>),
    Number(f64),
    Boolean(bool),
    Object(HashMap<String, Value>),
    Null,
}

/// Parsing error.
///
/// This error is returned when the parser encounters an error while parsing the input.
#[derive(Debug, Clone, Copy)]
pub struct Error {
    /// The line number where the error occurred.
    pub line: i32,
    /// The column number where the error occurred.
    pub col: i32,
    /// The reason for the error.
    pub reason: &'static str,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Parsing error[{}:{}]: ", self.line, self.col).and_then(|()| write!(f, "{}", self.reason))
    }
}

struct Parser<'t> {
    tokens: &'t [TokenWrapper<'t>],
    token_ptr: usize,
    cur: &'t TokenWrapper<'t>,
}

/// Parse the input yarn lock data.
///
/// Translated from [https://github.com/yarnpkg/yarn/blob/master/src/lockfile/parse.js#L50](https://github.com/yarnpkg/yarn/blob/7cafa512a777048ce0b666080a24e80aae3d66a9/src/lockfile/parse.js#L50)
/// Keep code-style consistent with the original code.
/// 
/// # Errors
/// - [`Error`]: When parsing failed
/// 
pub fn parse(input: &[u8]) -> Result<Value, Error> {
    let tokens = &tokenize(input).map_err(|e| Error { line: e.line, col: e.col, reason: e.reason })?;
    let mut parser = Parser {
        tokens,
        token_ptr: 0,
        cur: &tokens[0],
    };
    parser.next()?;
    parser.parse(0)
}

impl<'t> Parser<'t> {
    fn next(&mut self) -> Result<&'t TokenWrapper<'t>, Error> {
        loop {
            if self.token_ptr >= self.tokens.len() {
                return Err(Error { line: 0, col: 0, reason: "Unexpected end of input" });
            }
            let tk = &self.tokens[self.token_ptr];
            self.token_ptr += 1;
            if let Token::Comment(cm) = tk.token {
                match version_match(cm) {
                    None => { continue; }
                    Some(v) => {
                        if v > 1 {
                            return Err(Error { line: 0, col: 0, reason: "Unsupported lockfile version" });
                        }
                        continue;
                    }
                }
            };
            self.cur = tk;
            return Ok(tk);
        }
    }

    fn parse(&mut self, indent: usize) -> Result<Value, Error> {
        let mut map: HashMap<String, Value> = HashMap::new();
        macro_rules! unquote_string_token {
            ($token: expr, $s:expr) => {
                unquote_string($s).map_err(|s| Error { line: $token.line, col: $token.col, reason: s })
            };
        }
        macro_rules! key_check {
            ($token: expr, $s: expr) => {
                if $s.is_empty() {
                    return Err(Error { line: $token.line, col: $token.col, reason: "Expected a key" });
                }
            };
        }
        loop {
            let prop_token = self.cur;
            match prop_token.token {
                Token::NewLine => {
                    let next_token = self.next()?;
                    if indent == 0 {
                        // if we have indentation 0, then the next token doesn't matter
                        continue;
                    }
                    match next_token.token {
                        Token::Indent(n) => {
                            if n == indent {
                                // all is good, the indent is on our level
                                _ = self.next();
                            } else {
                                // the indentation is less than our level
                                break;
                            }
                        }
                        _ => {
                            // if we have no indentation after a newline then we've gone down a level
                            break;
                        }
                    }
                }
                Token::Indent(n) => {
                    if n == indent {
                        _ = self.next();
                    } else {
                        break;
                    }
                }
                Token::Eof => {
                    break;
                }
                Token::String(s) => {
                    // property key
                    let key = unquote_string_token!(prop_token, s)?;
                    key_check!(prop_token, key);
                    let mut keys = vec![key];
                    _ = self.next()?;
                    // support multiple keys
                    loop {
                        match self.cur.token {
                            Token::Comma => {
                                // skip comma
                                _ = self.next();
                                let key_token = self.cur;
                                match key_token.token {
                                    Token::String(s) => {
                                        let key = unquote_string_token!(key_token, s)?;
                                        key_check!(key_token, key);
                                        keys.push(key);
                                        _ = self.next()?;
                                    }
                                    _ => { return Err(Error { line: key_token.line, col: key_token.col, reason: "Expected string" }) }
                                };
                            }
                            _ => { break; }
                        };
                    };
                    let was_colon = matches!(self.cur.token, Token::Colon);
                    if was_colon {
                        _ = self.next()?;
                    }
                    match self.cur.token {
                        Token::String(u) => {
                            let v = Value::String(Rc::new(unquote_string_token!(self.cur, u)?));
                            for x in keys {
                                map.insert(x, v.clone());
                            };
                            self.next()?;
                        }
                        Token::Number(n) => {
                            for x in keys {
                                map.insert(x, Value::Number(n));
                            };
                            self.next()?;
                        }
                        Token::Bool(b) => {
                            for x in keys {
                                map.insert(x, Value::Boolean(b));
                            };
                            self.next()?;
                        }
                        _ => {
                            if was_colon {
                                let v = self.parse(indent + 2)?;
                                for x in keys {
                                    map.insert(x, v.clone());
                                };
                                if let Token::Indent(_) = self.cur.token {
                                    if indent == 0 { break; }
                                };
                            } else {
                                return Err(Error { line: self.cur.line, col: self.cur.col, reason: unexpected_token_string(&self.cur.token) });
                            }
                        }
                    };
                }
                _ => {
                    return Err(Error { line: prop_token.line, col: prop_token.col, reason: unexpected_token_string(&prop_token.token) });
                }
            }
        };
        Ok(Value::Object(map))
    }
}


const fn unexpected_token_string(token: &Token) -> &'static str {
    match token {
        Token::Bool(_) => "Unexpected token Bool",
        Token::String(_) => "Unexpected token String",
        Token::Number(_) => "Unexpected token Number",
        Token::Indent(_) => "Unexpected token Indent",
        Token::Comment(_) => "Unexpected token Comment",
        Token::Eof => "Unexpected token EOF",
        Token::Colon => "Unexpected token Colon",
        Token::NewLine => "Unexpected token NewLine",
        Token::Invalid => "Unexpected token Invalid",
        Token::Comma => "Unexpected token Comma",
    }
}

fn unquote_string(input: &[u8]) -> Result<String, &'static str> {
    if !input.is_empty() && input[0] == b'"' {
        unquote_json_string(input).ok_or("Invalid JSON string")
    } else {
        std::str::from_utf8(input).map(ToString::to_string).map_err(|_| "Invalid UTF-8 string")
    }
}

fn unquote_json_string(input: &[u8]) -> Option<String> {
    let Ok(input) = std::str::from_utf8(input) else { return None };
    let mut begin = false;
    let mut chars = input.chars();
    let mut buffer = String::new();
    loop {
        let ch = match chars.next() {
            None => return None,
            Some(ch) => ch
        };
        if !begin {
            if ch == '"' {
                begin = true;
                continue;
            }
            return None;
        }
        match ch {
            '"' => return Some(buffer),
            '\\' => {
                let ch = match chars.next() {
                    None => return None,
                    Some(ch) => ch
                };
                match ch {
                    '"' => buffer.push('"'),
                    '\\' => buffer.push('\\'),
                    '/' => buffer.push('/'),
                    'b' => buffer.push('\u{0008}'),
                    'f' => buffer.push('\u{000c}'),
                    'n' => buffer.push('\n'),
                    'r' => buffer.push('\r'),
                    't' => buffer.push('\t'),
                    'u' => {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            let ch = match chars.next() {
                                None => return None,
                                Some(ch) => ch
                            };
                            hex.push(ch);
                        }
                        let Ok(code) = u32::from_str_radix(&hex, 16) else { return None };
                        match std::char::from_u32(code) {
                            Some(ch) => buffer.push(ch),
                            None => return None
                        }
                    }
                    _ => return None
                }
            }
            _ => buffer.push(ch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_match() {
        let s = "yarn lockfile v1";
        assert_eq!(Some(1), version_match(s.as_bytes()));
    }
    #[test]
    fn test_version_match_negative() {
        let s = "yarn lockfile v";
        assert_eq!(None, version_match(s.as_bytes()));
    }

    #[test]
    fn unquotes_simple_string() {
        let input = "\"hello\"";
        assert_eq!(Some("hello".to_string()), unquote_json_string(input.as_bytes()));
    }

    #[test]
    fn returns_none_for_non_quoted_string() {
        let input = "hello";
        assert_eq!(None, unquote_json_string(input.as_bytes()));
    }

    #[test]
    fn unquotes_string_with_escaped_characters() {
        let input = "\"he\\\"llo\"";
        assert_eq!(Some("he\"llo".to_string()), unquote_json_string(input.as_bytes()));
    }

    #[test]
    fn unquotes_string_with_unicode_characters() {
        let input = "\"\\u0048ello\"";
        assert_eq!(Some("Hello".to_string()), unquote_json_string(input.as_bytes()));
    }

    #[test]
    fn returns_none_for_incomplete_unicode_escape() {
        let input = "\"\\u004\"";
        assert_eq!(None, unquote_json_string(input.as_bytes()));
    }

    #[test]
    fn unquotes_string_with_all_escape_characters() {
        let input = "\"\\\"\\\\\\/\\b\\f\\n\\r\\t\"";
        assert_eq!(Some("\"\\/\u{0008}\u{000c}\n\r\t".to_string()), unquote_json_string(input.as_bytes()));
    }

    #[test]
    fn returns_none_for_string_without_closing_quote() {
        let input = "\"hello";
        assert_eq!(None, unquote_json_string(input.as_bytes()));
    }

    #[test]
    fn returns_none_for_empty_string() {
        let input = "";
        assert_eq!(None, unquote_json_string(input.as_bytes()));
    }

    #[test]
    fn unquotes_empty_quoted_string() {
        let input = "\"\"";
        assert_eq!(Some(String::new()), unquote_json_string(input.as_bytes()));
    }

    #[test]
    fn parse0() {
        println!("{:?}", parse(include_bytes!("test.lock.0")).unwrap());
    }

    #[test]
    fn parse1() {
        println!("{:?}", parse(include_bytes!("test.lock.1")).unwrap());
    }

    #[test]
    fn parse2() {
        println!("{:?}", parse(include_bytes!("test.lock.2")).unwrap());
    }

    #[test]
    fn test_parse_err() {
        const fn foo() -> Result<(), Error> {
            let pe = Error { line: 1, col: 1, reason: "test" };
            Err(pe)
        }

        fn bar() -> Result<(), Box<dyn std::error::Error>> {
            foo()?;
            Ok(())
        }
        
        let pe = Error { line: 1, col: 1, reason: "test" };
        let ee: Box<dyn std::error::Error> = Box::new(pe);
        println!("{ee}");
        
        println!("{:?}", bar());
    }
}
