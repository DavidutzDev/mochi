//! A small calculator for the launcher's search box.
//!
//! It reads numbers like `12`, `3.5`, `.5`, `1e3` and `0x1F`, with `_`
//! allowed between digits. The operators are `+`, `-`, `*`, `/`, `%` for the
//! remainder and `^` for powers, plus `×` and `÷`. A `!` after a whole number
//! is its factorial. Names are the constants `pi`, `π`, `e` and `tau`, and
//! functions called with parentheses, like `sqrt(2)` or `max(3, 4)`. Trig
//! works in radians. A value right before a name or an opening parenthesis
//! multiplies, so `2pi`, `2(3+1)` and `(1+1)(2)` work.
//!
//! `^` binds tighter than a leading minus, so `-2^2` is -4, and it groups
//! from the right, so `2^3^2` is 2^9.

/// The value of `expression`, or why there is none.
pub fn evaluate(expression: &str) -> Result<f64, String> {
    run(expression).map(|(value, _)| value)
}

/// Whether a query typed without the calculator's prefix is meant as math:
/// it has at least one operator or function call and evaluates, and isn't
/// just a number or a word. "2+2", "sqrt(2)", "(3)*4", "2^10" yes; "42",
/// "firefox", "-5", "e", "pi" no.
pub fn looks_like_math(query: &str) -> bool {
    matches!(run(query), Ok((_, true)))
}

/// A value as the launcher shows and copies it: up to 12 significant
/// digits, no trailing zeros, no exponent between 1e-6 and 1e15, integers
/// without a decimal point, "-0" shown as "0". Infinite or NaN values are
/// errors in evaluate, so format never sees them.
pub fn format(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return if value == 0.0 {
            "0".into()
        } else {
            value.to_string()
        };
    }
    // Rounding to 12 digits once, through the exponent form, keeps the
    // digits and the exponent in agreement: 999999999999999.9 becomes 1e15.
    let rounded = format!("{:.11e}", value.abs());
    let (mantissa, exponent) = rounded.split_once('e').unwrap_or((&rounded, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let digits = mantissa.replace('.', "");
    let digits = digits.trim_end_matches('0');
    let sign = if value < 0.0 { "-" } else { "" };

    if !(-6..15).contains(&exponent) {
        let (first, rest) = digits.split_at(1);
        return if rest.is_empty() {
            format!("{sign}{first}e{exponent}")
        } else {
            format!("{sign}{first}.{rest}e{exponent}")
        };
    }
    if exponent < 0 {
        let zeros = "0".repeat(exponent.unsigned_abs() as usize - 1);
        return format!("{sign}0.{zeros}{digits}");
    }
    let whole = exponent as usize + 1;
    if digits.len() <= whole {
        format!("{sign}{digits:0<whole$}")
    } else {
        let (int, frac) = digits.split_at(whole);
        format!("{sign}{int}.{frac}")
    }
}

/// The value, and whether anything past a lone number or name and its sign
/// was applied.
fn run(expression: &str) -> Result<(f64, bool), String> {
    let tokens = tokenize(expression)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        operations: 0,
    };
    let value = parser.expression()?;
    if let Some(token) = parser.peek() {
        return Err(if *token == Token::Close {
            "unbalanced parentheses".into()
        } else {
            format!("unexpected \"{token}\"")
        });
    }
    if !value.is_finite() {
        return Err("the result isn't a finite number".into());
    }
    Ok((value, parser.operations > 0))
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    /// One of `+ - * / % ^`.
    Operator(char),
    Open,
    Close,
    Comma,
    Bang,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(value) => write!(f, "{}", format(*value)),
            Self::Name(name) => f.write_str(name),
            Self::Operator(op) => write!(f, "{op}"),
            Self::Open => f.write_str("("),
            Self::Close => f.write_str(")"),
            Self::Comma => f.write_str(","),
            Self::Bang => f.write_str("!"),
        }
    }
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            tokens.push(Token::Number(number(&mut chars)?));
            continue;
        }
        if c.is_alphabetic() {
            let mut name = String::new();
            while let Some(&c) = chars.peek().filter(|c| c.is_alphanumeric()) {
                name.push(c);
                chars.next();
            }
            tokens.push(Token::Name(name));
            continue;
        }
        chars.next();
        tokens.push(match c {
            '+' | '-' | '*' | '/' | '%' | '^' => Token::Operator(c),
            '−' => Token::Operator('-'),
            '×' => Token::Operator('*'),
            '÷' => Token::Operator('/'),
            '(' => Token::Open,
            ')' => Token::Close,
            ',' => Token::Comma,
            '!' => Token::Bang,
            _ => return Err(format!("unexpected \"{c}\"")),
        });
    }
    Ok(tokens)
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// Moves the characters `accept` takes, and any `_`, from `chars` to `text`,
/// leaving out the `_`.
fn take(chars: &mut Chars<'_>, text: &mut String, accept: impl Fn(char) -> bool) {
    while let Some(c) = chars.next_if(|&c| accept(c) || c == '_') {
        if c != '_' {
            text.push(c);
        }
    }
}

