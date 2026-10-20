//! A deliberately tiny arithmetic evaluator for `[[row]]` value transforms.
//!
//! Only numbers, `value`, regex captures (`$1`…) and a few pure functions are
//! allowed. There is no I/O, no property access and no way to call into the
//! host, so a config file arriving from a pull request is safe to evaluate.

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Value,
    Capture(usize),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Call(&'static str, Vec<Expr>),
}

pub fn parse(input: &str) -> Result<Expr, String> {
    let mut parser = Parser {
        tokens: tokenize(input)?,
        pos: 0,
    };
    let expr = parser.sum()?;
    if parser.pos != parser.tokens.len() {
        return Err(format!("unexpected token after expression: {:?}", input));
    }
    Ok(expr)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Ident(String),
    Capture(usize),
    Op(char),
    LParen,
    RParen,
    Comma,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        match ch {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            '+' | '-' | '*' | '/' => {
                tokens.push(Token::Op(ch));
                i += 1;
            }
            '$' => {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && (bytes[end] as char).is_ascii_digit() {
                    end += 1;
                }
                if end == start {
                    return Err("`$` must be followed by a capture number".to_owned());
                }
                let index: usize = input[start..end]
                    .parse()
                    .map_err(|_| "invalid capture index".to_owned())?;
                tokens.push(Token::Capture(index));
                i = end;
            }
            '0'..='9' | '.' => {
                let start = i;
                let mut end = i;
                while end < bytes.len() {
                    let c = bytes[end] as char;
                    if c.is_ascii_digit() || c == '.' {
                        end += 1;
                    } else {
                        break;
                    }
                }
                let value: f64 = input[start..end]
                    .parse()
                    .map_err(|_| format!("invalid number {:?}", &input[start..end]))?;
                tokens.push(Token::Num(value));
                i = end;
            }
            _ if ch.is_ascii_alphabetic() || ch == '_' => {
                let start = i;
                let mut end = i;
                while end < bytes.len() {
                    let c = bytes[end] as char;
                    if c.is_ascii_alphanumeric() || c == '_' {
                        end += 1;
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(input[start..end].to_owned()));
                i = end;
            }
            _ => return Err(format!("unexpected character {ch:?}")),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    fn sum(&mut self) -> Result<Expr, String> {
        let mut left = self.product()?;
        while let Some(Token::Op(op @ ('+' | '-'))) = self.peek().cloned() {
            self.bump();
            let right = self.product()?;
            left = Expr::Bin(
                if op == '+' { BinOp::Add } else { BinOp::Sub },
                Box::new(left),
                Box::new(right),
            );
        }
        Ok(left)
    }

    fn product(&mut self) -> Result<Expr, String> {
        let mut left = self.unary()?;
        while let Some(Token::Op(op @ ('*' | '/'))) = self.peek().cloned() {
            self.bump();
            let right = self.unary()?;
            left = Expr::Bin(
                if op == '*' { BinOp::Mul } else { BinOp::Div },
                Box::new(left),
                Box::new(right),
            );
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if let Some(Token::Op('-')) = self.peek() {
            self.bump();
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.bump() {
            Some(Token::Num(value)) => Ok(Expr::Num(value)),
            Some(Token::Capture(index)) => Ok(Expr::Capture(index)),
            Some(Token::Ident(name)) if name == "value" => Ok(Expr::Value),
            Some(Token::Ident(name)) => {
                let Some(Token::LParen) = self.peek() else {
                    return Err(format!(
                        "unknown name {name:?}; expected `value` or a function"
                    ));
                };
                self.bump();
                let mut args = Vec::new();
                loop {
                    match self.peek() {
                        Some(Token::RParen) => {
                            self.bump();
                            break;
                        }
                        _ => {
                            args.push(self.sum()?);
                            match self.bump() {
                                Some(Token::Comma) => continue,
                                Some(Token::RParen) => break,
                                _ => return Err("expected `,` or `)` in call".to_owned()),
                            }
                        }
                    }
                }
                let function = match name.as_str() {
                    "min" | "max" | "abs" | "log10" | "sqrt" | "round" => {
                        // Leak is fine: config expressions are parsed once.
                        Box::leak(name.into_boxed_str()) as &'static str
                    }
                    _ => return Err(format!("unknown function {name:?}")),
                };
                Ok(Expr::Call(function, args))
            }
            Some(Token::LParen) => {
                let expr = self.sum()?;
                match self.bump() {
                    Some(Token::RParen) => Ok(expr),
                    _ => Err("missing `)`".to_owned()),
                }
            }
            other => Err(format!("unexpected token {other:?}")),
        }
    }
}

impl Expr {
    pub fn eval(&self, value: f64, captures: &[String]) -> f64 {
        match self {
            Expr::Num(number) => *number,
            Expr::Value => value,
            Expr::Capture(index) => captures
                .get(index.saturating_sub(1))
                .and_then(|capture| capture.parse::<f64>().ok())
                .unwrap_or(0.0),
            Expr::Neg(inner) => -inner.eval(value, captures),
            Expr::Bin(op, left, right) => {
                let left = left.eval(value, captures);
                let right = right.eval(value, captures);
                match op {
                    BinOp::Add => left + right,
                    BinOp::Sub => left - right,
                    BinOp::Mul => left * right,
                    BinOp::Div => {
                        if right == 0.0 {
                            left
                        } else {
                            left / right
                        }
                    }
                }
            }
            Expr::Call(name, args) => {
                let values: Vec<f64> = args.iter().map(|arg| arg.eval(value, captures)).collect();
                match (*name, values.as_slice()) {
                    ("min", [a, b]) => a.min(*b),
                    ("max", [a, b]) => a.max(*b),
                    ("min", [a]) => *a,
                    ("max", [a]) => *a,
                    ("abs", [a]) => a.abs(),
                    ("log10", [a]) => a.max(1e-12).log10(),
                    ("sqrt", [a]) => a.max(0.0).sqrt(),
                    ("round", [a]) => a.round(),
                    _ => 0.0,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(source: &str, value: f64, captures: &[&str]) -> f64 {
        let captures: Vec<String> = captures.iter().map(|c| (*c).to_owned()).collect();
        parse(source).unwrap().eval(value, &captures)
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(eval("1 + 2 * 3", 0.0, &[]), 7.0);
        assert_eq!(eval("(1 + 2) * 3", 0.0, &[]), 9.0);
        assert_eq!(eval("-value / 2", 8.0, &[]), -4.0);
    }

    #[test]
    fn captures_multiply_the_measured_value() {
        assert_eq!(eval("value * $1 * 32", 10.0, &["4"]), 1280.0);
        assert!((eval("value / $1", 10.0, &["4"]) - 2.5).abs() < 1e-12);
    }

    #[test]
    fn functions_are_available() {
        assert_eq!(eval("max(value, 10)", 3.0, &[]), 10.0);
        assert_eq!(eval("round(value)", 3.4, &[]), 3.0);
        assert!((eval("log10(value)", 1000.0, &[]) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn rejects_unknown_names() {
        assert!(parse("params.workers").is_err());
        assert!(parse("exec(value)").is_err());
        assert!(parse("value +").is_err());
    }
}
