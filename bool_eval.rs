// HeroBot AI runtime bool-expression engine, ported 1:1 from
// herobot-common hero.bane.herobot.common.ai.expr.BoolEval (CC-BY-NC-4.0).
// Clean-room port of grammar and semantics from the upstream Java we read.
//
// Grammar (upstream): or/|| < xor < and/&& < unary (!, not(...)) < primary
// ((expr) | operand [cmpOp operand]*). Operands: quoted strings, numeric
// sub-expressions (the ExprEval grammar, so {var}[idx] and Input<n> refs
// and arithmetic work inside), {var} refs, true/false, and Input<n> names.
// A bare operand is truthy via as_bool; comparison chains conjoin left to
// right. A parse error compiles to FALSE and logs once, like upstream.

use crate::expr::{
    canonical_input, is_input_ref, log_bad_expression, parse_numeric, Expr as NumExpr, ParamKind,
};

/// Upstream hands the vars function raw Java Objects; BoolVal stands in for
/// the shapes the ported runtime produces. Arr is upstream float[]/double[]
/// (our Rot), Other is any other non-null object carried by its
/// String.valueOf form (our Vec3).
#[derive(Clone, Debug)]
pub enum BoolVal {
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<f64>),
    Other(String),
}

pub type VarsFn<'a> = dyn FnMut(&str) -> Option<BoolVal> + 'a;

/// Upstream asBool: Boolean as is, Number non-zero, String parsed as a
/// boolean, any other non-null value true, null false.
pub fn as_bool(v: Option<&BoolVal>) -> bool {
    match v {
        None => false,
        Some(BoolVal::Bool(b)) => *b,
        Some(BoolVal::Num(n)) => *n != 0.0,
        Some(BoolVal::Str(s)) => s.eq_ignore_ascii_case("true"),
        Some(BoolVal::Arr(_)) | Some(BoolVal::Other(_)) => true,
    }
}

