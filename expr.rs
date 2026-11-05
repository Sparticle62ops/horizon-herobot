// Upstream herobot-common hero.bane.herobot.common.ai.expr.{ExprEval,
// ExprLog}.java (CC-BY-NC-4.0), ported 1:1: the numeric expression
// engine behind NUM_CALC and the numeric sub-grammar that
// BOOL_CALC/STRING_CALC/POS_CALC/DIR_CALC embed through parse_numeric.
// Upstream catches every parse failure and evaluates to a constant 0
// after warning once per distinct (expression reason) pair, so the
// Results below feed that path and never escape eval(). The 256-entry
// compile cache and the same-bounded log set are thread-locals here
// (upstream: ConcurrentHashMap + synchronized set; the portable
// runtime evaluates on one thread).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

/// Upstream ExprEval.ParamType, as far as the numeric grammar checks
/// it: only POSITION/ROTATION decide indexability in checkRef; the
/// other kinds matter to the boolean/string/vector grammars and get
/// their variants as those ports land.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamKind {
    Position,
    Rotation,
    // The other grammars type their operands with these.
    Bool,
    Num,
    Str,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fn1Kind {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sqrt,
    Floor,
    Ceil,
    Abs,
}

/// Upstream ExprEval.Expr: a compiled numeric expression tree.
#[derive(Clone, Debug)]
pub enum Expr {
    Zero,
    Const(f64),
    Var(String, i32),
    Neg(Box<Expr>),
    /// Upstream ++ / -- postfix: the value plus (true) or minus one.
    Post(Box<Expr>, bool),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Pi,
    E,
    Tau,
    Rand,
    Fn1(Fn1Kind, Box<Expr>),
    /// Upstream pow(a, b); with one argument it evaluates to a.
    Pow(Box<Expr>, Box<Expr>),
    Rand1(Box<Expr>),
    Rand2(Box<Expr>, Box<Expr>),
    /// randomint(a, b): floor(min)..=floor(max), inclusive.
    RandInt(Box<Expr>, Box<Expr>, bool),
}

impl Expr {
    pub fn eval(&self, vars: &mut dyn FnMut(&str, i32) -> f64) -> f64 {
        match self {
            Expr::Zero => 0.0,
            Expr::Const(v) => *v,
            Expr::Var(name, idx) => vars(name, *idx),
            Expr::Neg(a) => -a.eval(vars),
            Expr::Post(a, up) => a.eval(vars) + if *up { 1.0 } else { -1.0 },
            Expr::Bin(op, l, r) => match op {
                BinOp::Add => l.eval(vars) + r.eval(vars),
                BinOp::Sub => l.eval(vars) - r.eval(vars),
                BinOp::Mul => l.eval(vars) * r.eval(vars),
                // Upstream divides and mods guard the zero divisor with 0.
                BinOp::Div => {
                    let d = r.eval(vars);
                    if d == 0.0 { 0.0 } else { l.eval(vars) / d }
                }
                BinOp::Mod => {
                    let d = r.eval(vars);
                    if d == 0.0 { 0.0 } else { l.eval(vars) % d }
                }
                BinOp::Pow => l.eval(vars).powf(r.eval(vars)),
            },
            Expr::Pi => std::f64::consts::PI,
            Expr::E => std::f64::consts::E,
            Expr::Tau => std::f64::consts::PI * 2.0,
            Expr::Rand => rand_unit(),
            Expr::Fn1(kind, a) => {
                let x = a.eval(vars);
                match kind {
                    Fn1Kind::Sin => x.sin(),
                    Fn1Kind::Cos => x.cos(),
                    Fn1Kind::Tan => x.tan(),
                    Fn1Kind::Asin => x.asin(),
                    Fn1Kind::Acos => x.acos(),
                    Fn1Kind::Atan => x.atan(),
                    Fn1Kind::Sqrt => x.sqrt(),
                    Fn1Kind::Floor => x.floor(),
                    Fn1Kind::Ceil => x.ceil(),
                    Fn1Kind::Abs => x.abs(),
                }
            }
            Expr::Pow(a, b) => a.eval(vars).powf(b.eval(vars)),
            Expr::Rand1(a) => rand_unit() * a.eval(vars),
            // Upstream evaluates the lower bound twice here.
            Expr::Rand2(a, b) => {
                let lo = a.eval(vars);
                lo + rand_unit() * (b.eval(vars) - a.eval(vars))
            }
            Expr::RandInt(a, b, two) => {
                let x = a.eval(vars);
                let y = if *two { b.eval(vars) } else { 0.0 };
                let lo = x.min(y).floor() as i64;
                let hi = x.max(y).floor() as i64;
                (lo + (rand_unit() * (hi - lo + 1) as f64) as i64) as f64
            }
        }
    }
}