/// Reads a decimal or `0x` number, skipping `_` between digits.
fn number(chars: &mut Chars<'_>) -> Result<f64, String> {
    let mut text = String::new();
    take(chars, &mut text, |c| c.is_ascii_digit() || c == '.');
    if text == "0" && matches!(chars.peek(), Some('x' | 'X')) {
        chars.next();
        let mut hex = String::new();
        take(chars, &mut hex, |c| c.is_ascii_hexdigit());
        return u64::from_str_radix(&hex, 16)
            .map(|value| value as f64)
            .map_err(|_| format!("bad number \"0x{hex}\""));
    }
    // An `e` is an exponent only when digits follow, so `2e` is 2 times e.
    if matches!(chars.peek(), Some('e' | 'E')) {
        let mut ahead = chars.clone();
        ahead.next();
        let sign = ahead.next_if(|&c| c == '+' || c == '-');
        if ahead.peek().is_some_and(char::is_ascii_digit) {
            text.push('e');
            text.extend(sign);
            *chars = ahead;
            take(chars, &mut text, |c| c.is_ascii_digit());
        }
    }
    text.parse().map_err(|_| format!("bad number \"{text}\""))
}

fn function(name: &str) -> Option<fn(f64) -> f64> {
    Some(match name {
        "sqrt" => f64::sqrt,
        "cbrt" => f64::cbrt,
        "abs" => f64::abs,
        "floor" => f64::floor,
        "ceil" => f64::ceil,
        "round" => f64::round,
        "sin" => f64::sin,
        "cos" => f64::cos,
        "tan" => f64::tan,
        "asin" => f64::asin,
        "acos" => f64::acos,
        "atan" => f64::atan,
        "sinh" => f64::sinh,
        "cosh" => f64::cosh,
        "tanh" => f64::tanh,
        "ln" => f64::ln,
        "log" => f64::log10,
        "log2" => f64::log2,
        "exp" => f64::exp,
        _ => return None,
    })
}

fn binary_function(name: &str) -> Option<fn(f64, f64) -> f64> {
    Some(match name {
        "min" => f64::min,
        "max" => f64::max,
        "pow" => f64::powf,
        _ => return None,
    })
}

fn constant(name: &str) -> Option<f64> {
    match name {
        "pi" | "π" => Some(std::f64::consts::PI),
        "e" => Some(std::f64::consts::E),
        "tau" => Some(std::f64::consts::TAU),
        _ => None,
    }
}