fn num_or_len(v: Option<&BoolVal>) -> Option<f64> {
    match v {
        Some(BoolVal::Num(n)) => Some(*n),
        Some(BoolVal::Str(s)) => Some(s.len() as f64),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CmpOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

fn cmp_num(a: f64, b: f64, op: CmpOp) -> bool {
    match op {
        CmpOp::Gt => a > b,
        CmpOp::Lt => a < b,
        CmpOp::Ge => a >= b,
        CmpOp::Le => a <= b,
        _ => false,
    }
}

/// Upstream compare(): two strings compare textually (or by length for
/// ordering); when both sides yield a Number or a String length, compare
/// numerically; when either side is Boolean, fall back to as_bool
/// equality; otherwise fall back to String.valueOf equality.
pub fn compare(a: Option<BoolVal>, b: Option<BoolVal>, op: CmpOp) -> bool {
    let eq_op = op == CmpOp::Eq;
    let ne_op = op == CmpOp::Ne;
    if let (Some(BoolVal::Str(sa)), Some(BoolVal::Str(sb))) = (a.as_ref(), b.as_ref()) {
        if eq_op {
            return sa == sb;
        }
        if ne_op {
            return sa != sb;
        }
        return cmp_num(sa.len() as f64, sb.len() as f64, op);
    }
    if let (Some(x), Some(y)) = (num_or_len(a.as_ref()), num_or_len(b.as_ref())) {
        if eq_op {
            return x == y;
        }
        if ne_op {
            return x != y;
        }
        return cmp_num(x, y, op);
    }
    if matches!(a.as_ref(), Some(BoolVal::Bool(_)))
        || matches!(b.as_ref(), Some(BoolVal::Bool(_)))
    {
        let ba = as_bool(a.as_ref());
        let bb = as_bool(b.as_ref());
        if eq_op {
            return ba == bb;
        }
        if ne_op {
            return ba != bb;
        }
        return false;
    }
    let eq = values_equal(a.as_ref(), b.as_ref());
    if eq_op {
        eq
    } else if ne_op {
        !eq
    } else {
        false
    }
}

fn values_equal(a: Option<&BoolVal>, b: Option<&BoolVal>) -> bool {
    match (a, b) {
        // Upstream Objects.equals(null, null) is true.
        (None, None) => true,
        (Some(BoolVal::Arr(x)), Some(BoolVal::Arr(y))) => x == y,
        (Some(x), Some(y)) => value_string(x) == value_string(y),
        _ => false,
    }
}

fn value_string(v: &BoolVal) -> String {
    match v {
        BoolVal::Bool(b) => b.to_string(),
        BoolVal::Num(n) => n.to_string(),
        BoolVal::Str(s) => s.clone(),
        BoolVal::Arr(f) => f
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(" "),
        BoolVal::Other(s) => s.clone(),
    }
}

/// Upstream refToDouble: only array values index (our Rot); elsewhere a
/// Boolean is 1/0, a Number its value, a String its length, anything else 0.
pub fn ref_to_double(v: Option<BoolVal>, idx: i32) -> f64 {
    if idx >= 0 {
        return match v {
            Some(BoolVal::Arr(f)) => {
                if (idx as usize) < f.len() {
                    f[idx as usize]
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
    }
    match v {
        Some(BoolVal::Bool(b)) => {
            if b {
                1.0
            } else {
                0.0
            }
        }
        Some(BoolVal::Num(n)) => n,
        Some(BoolVal::Str(s)) => s.len() as f64,
        _ => 0.0,
    }
}

#[derive(Clone, Debug)]
pub enum OperandKind {
    Var(String),
    Str(String),
    BoolConst(bool),
    Num(NumExpr),
}

#[derive(Clone, Debug)]
pub struct Operand {
    pub kind: OperandKind,
    pub ty: Option<ParamKind>,
}

#[derive(Clone, Debug)]
pub enum BoolExpr {
    False,
    Not(Box<BoolExpr>),
    Or(Box<BoolExpr>, Box<BoolExpr>),
    Xor(Box<BoolExpr>, Box<BoolExpr>),
    And(Box<BoolExpr>, Box<BoolExpr>),
    /// A bare operand, truthy via as_bool.
    Atom(Operand),
    /// compare(a, op, b).
    Cmp(Box<Operand>, CmpOp, Box<Operand>),
}

impl BoolExpr {
    pub fn eval(&self, vars: &mut VarsFn) -> bool {
        match self {
            BoolExpr::False => false,
            BoolExpr::Not(a) => !a.eval(&mut *vars),
            BoolExpr::Or(a, b) => a.eval(&mut *vars) || b.eval(&mut *vars),
            BoolExpr::Xor(a, b) => a.eval(&mut *vars) ^ b.eval(&mut *vars),
            BoolExpr::And(a, b) => a.eval(&mut *vars) && b.eval(&mut *vars),
            BoolExpr::Atom(op) => as_bool(operand_value(op, vars).as_ref()),
            BoolExpr::Cmp(a, o, b) => {
                compare(operand_value(a, vars), operand_value(b, vars), *o)
            }
        }
    }
}

fn operand_value(op: &Operand, vars: &mut VarsFn) -> Option<BoolVal> {
    match &op.kind {
        OperandKind::Var(name) => vars(name),
        OperandKind::Str(s) => Some(BoolVal::Str(s.clone())),
        OperandKind::BoolConst(b) => Some(BoolVal::Bool(*b)),
        OperandKind::Num(e) => {
            let mut sub = |n: &str, idx: i32| ref_to_double(vars(n), idx);
            Some(BoolVal::Num(e.eval(&mut sub)))
        }
    }
}

/// Upstream Parser (BoolEval).
pub struct Parser<'a> {
    src: &'a str,
    chars: Vec<char>,
    strict: bool,
    types: Option<&'a dyn Fn(&str) -> Option<ParamKind>>,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(
        src: &'a str,
        strict: bool,
        types: Option<&'a dyn Fn(&str) -> Option<ParamKind>>,
    ) -> Self {
        Parser {
            src,
            chars: src.chars().collect(),
            strict,
            types,
            pos: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn lookahead(&self, tok: &str) -> bool {
        let n = tok.chars().count();
        if self.pos + n > self.chars.len() {
            return false;
        }
        self.chars[self.pos..self.pos + n]
            .iter()
            .copied()
            .eq(tok.chars())
    }

    /// Upstream matchWord: whitespace-skipping, case-insensitive word match
    /// that must not be followed by another word character.
    fn match_word(&mut self, w: &str) -> bool {
        self.skip_ws();
        let n = w.len();
        if self.pos + n > self.chars.len() {
            return false;
        }
        for (i, wc) in w.chars().enumerate() {
            let c = self.chars[self.pos + i];
            if !c.eq_ignore_ascii_case(&wc) {
                return false;
            }
        }
        let end = self.pos + n;
        if end < self.chars.len() {
            let c = self.chars[end];
            if c.is_alphanumeric() || c == '_' {
                return false;
            }
        }
        self.pos = end;
        true
    }

    fn match_symbol(&mut self, sym: &str) -> bool {
        self.skip_ws();
        if !self.lookahead(sym) {
            return false;
        }
        self.pos += sym.chars().count();
        true
    }

    pub fn or_expr(&mut self) -> Result<BoolExpr, String> {
        let mut v = self.xor_expr()?;
        loop {
            if self.match_word("or") || self.match_symbol("||") {
                let l = v;
                let r = self.xor_expr()?;
                v = BoolExpr::Or(Box::new(l), Box::new(r));
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn xor_expr(&mut self) -> Result<BoolExpr, String> {
        let mut v = self.and_expr()?;
        while self.match_word("xor") {
            let l = v;
            let r = self.and_expr()?;
            v = BoolExpr::Xor(Box::new(l), Box::new(r));
        }
        Ok(v)
    }

    fn and_expr(&mut self) -> Result<BoolExpr, String> {
        let mut v = self.unary()?;
        loop {
            if self.match_word("and") || self.match_symbol("&&") {
                let l = v;
                let r = self.unary()?;
                v = BoolExpr::And(Box::new(l), Box::new(r));
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn unary(&mut self) -> Result<BoolExpr, String> {
        self.skip_ws();
        if self.peek() == Some('!') && !self.lookahead("!=") {
            self.pos += 1;
            let a = self.unary()?;
            return Ok(BoolExpr::Not(Box::new(a)));
        }
        if self.match_word("not") {
            self.skip_ws();
            if self.peek() != Some('(') {
                return Err("not needs (...)".to_string());
            }
            self.pos += 1;
            let a = self.or_expr()?;
            self.skip_ws();
            if self.peek() == Some(')') {
                self.pos += 1;
            } else if self.strict {
                return Err("expected )".to_string());
            }
            return Ok(BoolExpr::Not(Box::new(a)));
        }
        self.primary()
    }

    fn cmp_op(&mut self) -> Option<CmpOp> {
        self.skip_ws();
        if self.lookahead("==") {
            self.pos += 2;
            return Some(CmpOp::Eq);
        }
        if self.lookahead("!=") {
            self.pos += 2;
            return Some(CmpOp::Ne);
        }
        if self.lookahead(">=") {
            self.pos += 2;
            return Some(CmpOp::Ge);
        }
        if self.lookahead("<=") {
            self.pos += 2;
            return Some(CmpOp::Le);
        }
        match self.peek() {
            Some('>') => {
                self.pos += 1;
                Some(CmpOp::Gt)
            }
            Some('<') => {
                self.pos += 1;
                Some(CmpOp::Lt)
            }
            Some('=') => {
                self.pos += 1;
                Some(CmpOp::Eq)
            }
            _ => None,
        }
    }

    fn primary(&mut self) -> Result<BoolExpr, String> {
        self.skip_ws();
        if self.peek() == Some('(') {
            self.pos += 1;
            let v = self.or_expr()?;
            self.skip_ws();
            if self.peek() == Some(')') {
                self.pos += 1;
            } else if self.strict {
                return Err("expected )".to_string());
            }
            return Ok(v);
        }
        let a = self.operand()?;
        let op = match self.cmp_op() {
            Some(o) => o,
            None => {
                if self.strict && self.types.is_some() {
                    if let Some(t) = a.ty {
                        if t != ParamKind::Bool {
                            return Err("not a boolean".to_string());
                        }
                    }
                }
                return Ok(BoolExpr::Atom(a));
            }
        };
        let b = self.operand()?;
        if self.strict && self.types.is_some() {
            self.check_comparable(a.ty, b.ty, op)?;
        }
        let mut chain = BoolExpr::Cmp(Box::new(a), op, Box::new(b.clone()));
        let mut prev = b;
        loop {
            let next_op = match self.cmp_op() {
                Some(o) => o,
                None => break,
            };
            let next = self.operand()?;
            if self.strict && self.types.is_some() {
                self.check_comparable(prev.ty, next.ty, next_op)?;
            }
            let link = BoolExpr::Cmp(Box::new(prev), next_op, Box::new(next.clone()));
            chain = BoolExpr::And(Box::new(chain), Box::new(link));
            prev = next;
        }
        Ok(chain)
    }

    fn check_comparable(
        &self,
        a: Option<ParamKind>,
        b: Option<ParamKind>,
        op: CmpOp,
    ) -> Result<(), String> {
        let (a, b) = match (a, b) {
            (Some(a), Some(b)) => (a, b),
            _ => return Ok(()),
        };
        let a_flex = matches!(a, ParamKind::Num | ParamKind::Str);
        let b_flex = matches!(b, ParamKind::Num | ParamKind::Str);
        if a_flex && b_flex {
            return Ok(());
        }
        let eq_op = matches!(op, CmpOp::Eq | CmpOp::Ne);
        if types_same(a, b) && eq_op {
            return Ok(());
        }
        Err(format!("cannot compare {:?} {:?} {:?}", a, op, b))
    }

    fn operand(&mut self) -> Result<Operand, String> {
        self.skip_ws();
        let c = match self.peek() {
            Some(c) => c,
            None => return Err("expected a value".to_string()),
        };
        if c == '"' {
            return self.quoted();
        }
        let start = self.pos;
        if c == '-' || c == '.' || c.is_ascii_digit() || self.function_call_ahead() {
            return self.numeric(start);
        }
        if c != '{' && !c.is_alphabetic() && c != '_' {
            return Err("expected a value".to_string());
        }
        let a = if c == '{' {
            self.var_ref()?
        } else {
            self.identifier()?
        };
        self.skip_ws();
        if self.arith_op_ahead() {
            self.numeric(start)
        } else {
            Ok(a)
        }
    }

    fn function_call_ahead(&self) -> bool {
        let mut p = self.pos;
        match self.chars.get(p) {
            Some(&c) if c.is_alphabetic() || c == '_' => {}
            _ => return false,
        }
        while let Some(&c) = self.chars.get(p) {
            if c.is_alphanumeric() || c == '_' {
                p += 1;
            } else {
                break;
            }
        }
        while let Some(&c) = self.chars.get(p) {
            if c.is_whitespace() {
                p += 1;
            } else {
                break;
            }
        }
        self.chars.get(p) == Some(&'(')
    }

    fn arith_op_ahead(&self) -> bool {
        matches!(
            self.peek(),
            Some('+') | Some('-') | Some('*') | Some('/') | Some('%') | Some('^')
        )
    }
    fn numeric(&mut self, start: usize) -> Result<Operand, String> {
        let sub = parse_numeric(self.src, start, self.strict, self.types)?;
        self.pos = sub.end;
        Ok(Operand {
            kind: OperandKind::Num(sub.expr),
            ty: Some(ParamKind::Num),
        })
    }

    fn var_ref(&mut self) -> Result<Operand, String> {
        self.pos += 1;
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == '}' {
                break;
            }
            self.pos += 1;
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        let name = name.trim().to_string();
        if self.peek() == Some('}') {
            self.pos += 1;
        } else if self.strict {
            return Err("expected }".to_string());
        }
        if self.strict && name.is_empty() {
            return Err("empty variable name".to_string());
        }
        let ty = self.types.and_then(|t| t(&name));
        Ok(Operand {
            kind: OperandKind::Var(name),
            ty,
        })
    }

    fn quoted(&mut self) -> Result<Operand, String> {
        self.pos += 1;
        let mut sb = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                break;
            }
            if c == '\\' && self.pos + 1 < self.chars.len() {
                self.pos += 1;
                sb.push(self.chars[self.pos]);
            } else {
                sb.push(c);
            }
            self.pos += 1;
        }
        if self.peek() == Some('"') {
            self.pos += 1;
        } else if self.strict {
            return Err("unclosed string".to_string());
        }
        Ok(Operand {
            kind: OperandKind::Str(sb),
            ty: Some(ParamKind::Str),
        })
    }

    fn identifier(&mut self) -> Result<Operand, String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        if name.eq_ignore_ascii_case("true") {
            return Ok(Operand {
                kind: OperandKind::BoolConst(true),
                ty: Some(ParamKind::Bool),
            });
        }
        if name.eq_ignore_ascii_case("false") {
            return Ok(Operand {
                kind: OperandKind::BoolConst(false),
                ty: Some(ParamKind::Bool),
            });
        }
        if is_input_ref(&name) {
            let canonical = canonical_input(&name);
            let ty = self.types.and_then(|t| t(&canonical));
            return Ok(Operand {
                kind: OperandKind::Var(canonical),
                ty,
            });
        }
        Err(format!("unknown name: {}", name))
    }

    fn parse_full(&mut self) -> Result<BoolExpr, String> {
        let e = self.or_expr()?;
        self.skip_ws();
        if self.at_end() {
            Ok(e)
        } else {
            Err("unexpected trailing input".to_string())
        }
    }
}

fn types_same(a: ParamKind, b: ParamKind) -> bool {
    if a == b {
        return true;
    }
    matches!(a, ParamKind::Num) && matches!(b, ParamKind::Num)
}

/// Upstream eval(): compile (cached) then evaluate.
pub fn eval(expression: &str, vars: &mut VarsFn) -> bool {
    compile(expression).eval(vars)
}

/// Upstream compile(): blank strings and parse errors compile to FALSE and
/// the error is logged once; results are memoized (limit 256, cleared).
pub fn compile(expression: &str) -> BoolExpr {
    if expression.trim().is_empty() {
        return BoolExpr::False;
    }
    if let Some(hit) = CACHE.with(|c| c.borrow_mut().get(expression).cloned()) {
        return hit;
    }
    let compiled = match Parser::new(expression, true, None).parse_full() {
        Ok(e) => e,
        Err(reason) => {
            log_bad_expression(expression, &reason);
            BoolExpr::False
        }
    };
    CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        if cache.len() >= 256 {
            cache.clear();
        }
        cache.insert(expression.to_string(), compiled.clone());
    });
    compiled
}

/// Upstream isValid: parses with types for editor-side validation.
pub fn is_valid(expression: &str, types: Option<&dyn Fn(&str) -> Option<ParamKind>>) -> bool {
    if expression.trim().is_empty() {
        return true;
    }
    let mut p = Parser::new(expression, true, types);
    match p.or_expr() {
        Ok(_) => {
            p.skip_ws();
            p.at_end()
        }
        Err(_) => false,
    }
}

thread_local! {
    static CACHE: std::cell::RefCell<std::collections::HashMap<String, BoolExpr>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}
