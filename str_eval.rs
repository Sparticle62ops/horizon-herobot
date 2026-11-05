// Port of herobot-common StrEval.java (CC-BY-NC-4.0, HerobaneNair/HeRoBot).
// String expression engine: "text" + {var} + Input refs, python-style
// slicing [start:stop:step] and single-char indexing [i], and .strip().
// Compile failures and trailing input yield the empty expression; missing
// variables stringify to "". Upstream indexes UTF-16 code units; this port
// indexes Unicode scalar values (identical for the ASCII bot scripts use).

use crate::expr::{canonical_input, is_input_ref};
use std::cell::RefCell;
use std::collections::HashMap;

pub const OPS_LEGEND: &str = "\"text\"  +  {var}  Input1  ' \\,\" '";
pub const OPS_LEGEND_2: &str = "[start:stop:step]  [::-1]  .strip()";

const CACHE_LIMIT: usize = 256;

#[derive(Clone, Debug)]
pub enum StrExpr {
    Empty,
    Lit(String),
    Concat(Box<StrExpr>, Box<StrExpr>),
    Var(String),
    Input(String),
    Index(Box<StrExpr>, i32),
    Slice(Box<StrExpr>, Option<i32>, Option<i32>, Option<i32>),
    Strip(Box<StrExpr>),
}

pub type StrVarsFn<'a> = dyn FnMut(&str) -> Option<String> + 'a;

impl StrExpr {
    pub fn eval(&self, vars: &mut StrVarsFn) -> String {
        match self {
            StrExpr::Empty => String::new(),
            StrExpr::Lit(s) => s.clone(),
            StrExpr::Concat(l, r) => format!("{}{}", l.eval(vars), r.eval(vars)),
            StrExpr::Var(n) | StrExpr::Input(n) => vars(n).unwrap_or_default(),
            StrExpr::Index(b, i) => str_index(&b.eval(vars), *i),
            StrExpr::Slice(b, s, e, st) => str_slice(&b.eval(vars), *s, *e, *st),
            StrExpr::Strip(b) => b.eval(vars).trim().to_string(),
        }
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<String, StrExpr>> = RefCell::new(HashMap::new());
}

pub fn eval(expression: &str, vars: &mut StrVarsFn) -> String {
    compile(expression).eval(vars)
}

pub fn is_valid(expression: &str) -> bool {
    if expression.trim().is_empty() {
        return true;
    }
    let mut p = Parser::new(expression, true);
    match p.expr() {
        Ok(_) => {
            p.skip_ws();
            p.at_end()
        }
        Err(_) => false,
    }
}

pub fn compile(expression: &str) -> StrExpr {
    if expression.trim().is_empty() {
        return StrExpr::Empty;
    }
    let mut compiled = StrExpr::Empty;
    let mut p = Parser::new(expression, false);
    if let Ok(e) = p.expr() {
        p.skip_ws();
        if p.at_end() {
            compiled = e;
        }
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= CACHE_LIMIT {
            c.clear();
        }
        c.insert(expression.to_string(), compiled.clone());
    });
    compiled
}

struct Parser {
    input: Vec<char>,
    pos: usize,
    strict: bool,
}