thread_local! {
    static RNG_STATE: Cell<u64> = Cell::new(0);
    static CACHE: RefCell<HashMap<String, Expr>> = RefCell::new(HashMap::new());
    static REPORTED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// Thread-local xorshift* standing in for Math.random(), seeded from
/// the clock like the runtime's RANDOM_* port.
pub fn rand_unit() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    RNG_STATE.with(|st| {
        let mut x = st.get();
        if x == 0 {
            x = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E37_79B9_7F4A_7C15)
                | 1;
        }
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        st.set(x);
        let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (r >> 11) as f64 / (1u64 << 53) as f64
    })
}

/// Upstream ExprLog.badExpression.
pub fn log_bad_expression(expression: &str, reason: &str) {
    REPORTED.with(|rep| {
        let mut set = rep.borrow_mut();
        if set.len() >= 256 {
            set.clear();
        }
        if set.insert(format!("{} {}", expression, reason)) {
            eprintln!(
                "Invalid expression \"{}\" ({}) - it will evaluate to a constant",
                expression, reason
            );
        }
    });
}

/// Upstream ExprEval.isInputRef: "Input" followed by one or more digits.
pub fn is_input_ref(name: &str) -> bool {
    if name.len() < 6 {
        return false;
    }
    if !name[..5].eq_ignore_ascii_case("Input") {
        return false;
    }
    name[5..].chars().all(|c| c.is_ascii_digit())
}

/// Upstream ExprEval.canonicalInput.
pub fn canonical_input(name: &str) -> String {
    format!("Input{}", &name[5..])
}

fn cache_get(key: &str) -> Option<Expr> {
    CACHE.with(|c| c.borrow().get(key).cloned())
}

fn cache_put(key: &str, e: Expr) {
    CACHE.with(|c| {
        let mut m = c.borrow_mut();
        if m.len() >= 256 {
            m.clear();
        }
        m.insert(key.to_string(), e);
    });
}

/// Upstream ExprEval.compile: parse (strict), cache, fall back to the
/// zero constant on any parse failure.
pub fn compile(expression: &str) -> Expr {
    if expression.trim().is_empty() {
        return Expr::Zero;
    }
    if let Some(cached) = cache_get(expression) {
        return cached;
    }
    let compiled = match Parser::new(expression, true, None).parse_full() {
        Ok(e) => e,
        Err(reason) => {
            log_bad_expression(expression, &reason);
            Expr::Zero
        }
    };
    cache_put(expression, compiled.clone());
    compiled
}

/// Upstream ExprEval.eval(String, IndexedVars).
pub fn eval(expression: &str, vars: &mut dyn FnMut(&str, i32) -> f64) -> f64 {
    compile(expression).eval(vars)
}

/// Upstream ExprEval.isValid: full strict parse with no trailing input.
pub fn is_valid(expression: &str) -> bool {
    if expression.trim().is_empty() {
        return true;
    }
    let mut p = Parser::new(expression, true, None);
    match p.expr() {
        Ok(_) => {
            p.skip_ws();
            p.at_end()
        }
        Err(_) => false,
    }
}

/// Upstream ExprEval.Sub: a parsed sub-expression plus the end offset.
pub struct Sub {
    pub expr: Expr,
    pub end: usize,
}

/// Upstream ExprEval.parseNumeric, used by the other grammars to embed
/// numeric sub-expressions.
pub fn parse_numeric(
    s: &str,
    from: usize,
    strict: bool,
    types: Option<&dyn Fn(&str) -> Option<ParamKind>>,
) -> Result<Sub, String> {
    let mut p = Parser::new(s, strict, types);
    p.pos = from;
    let e = p.expr()?;
    Ok(Sub { expr: e, end: p.pos })
}