/// Recursive descent, lowest precedence first:
///
/// ```text
/// expression = term (("+" | "-") term)*
/// term       = unary (("*" | "/" | "%") unary | implicit unary)*
/// unary      = ("+" | "-") unary | power
/// power      = postfix ("^" unary)?
/// postfix    = primary "!"*
/// primary    = number | name | name "(" arguments ")" | "(" expression ")"
/// ```
#[derive(Debug)]
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    /// Operators, calls and factorials applied, leaving out signs.
    operations: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    fn eat_operator(&mut self, ops: &[char]) -> Option<char> {
        match self.peek() {
            Some(Token::Operator(op)) if ops.contains(op) => {
                let op = *op;
                self.pos += 1;
                Some(op)
            }
            _ => None,
        }
    }

    fn expression(&mut self) -> Result<f64, String> {
        let mut value = self.term()?;
        while let Some(op) = self.eat_operator(&['+', '-']) {
            let right = self.term()?;
            self.operations += 1;
            value = if op == '+' {
                value + right
            } else {
                value - right
            };
        }
        Ok(value)
    }

    fn term(&mut self) -> Result<f64, String> {
        let mut value = self.unary()?;
        loop {
            if let Some(op) = self.eat_operator(&['*', '/', '%']) {
                let right = self.unary()?;
                if op != '*' && right == 0.0 {
                    return Err("division by zero".into());
                }
                value = match op {
                    '*' => value * right,
                    '/' => value / right,
                    _ => value % right,
                };
            } else if matches!(self.peek(), Some(Token::Name(_) | Token::Open)) {
                value *= self.unary()?;
            } else {
                return Ok(value);
            }
            self.operations += 1;
        }
    }

    fn unary(&mut self) -> Result<f64, String> {
        match self.eat_operator(&['+', '-']) {
            Some('-') => Ok(-self.unary()?),
            Some(_) => self.unary(),
            None => self.power(),
        }
    }

    fn power(&mut self) -> Result<f64, String> {
        let base = self.postfix()?;
        if self.eat_operator(&['^']).is_none() {
            return Ok(base);
        }
        // The exponent goes through unary so `2^-1` works, and that recurses
        // back here, which makes `^` group from the right.
        let exponent = self.unary()?;
        self.operations += 1;
        Ok(base.powf(exponent))
    }

    fn postfix(&mut self) -> Result<f64, String> {
        let mut value = self.primary()?;
        while self.peek() == Some(&Token::Bang) {
            self.pos += 1;
            self.operations += 1;
            value = factorial(value)?;
        }
        Ok(value)
    }

    fn primary(&mut self) -> Result<f64, String> {
        let previous = self.pos.checked_sub(1).and_then(|i| self.tokens.get(i));
        let missing = match previous {
            None => "empty expression",
            Some(Token::Open) => "unbalanced parentheses",
            Some(_) => "trailing operator",
        };
        match self.next() {
            None => Err(missing.into()),
            Some(Token::Number(value)) => Ok(value),
            Some(Token::Open) => {
                let value = self.expression()?;
                self.close()?;
                Ok(value)
            }
            Some(Token::Name(name)) => self.name(&name),
            Some(token) => Err(format!("unexpected \"{token}\"")),
        }
    }

    fn name(&mut self, name: &str) -> Result<f64, String> {
        if let Some(value) = constant(name) {
            return Ok(value);
        }
        let one = function(name);
        let two = binary_function(name);
        if one.is_none() && two.is_none() {
            return Err(format!("unknown name \"{name}\""));
        }
        if self.next() != Some(Token::Open) {
            return Err(format!("{name} needs parentheses"));
        }
        let mut args = vec![self.expression()?];
        while self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            args.push(self.expression()?);
        }
        self.close()?;
        self.operations += 1;
        match (one, two, args.as_slice()) {
            (Some(f), _, &[x]) => Ok(f(x)),
            (_, Some(f), &[a, b]) => Ok(f(a, b)),
            (Some(_), ..) => Err(format!("{name} takes one argument")),
            _ => Err(format!("{name} takes two arguments")),
        }
    }

    fn close(&mut self) -> Result<(), String> {
        match self.next() {
            Some(Token::Close) => Ok(()),
            None => Err("unbalanced parentheses".into()),
            Some(token) => Err(format!("unexpected \"{token}\"")),
        }
    }
}

