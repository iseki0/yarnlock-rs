use crate::tokens::{Token, TokenWrapper};

fn index_of_char(input: &[u8], start: usize, target: u8) -> Result<usize, ()> {
    input[start..].iter().position(|&x| x == target).map_or(Err(()), |v| Ok(v + start))
}

fn measure_indent_len(input: &[u8]) -> usize {
    input.iter().position(|&x| x != b' ').unwrap_or(input.len())
}

fn measure_quoted_string(input: &[u8]) -> Result<usize, &'static str> {
    assert_eq!(input[0], b'\"');
    for i in 1..input.len() {
        if input[i] == b'\"' {
            let escaped = input[i - 1] == b'\\' && input[i - 2] != b'\\';
            if !escaped {
                return Ok(i + 1);
            }
        }
    }
    Err("Unexpected EOF")
}

fn parse_number(input: &[u8]) -> Result<(i64, usize), &'static str> {
    let end = input.iter().position(|x| !x.is_ascii_digit()).unwrap_or(input.len());
    let s = std::str::from_utf8(&input[..end]).unwrap();
    s.parse::<i64>().map_or(Err("Number parse failed"), |v| Ok((v, end)))
}

fn match_str_prefix(input: &[u8], prefix: &str) -> bool {
    let pb = prefix.as_bytes();
    if pb.len() > input.len() { return false; }
    &input[..pb.len()] == pb
}

fn measure_unquoted_string(input: &[u8]) -> usize {
    input.iter().position(|&x| x == b':' || x == b' ' || x == b'\n' || x == b'\r' || x == b',').unwrap_or(input.len())
}

/// Tokenize the input yarn lock data.
///
/// Translated from [https://github.com/yarnpkg/yarn/blob/master/src/lockfile/parse.js#L50](https://github.com/yarnpkg/yarn/blob/7cafa512a777048ce0b666080a24e80aae3d66a9/src/lockfile/parse.js#L50)
#[allow(clippy::cast_precision_loss)]
#[allow(clippy::too_many_lines)]
pub fn tokenize(input: &[u8]) -> Result<Vec<TokenWrapper>, Error> {
    let mut input = input;
    let mut line = 1;
    let mut col = 0;
    let mut last_new_line = true;
    let mut tokens: Vec<TokenWrapper> = vec![];

    macro_rules! commit {
        ($t: expr) => {tokens.push(TokenWrapper { col, line, token:$t })};
    }
    macro_rules! error {
        ($reason: expr) => {return Err(Error { line, col, reason: $reason });};
    }
    while !input.is_empty() {
        let mut chop = 0;
        let ch = input[0];
        match ch {
            b'\r' | b'\n' => {
                commit!(Token::NewLine);
                chop += 1;
                if input.len() > 1 && input[1] == b'\n' {
                    chop += 1;
                }
                line += 1;
                col = 0;
                input = &input[chop..];
                last_new_line = true;
                continue;
            }
            b'#' => {
                let next_new_line = match index_of_char(input, 1, b'\n') {
                    Ok(idx) => { idx }
                    Err(()) => { input.len() }
                };
                commit!(Token::Comment(&input[1..next_new_line]));
                chop += next_new_line;
            }
            b' ' => {
                if last_new_line {
                    let indent_size = measure_indent_len(input);
                    if indent_size % 2 != 0 {
                        error!("Invalid number of spaces");
                    } else {
                        commit!(Token::Indent(indent_size));
                        chop += indent_size;
                    }
                } else {
                    chop += 1;
                }
            }
            b'"' => {
                match measure_quoted_string(input) {
                    Ok(len) => {
                        commit!(Token::String(&input[..len]));
                        chop += len;
                    }
                    Err(reason) => {
                        error!(reason);
                    }
                }
            }
            b':' => {
                commit!(Token::Colon);
                chop += 1;
            }
            b',' => {
                commit!(Token::Comma);
                chop += 1;
            }
            _ => {
                if match_str_prefix(input, "true") {
                    commit!(Token::Bool(true));
                    chop += 4;
                } else if match_str_prefix(input, "false") {
                    commit!(Token::Bool(false));
                    chop += 5;
                } else if ch.is_ascii_digit() {
                    match parse_number(input) {
                        Ok((n, len)) => {
                            commit!(Token::Number(n as f64));
                            chop += len;
                        }
                        Err(reason) => {
                            error!(reason);
                        }
                    }
                } else if ch.is_ascii_alphabetic() || ch == b'/' || ch == b'.' || ch == b'_' || ch == b'-' {
                    let len = measure_unquoted_string(input);
                    commit!(Token::String(&input[..len]));
                    chop += len;
                } else {
                    commit!(Token::Invalid);
                }
            }
        }
        if chop == 0 {
            error!("infinite");
        }
        last_new_line = false;
        col += i32::try_from(chop).unwrap();
        input = &input[chop..];
    }
    commit!(Token::Eof);
    Ok(tokens)
}