pub struct Parser<'a> {
    input: Vec<char>,
    strict: bool,
    types: Option<&'a dyn Fn(&str) -> Option<ParamKind>>,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(
        input: &str,
        strict: bool,
        types: Option<&'a dyn Fn(&str) -> Option<ParamKind>>,
    ) -> Parser<'a> {
        Parser {
            input: input.chars().collect(),
            strict,
            types,
            pos: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    pub fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn lookahead(&self, tok: &str) -> bool {
        let mut i = self.pos;
        for c in tok.chars() {
            if self.input.get(i).copied() != Some(c) {
                return false;
            }
            i += 1;
        }
        true
    }

    /// Upstream: expr() then skipWs() then atEnd() from compile().
    pub fn parse_full(&mut self) -> Result<Expr, String> {
        let e = self.expr()?;
        self.skip_ws();
        if self.at_end() {
            Ok(e)
        } else {
            Err("unexpected trailing input".to_string())
        }
    }

    pub fn expr(&mut self) -> Result<Expr, String> {
        let mut v = self.term()?;
        loop {
            self.skip_ws();
            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };
            if c == '+' && !self.lookahead("++") {
                self.pos += 1;
                let l = v;
                let r = self.term()?;
                v = Expr::Bin(BinOp::Add, Box::new(l), Box::new(r));
            } else if c == '-' && !self.lookahead("--") {
                self.pos += 1;
                let l = v;
                let r = self.term()?;
                v = Expr::Bin(BinOp::Sub, Box::new(l), Box::new(r));
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut v = self.power()?;
        loop {
            self.skip_ws();
            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };
            if c == '*' {
                self.pos += 1;
                let l = v;
                let r = self.power()?;
                v = Expr::Bin(BinOp::Mul, Box::new(l), Box::new(r));
            } else if c == '/' {
                self.pos += 1;
                let l = v;
                let r = self.power()?;
                v = Expr::Bin(BinOp::Div, Box::new(l), Box::new(r));
            } else if c == '%' {
                self.pos += 1;
                let l = v;
                let r = self.power()?;
                v = Expr::Bin(BinOp::Mod, Box::new(l), Box::new(r));
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn power(&mut self) -> Result<Expr, String> {
        let base = self.unary()?;
        self.skip_ws();
        if self.peek() == Some('^') {
            self.pos += 1;
            let e = self.power()?;
            return Ok(Expr::Bin(BinOp::Pow, Box::new(base), Box::new(e)));
        }
        Ok(base)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        if self.peek() == Some('-') && !self.lookahead("--") {
            self.pos += 1;
            let a = self.unary()?;
            return Ok(Expr::Neg(Box::new(a)));
        }
        if self.peek() == Some('+') && !self.lookahead("++") {
            self.pos += 1;
            return self.unary();
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut v = self.primary()?;
        loop {
            self.skip_ws();
            if self.lookahead("++") {
                self.pos += 2;
                v = Expr::Post(Box::new(v), true);
            } else if self.lookahead("--") {
                self.pos += 2;
                v = Expr::Post(Box::new(v), false);
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        let c = match self.peek() {
            Some(c) => c,
            None => return Err("unexpected input end".to_string()),
        };
        if c == '(' {
            self.pos += 1;
            let v = self.expr()?;
            self.skip_ws();
            if self.peek() == Some(')') {
                self.pos += 1;
            } else if self.strict {
                return Err("expected )".to_string());
            }
            return Ok(v);
        }
        if c == '{' {
            return self.variable();
        }
        if c == '.' || c.is_ascii_digit() {
            return self.number();
        }
        if c.is_alphabetic() || c == '_' {
            return self.identifier();
        }
        Err(format!("unexpected '{}'", c))
    }

    fn variable(&mut self) -> Result<Expr, String> {
        self.pos += 1;
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == '}' {
                break;
            }
            self.pos += 1;
        }
        let name: String = self.input[start..self.pos].iter().collect::<String>().trim().to_string();
        if self.peek() == Some('}') {
            self.pos += 1;
        } else if self.strict {
            return Err("expected }".to_string());
        }
        let idx = self.parse_index()?;
        self.check_ref(&name, idx)?;
        Ok(Expr::Var(name, idx))
    }

    fn parse_index(&mut self) -> Result<i32, String> {
        self.skip_ws();
        if self.peek() != Some('[') {
            return Ok(-1);
        }
        self.pos += 1;
        self.skip_ws();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err("expected index".to_string());
        }
        let text: String = self.input[start..self.pos].iter().collect();
        let idx: i32 = text.parse().map_err(|_| "bad index".to_string())?;
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
        } else if self.strict {
            return Err("expected ]".to_string());
        }
        Ok(idx)
    }

    fn check_ref(&self, name: &str, idx: i32) -> Result<(), String> {
        if !self.strict {
            return Ok(());
        }
        let types = match self.types {
            Some(t) => t,
            None => return Ok(()),
        };
        let t = match types(name) {
            Some(t) => t,
            None => return Ok(()),
        };
        let vec = t == ParamKind::Position || t == ParamKind::Rotation;
        if idx < 0 {
            if vec {
                return Err(format!("{} needs an index like [0]", name));
            }
            return Ok(());
        }
        if !vec {
            return Err(format!("{} can't be indexed", name));
        }
        let max = if t == ParamKind::Position { 2 } else { 1 };
        if idx > max {
            return Err("index out of range".to_string());
        }
        Ok(())
    }

    fn number(&mut self) -> Result<Expr, String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text: String = self.input[start..self.pos].iter().collect();
        let val: f64 = text.parse().map_err(|_| "invalid number".to_string())?;
        Ok(Expr::Const(val))
    }

    fn identifier(&mut self) -> Result<Expr, String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let name: String = self.input[start..self.pos].iter().collect();
        self.skip_ws();
        if self.peek() == Some('(') {
            return self.call_function(&name);
        }
        self.constant(&name)
    }

    fn constant(&mut self, name: &str) -> Result<Expr, String> {
        if is_input_ref(name) {
            let canonical = canonical_input(name);
            let idx = self.parse_index()?;
            self.check_ref(&canonical, idx)?;
            return Ok(Expr::Var(canonical, idx));
        }
        match name.to_lowercase().as_str() {
            "pi" => Ok(Expr::Pi),
            "e" => Ok(Expr::E),
            "tau" => Ok(Expr::Tau),
            "rand" | "random" => Ok(Expr::Rand),
            _ => {
                if self.strict {
                    Err(format!("unknown name: {}", name))
                } else {
                    Ok(Expr::Zero)
                }
            }
        }
    }

    fn call_function(&mut self, name: &str) -> Result<Expr, String> {
        self.pos += 1;
        let arg = self.expr()?;
        let arg2 = Expr::Zero;
        self.skip_ws();
        let mut two_args = false;
        let mut arg2 = arg2;
        if self.peek() == Some(',') {
            self.pos += 1;
            arg2 = self.expr()?;
            two_args = true;
            self.skip_ws();
        }
        if self.peek() == Some(')') {
            self.pos += 1;
        } else if self.strict {
            return Err("expected )".to_string());
        }
        match name {
            "sin" => Ok(Expr::Fn1(Fn1Kind::Sin, Box::new(arg))),
            "cos" => Ok(Expr::Fn1(Fn1Kind::Cos, Box::new(arg))),
            "tan" => Ok(Expr::Fn1(Fn1Kind::Tan, Box::new(arg))),
            "asin" | "arcsin" => Ok(Expr::Fn1(Fn1Kind::Asin, Box::new(arg))),
            "acos" | "arccos" => Ok(Expr::Fn1(Fn1Kind::Acos, Box::new(arg))),
            "atan" | "arctan" => Ok(Expr::Fn1(Fn1Kind::Atan, Box::new(arg))),
            "sqrt" => Ok(Expr::Fn1(Fn1Kind::Sqrt, Box::new(arg))),
            "floor" => Ok(Expr::Fn1(Fn1Kind::Floor, Box::new(arg))),
            "ceil" => Ok(Expr::Fn1(Fn1Kind::Ceil, Box::new(arg))),
            "abs" => Ok(Expr::Fn1(Fn1Kind::Abs, Box::new(arg))),
            "pow" => {
                if two_args {
                    Ok(Expr::Pow(Box::new(arg), Box::new(arg2)))
                } else {
                    Ok(arg)
                }
            }
            "rand" | "random" => {
                if two_args {
                    Ok(Expr::Rand2(Box::new(arg), Box::new(arg2)))
                } else {
                    Ok(Expr::Rand1(Box::new(arg)))
                }
            }
            "randomint" | "randint" => Ok(Expr::RandInt(Box::new(arg), Box::new(arg2), two_args)),
            _ => {
                if self.strict {
                    Err(format!("unknown function: {}", name))
                } else {
                    Ok(Expr::Zero)
                }
            }
        }
    }
}