fn factorial(n: f64) -> Result<f64, String> {
    if !(0.0..=170.0).contains(&n) || n.fract() != 0.0 {
        return Err("factorial needs a whole number from 0 to 170".into());
    }
    Ok((2..=n as u32).map(f64::from).product())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(expression: &str) -> f64 {
        evaluate(expression).unwrap_or_else(|err| panic!("{expression}: {err}"))
    }

    fn close(expression: &str, expected: f64) {
        let value = eval(expression);
        assert!(
            (value - expected).abs() < 1e-9,
            "{expression} = {value}, expected {expected}"
        );
    }

    fn error(expression: &str) -> String {
        evaluate(expression).expect_err(expression)
    }

    #[test]
    fn precedence_and_associativity() {
        close("1+2*3", 7.0);
        close("(1+2)*3", 9.0);
        close("10-4-3", 3.0);
        close("100/10/5", 2.0);
        close("2^3^2", 512.0);
        close("2*3^2", 18.0);
        close("7%3", 1.0);
        close("-7%3", -1.0);
        close("1+10%4*2", 5.0);
        close("6×7", 42.0);
        close("1÷4", 0.25);
        close(" 1 +\t2 ", 3.0);
    }

    #[test]
    fn unary_minus_and_power() {
        close("-2^2", -4.0);
        close("(-2)^2", 4.0);
        close("2^-1", 0.5);
        close("--3", 3.0);
        close("+3", 3.0);
        close("3--2", 5.0);
        close("2*-3", -6.0);
        close("−5+1", -4.0);
    }

    #[test]
    fn implicit_multiplication() {
        close("2pi", 2.0 * std::f64::consts::PI);
        close("2(3+1)", 8.0);
        close("(1+1)(2)", 4.0);
        close("3sqrt(4)", 6.0);
        close("2pi^2", 2.0 * std::f64::consts::PI.powi(2));
        close("-2e", -2.0 * std::f64::consts::E);
    }

    #[test]
    fn constants() {
        close("pi", std::f64::consts::PI);
        close("π", std::f64::consts::PI);
        close("e", std::f64::consts::E);
        close("tau", std::f64::consts::TAU);
    }

    #[test]
    fn functions() {
        close("sqrt(16)", 4.0);
        close("cbrt(27)", 3.0);
        close("abs(-3)", 3.0);
        close("floor(2.7)", 2.0);
        close("ceil(2.1)", 3.0);
        close("round(2.5)", 3.0);
        close("sin(pi/2)", 1.0);
        close("cos(0)", 1.0);
        close("tan(pi/4)", 1.0);
        close("asin(1)", std::f64::consts::FRAC_PI_2);
        close("acos(1)", 0.0);
        close("atan(1)", std::f64::consts::FRAC_PI_4);
        close("sinh(0)", 0.0);
        close("cosh(0)", 1.0);
        close("tanh(0)", 0.0);
        close("ln(e)", 1.0);
        close("log(1000)", 3.0);
        close("log2(1024)", 10.0);
        close("exp(1)", std::f64::consts::E);
        close("min(3, 4)", 3.0);
        close("max(3, -4)", 3.0);
        close("pow(2, 10)", 1024.0);
        close("max(1, 2+3)*2", 10.0);
    }

    #[test]
    fn factorial() {
        close("0!", 1.0);
        close("5!", 120.0);
        close("3!!", 720.0);
        close("-3!", -6.0);
        close("2^3!", 64.0);
        assert!(eval("170!").is_finite());
        assert!(error("171!").contains("factorial"));
        assert!(error("2.5!").contains("factorial"));
        assert!(error("(-1)!").contains("factorial"));
    }

    #[test]
    fn numbers() {
        close("0x1F", 31.0);
        close("0XfF", 255.0);
        close("1e3", 1000.0);
        close("2.5E-3", 0.0025);
        close("1e+2", 100.0);
        close(".5", 0.5);
        close("3.5", 3.5);
        close("1_000_000", 1e6);
        close("0x_ff_ff", 65535.0);
    }

    #[test]
    fn errors() {
        assert_eq!(error("1/0"), "division by zero");
        assert_eq!(error("5%0"), "division by zero");
        assert_eq!(error("2*foo"), "unknown name \"foo\"");
        assert_eq!(error("(1+2"), "unbalanced parentheses");
        assert_eq!(error("1+2)"), "unbalanced parentheses");
        assert_eq!(error("("), "unbalanced parentheses");
        assert_eq!(error("1+"), "trailing operator");
        assert_eq!(error("2*"), "trailing operator");
        assert_eq!(error("sqrt(-1)"), "the result isn't a finite number");
        assert_eq!(error("10^400"), "the result isn't a finite number");
        assert_eq!(error(""), "empty expression");
        assert_eq!(error("1,2"), "unexpected \",\"");
        assert_eq!(error("2 3"), "unexpected \"3\"");
        assert_eq!(error("1.2.3"), "bad number \"1.2.3\"");
        assert_eq!(error("0x"), "bad number \"0x\"");
        assert_eq!(error("2$"), "unexpected \"$\"");
        assert_eq!(error("sqrt 4"), "sqrt needs parentheses");
        assert_eq!(error("sqrt(1, 2)"), "sqrt takes one argument");
        assert_eq!(error("min(1)"), "min takes two arguments");
    }

    #[test]
    fn math_or_not() {
        for query in [
            "2+2", "sqrt(2)", "(3)*4", "2^10", "2pi", "5!", "1 - 1", "max(1,2)", "0x10*2",
        ] {
            assert!(looks_like_math(query), "{query}");
        }
        for query in [
            "42", "firefox", "-5", "e", "pi", "(3)", "", "1/0", "2+", "fire fox", "1e3", "0x1F",
        ] {
            assert!(!looks_like_math(query), "{query}");
        }
    }

    #[test]
    fn formatting() {
        assert_eq!(format(eval("0.1+0.2")), "0.3");
        assert_eq!(format(eval("1/3")), "0.333333333333");
        assert_eq!(format(eval("2/3")), "0.666666666667");
        assert_eq!(format(eval("2^60")), "1.15292150461e18");
        assert_eq!(format(eval("1e-7")), "1e-7");
        assert_eq!(format(-1.5e-7), "-1.5e-7");
        assert_eq!(format(1e15), "1e15");
        assert_eq!(format(999_999_999_999_999.0), "1e15");
        assert_eq!(format(123_456_789_012_345.0), "123456789012000");
        assert_eq!(format(1e-6), "0.000001");
        assert_eq!(format(eval("10/4")), "2.5");
        assert_eq!(format(4.0), "4");
        assert_eq!(format(-4.0), "-4");
        assert_eq!(format(100.0), "100");
        assert_eq!(format(-0.0), "0");
        assert_eq!(format(0.0), "0");
        assert_eq!(format(eval("pi")), "3.14159265359");
        assert_eq!(format(eval("-2^0.5")), "-1.41421356237");
    }
}
