use std::fmt;
use std::fmt::{Debug, Formatter};

#[derive(PartialEq)]
pub(crate) enum Token<'t> {
    Bool(bool),
    String(&'t [u8]),
    Number(f64),
    Indent(usize),
    Comment(&'t [u8]),
    EOF,
    Colon,
    NewLine,
    Invalid,
    Comma,
}


macro_rules! u8quote {
        ($v: expr) => {std::str::from_utf8($v).unwrap()};
    }

impl<'t> Debug for Token<'t> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Token::Bool(b) => write!(f, "Bool({})", b),
            Token::String(s) => write!(f, "String({:?}.as_bytes())", u8quote!(s)),
            Token::Number(n) => write!(f, "Number({})", n),
            Token::Indent(i) => write!(f, "Indent({})", i),
            Token::EOF => write!(f, "EOF"),
            Token::Colon => write!(f, "Colon"),
            Token::NewLine => write!(f, "NewLine"),
            Token::Comment(s) => write!(f, "Comment({:?}.as_bytes())", u8quote!(s)),
            Token::Invalid => write!(f, "Invalid"),
            Token::Comma => write!(f, "Comma"),
        }
    }
}


#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) struct TokenWrapper<'t> {
    pub col: i32,
    pub line: i32,
    pub token: Token<'t>,
}