impl Parser {
    fn new(src: &str, strict: bool) -> Parser {
        Parser {
            input: src.chars().collect(),
            pos: 0,
            strict,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek(&self) -> char {
        if self.at_end() { '\0' } else { self.input[self.pos] }
    }

    fn skip_ws(&mut self) {
        while !self.at_end() && self.input[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn expr(&mut self) -> Result<StrExpr, String> {
        let mut v = self.term()?;
        loop {
            self.skip_ws();
            if self.peek() == '+' {
                self.pos += 1;
                let r = self.term()?;
                v = StrExpr::Concat(Box::new(v), Box::new(r));
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn term(&mut self) -> Result<StrExpr, String> {
        let mut v = self.atom()?;
        loop {
            self.skip_ws();
            let c = self.peek();
            if c == '[' {
                v = self.slice_of(v)?;
            } else if c == '.' {
                v = self.method_of(v)?;
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn method_of(&mut self, base: StrExpr) -> Result<StrExpr, String> {
        self.pos += 1; // '.'
        let start = self.pos;
        while !self.at_end() && self.input[self.pos].is_alphabetic() {
            self.pos += 1;
        }
        let name: String = self.input[start..self.pos].iter().collect();
        self.skip_ws();
        if self.peek() == '(' {
            self.pos += 1;
        } else if self.strict {
            return Err(format!("expected ( after .{}", name));
        }
        self.skip_ws();
        if self.peek() == ')' {
            self.pos += 1;
        } else if self.strict {
            return Err(format!("expected ) after .{}(", name));
        }
        if name == "strip" {
            return Ok(StrExpr::Strip(Box::new(base)));
        }
        if self.strict {
            return Err(format!("unknown method: {}", name));
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<StrExpr, String> {
        self.skip_ws();
        let c = self.peek();
        if c == '"' {
            self.quoted()
        } else if c == '{' {
            self.var_ref()
        } else if c.is_alphabetic() {
            self.input_ref()
        } else {
            Err("expected \"string\", {var} or Input#".to_string())
        }
    }

    fn quoted(&mut self) -> Result<StrExpr, String> {
        self.pos += 1;
        let mut sb = String::new();
        while !self.at_end() && self.input[self.pos] != '"' {
            let c = self.input[self.pos];
            let next = if self.pos + 1 < self.input.len() {
                self.input[self.pos + 1]
            } else {
                '\0'
            };
            if c == '\\' && (next == '"' || next == '\\') {
                self.pos += 1;
                sb.push(next);
            } else {
                sb.push(c);
            }
            self.pos += 1;
        }
        if self.peek() == '"' {
            self.pos += 1;
        } else {
            // Upstream StrEval throws on an unclosed string; compile() then
            // yields the EMPTY expression, in both strict and lenient modes.
            return Err("unclosed string".to_string());
        }
        Ok(StrExpr::Lit(sb))
    }

    fn var_ref(&mut self) -> Result<StrExpr, String> {
        self.pos += 1;
        let start = self.pos;
        while !self.at_end() && self.input[self.pos] != '}' {
            self.pos += 1;
        }
        let name: String = self.input[start..self.pos].iter().collect::<String>().trim().to_string();
        if self.peek() == '}' {
            self.pos += 1;
        } else if self.strict {
            return Err("expected }".to_string());
        }
        if self.strict && name.is_empty() {
            return Err("empty variable name".to_string());
        }
        Ok(StrExpr::Var(name))
    }

    fn input_ref(&mut self) -> Result<StrExpr, String> {
        let start = self.pos;
        while !self.at_end()
            && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == '_')
        {
            self.pos += 1;
        }
        let name: String = self.input[start..self.pos].iter().collect();
        if !is_input_ref(&name) {
            return Err(format!("unknown name: {}", name));
        }
        Ok(StrExpr::Input(canonical_input(&name)))
    }

    fn slice_of(&mut self, base: StrExpr) -> Result<StrExpr, String> {
        self.pos += 1; // '['
        let start = self.opt_int();
        self.skip_ws();
        if self.peek() == ']' {
            self.pos += 1;
            if start.is_none() {
                if self.strict {
                    return Err("empty index".to_string());
                }
                return Ok(base);
            }
            return Ok(StrExpr::Index(Box::new(base), start.unwrap()));
        }
        if self.peek() != ':' {
            if self.strict {
                return Err("expected : or ] in slice".to_string());
            }
            return Ok(base);
        }
        self.pos += 1;
        let stop = self.opt_int();
        let mut c: Option<i32> = None;
        self.skip_ws();
        if self.peek() == ':' {
            self.pos += 1;
            c = self.opt_int();
            self.skip_ws();
        }
        if self.peek() == ']' {
            self.pos += 1;
        } else if self.strict {
            return Err("expected ] in slice".to_string());
        }
        Ok(StrExpr::Slice(Box::new(base), start, stop, c))
    }

    // Upstream uses Integer.parseInt here; an overflow digit string would
    // throw and compile to EMPTY. This port yields None instead (treated
    // as a missing bound), a divergence only for absurd inputs.
    fn opt_int(&mut self) -> Option<i32> {
        self.skip_ws();
        let start = self.pos;
        if self.peek() == '-' {
            self.pos += 1;
        }
        while !self.at_end() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == start || (self.pos == start + 1 && self.input[start] == '-') {
            self.pos = start;
            return None;
        }
        let text: String = self.input[start..self.pos].iter().collect();
        text.parse::<i32>().ok()
    }
}

fn str_slice(s: &str, start: Option<i32>, stop: Option<i32>, step: Option<i32>) -> String {
    let st = step.unwrap_or(1);
    if st == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i32;
    let mut out = String::new();
    if st > 0 {
        let b = norm(start, 0, len, false);
        let e = norm(stop, len, len, false);
        let mut i = b;
        while i < e {
            out.push(chars[i as usize]);
            i += st;
        }
    } else {
        let b = norm(start, len - 1, len, true);
        let e = norm(stop, -1, len, true);
        let mut i = b;
        while i > e {
            out.push(chars[i as usize]);
            i += st;
        }
    }
    out
}

fn norm(v: Option<i32>, def: i32, len: i32, neg_step: bool) -> i32 {
    let mut i = match v {
        None => return def,
        Some(x) => x,
    };
    if i < 0 {
        i += len;
    }
    if neg_step {
        if i < 0 {
            return -1;
        }
        if i >= len {
            return len - 1;
        }
    } else {
        if i < 0 {
            return 0;
        }
        if i > len {
            return len;
        }
    }
    i
}

fn str_index(s: &str, i: i32) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut i = i;
    if i < 0 {
        i += chars.len() as i32;
    }
    if i < 0 || i as usize >= chars.len() {
        return String::new();
    }
    chars[i as usize].to_string()
}