#[derive(Debug)]
pub struct Error {
    pub line: i32,
    pub col: i32,
    pub reason: &'static str,
}

#[cfg(test)]
mod tests {
    use std::cmp::min;

    use crate::lexer::tokenize;
    use crate::tokens::Token::*;
    use crate::tokens::TokenWrapper;

    #[test]
    fn hello() {
        assert_eq!(1, 1);
        println!("Hello world! test passed!");
    }

    #[test]
    fn test_tokenize0() {
        let r = do_test(include_bytes!("test.lock.0"));
        let expected = vec![
            TokenWrapper { col: 0, line: 1, token: Comment(" THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.".as_bytes()) },
            TokenWrapper { col: 64, line: 1, token: NewLine },
            TokenWrapper { col: 0, line: 2, token: Comment(b" yarn lockfile v1") },
            TokenWrapper { col: 18, line: 2, token: NewLine },
            TokenWrapper { col: 0, line: 3, token: NewLine },
            TokenWrapper { col: 0, line: 4, token: String(b"\"@colors/colors@1.5.0\"") },
            TokenWrapper { col: 22, line: 4, token: Colon },
            TokenWrapper { col: 23, line: 4, token: NewLine },
            TokenWrapper { col: 0, line: 5, token: Indent(2) },
            TokenWrapper { col: 2, line: 5, token: String(b"version") },
            TokenWrapper { col: 10, line: 5, token: String(b"\"1.5.0\"") },
            TokenWrapper { col: 17, line: 5, token: NewLine },
            TokenWrapper { col: 0, line: 6, token: Indent(2) },
            TokenWrapper { col: 2, line: 6, token: Eof },
        ];
        assert_eq!(expected, r);
    }

    #[test]
    fn test_tokenize1() {
        let actual = &do_test(include_bytes!("test.lock.1"))[..20];
        let expected = vec![
            TokenWrapper { col: 0, line: 1, token: Comment(" THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.".as_bytes()) },
            TokenWrapper { col: 64, line: 1, token: NewLine },
            TokenWrapper { col: 0, line: 2, token: Comment(b" yarn lockfile v1") },
            TokenWrapper { col: 18, line: 2, token: NewLine },
            TokenWrapper { col: 0, line: 3, token: NewLine },
            TokenWrapper { col: 0, line: 4, token: String(b"\"@colors/colors@1.5.0\"") },
            TokenWrapper { col: 22, line: 4, token: Colon },
            TokenWrapper { col: 23, line: 4, token: NewLine },
            TokenWrapper { col: 0, line: 5, token: Indent(2) },
            TokenWrapper { col: 2, line: 5, token: String(b"version") },
            TokenWrapper { col: 10, line: 5, token: String(b"\"1.5.0\"") },
            TokenWrapper { col: 17, line: 5, token: NewLine },
            TokenWrapper { col: 0, line: 6, token: Indent(2) },
            TokenWrapper { col: 2, line: 6, token: String(b"resolved") },
            TokenWrapper { col: 11, line: 6, token: String("\"https://registry.yarnpkg.com/@colors/colors/-/colors-1.5.0.tgz#bb504579c1cae923e6576a4f5da43d25f97bdbd9\"".as_bytes()) },
            TokenWrapper { col: 116, line: 6, token: NewLine },
            TokenWrapper { col: 0, line: 7, token: Indent(2) },
            TokenWrapper { col: 2, line: 7, token: String(b"integrity") },
            TokenWrapper { col: 12, line: 7, token: String("sha512-ooWCrlZP11i8GImSjTHYHLkvFDP48nS4+204nGb1RiX/WXYHmJA2III9/e2DWVabCESdW7hBAEzHRqUn9OUVvQ==".as_bytes()) },
            TokenWrapper { col: 107, line: 7, token: NewLine },
        ];
        assert_eq!(expected.len(), actual.len());
        for i in 0..expected.len() {
            assert_eq!(expected[i], actual[i], "i={i}");
        }
    }

    #[test]
    fn test_tokenize_indents() {
        let actual = do_test(b"  \r\n    \n\n  ");
        let expected = vec![
            TokenWrapper { col: 0, line: 1, token: Indent(2) },
            TokenWrapper { col: 2, line: 1, token: NewLine },
            TokenWrapper { col: 0, line: 2, token: Indent(4) },
            TokenWrapper { col: 4, line: 2, token: NewLine },
            TokenWrapper { col: 0, line: 3, token: Indent(2) },
            TokenWrapper { col: 2, line: 3, token: Eof },
        ];
        assert_eq!(expected, actual);
    }

    fn do_test(input: &[u8]) -> Vec<TokenWrapper> {
        let v = tokenize(input).unwrap();
        println!("tokens: {}", v.len());
        println!("vec![");
        for x in &v[0..min(20, v.len())] {
            println!("    {x:?},");
        }
        println!("];");
        v
    }
}
