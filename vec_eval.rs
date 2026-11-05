// Port of herobot-common VecEval.java (CC-BY-NC-4.0, HerobaneNair/HeRoBot).
// Vector expression engine behind POS_CALC and DIR_CALC: pos(x,y,z) and
// dir(y,p) relative constructors with ~ offsets and ^ local coordinates,
// element-wise + - * / % with scalar broadcast, per-component math calls,
// climb/fall terrain scans, and {var}/Input<n> refs. Compile errors fall
// back to the ZERO expression; runtime arity mismatches fall back to a zero
// vector of the required length (upstream MISMATCH catch semantics).

use crate::expr::{canonical_input, is_input_ref, rand_unit};
use std::cell::RefCell;
use std::collections::HashMap;

pub const OPS_LEGEND: &str = "pos(x,y,z)  dir(y,p)  +  -  *  /  %";
pub const OPS_LEGEND_2: &str =
    "climb(pos)  fall(pos)  floor(vector)  {var}  Input1  ~  ^";

const CACHE_LIMIT: usize = 256;

/// World seam for terrain scans and ~ / ^ bases; the game layer supplies it
/// (upstream: the player's Level, plus the player position and rotation for
/// the base values).
pub trait World {
    fn is_air(&self, x: i32, y: i32, z: i32) -> bool;
    fn min_y(&self) -> i32;
    fn max_y(&self) -> i32;
    /// Upstream default: no base information (all zeros).
    fn base(&self, axis: i32, rotation: bool) -> f64 {
        let _ = (axis, rotation);
        0.0
    }
}

/// What a {var} or Input<n> ref yields before coercion (upstream Object).
#[derive(Clone, Debug)]
pub enum VecVal {
    /// double[]: a Vec3 variable as [x, y, z].
    Arr(Vec<f64>),
    /// float[]: a Rot variable as [yaw, pitch].
    Rot([f64; 2]),
    Num(f64),
    Bool(bool),
    /// Anything else coerces to [0].
    Other,
}

/// Upstream coerce: double[] keeps its shape, float[] widens, Number and
/// Boolean become 1-length arrays, everything else is [0].
pub fn coerce(v: VecVal) -> Vec<f64> {
    match v {
        VecVal::Arr(a) => a,
        VecVal::Rot(r) => r.to_vec(),
        VecVal::Num(n) => vec![n],
        VecVal::Bool(b) => vec![if b { 1.0 } else { 0.0 }],
        VecVal::Other => vec![0.0],
    }
}

/// Ref type callback for is_valid (upstream Function<String, ParamType>).
pub enum RefType {
    Position,
    Rotation,
    Number,
    /// Any other parameter type is rejected in strict mode.
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp { Add, Sub, Mul, Div, Mod, Pow }

#[derive(Clone, Copy, Debug)]
pub enum MapOp { Floor, Ceil, Abs, Sqrt, Sin, Cos, Tan, Asin, Acos, Atan, Neg }

fn map_apply(op: MapOp, x: f64) -> f64 {
    match op {
        MapOp::Floor => x.floor(),
        MapOp::Ceil => x.ceil(),
        MapOp::Abs => x.abs(),
        MapOp::Sqrt => x.sqrt(),
        MapOp::Sin => x.sin(),
        MapOp::Cos => x.cos(),
        MapOp::Tan => x.tan(),
        MapOp::Asin => x.asin(),
        MapOp::Acos => x.acos(),
        MapOp::Atan => x.atan(),
        MapOp::Neg => -x,
    }
}

/// Element-wise apply with 1-vs-n scalar broadcast; two different multi
/// component shapes throw (upstream MISMATCH).
fn apply_op(a: &[f64], b: &[f64], op: BinOp) -> Result<Vec<f64>, ()> {
    let f = |x: f64, y: f64| match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div => {
            if y == 0.0 { 0.0 } else { x / y }
        }
        BinOp::Mod => {
            if y == 0.0 { 0.0 } else { x % y }
        }
        BinOp::Pow => x.powf(y),
    };
    if a.len() == b.len() {
        return Ok(a.iter().zip(b.iter()).map(|(x, y)| f(*x, *y)).collect());
    }
    if a.len() == 1 {
        return Ok(b.iter().map(|y| f(a[0], *y)).collect());
    }
    if b.len() == 1 {
        return Ok(a.iter().map(|x| f(*x, b[0])).collect());
    }
    Err(())
}

/// One pos(...) / dir(...) argument: an expression, a ~ offset, or a ^ local
/// offset. Upstream keeps three parallel lists (parts, offsets, carrots);
/// exactly one of the three is set per slot.
#[derive(Clone, Debug)]
pub enum RelArg {
    Expr(VecNode),
    Tilde(f64),
    Carrot(f64),
}

#[derive(Clone, Debug)]
pub enum VecNode {
    /// Upstream ZERO: [0], arity 1.
    Zero,
    Const(Vec<f64>, usize),
    /// rand / random evaluate per call (upstream Math.random()).
    Rand,
    Ref(String, usize),
    Bin(Box<VecNode>, Box<VecNode>, BinOp, usize),
    Map(Box<VecNode>, MapOp, usize),
    Rel(usize, Vec<RelArg>),
    /// climb / fall terrain scan; the bool is up.
    Terrain(bool, Box<VecNode>),
}

impl VecNode {
    fn num_args(&self) -> usize {
        match self {
            VecNode::Zero => 1,
            VecNode::Const(_, n) => *n,
            VecNode::Rand => 1,
            VecNode::Ref(_, n) => *n,
            VecNode::Bin(_, _, _, n) => *n,
            VecNode::Map(_, _, n) => *n,
            VecNode::Rel(n, _) => *n,
            VecNode::Terrain(..) => 3,
        }
    }

    /// Upstream Expr::eval; Err is the MISMATCH path.
    pub fn eval(
        &self,
        vars: &mut dyn FnMut(&str) -> Option<VecVal>,
        world: Option<&dyn World>,
    ) -> Result<Vec<f64>, ()> {
        match self {
            VecNode::Zero => Ok(vec![0.0]),
            VecNode::Const(v, _) => Ok(v.clone()),
            VecNode::Rand => Ok(vec![rand_unit()]),
            VecNode::Ref(name, _) => Ok(vars(name).map(coerce).unwrap_or_else(|| vec![0.0])),
            VecNode::Bin(l, r, op, _) => {
                let a = l.eval(vars, world)?;
                let b = r.eval(vars, world)?;
                apply_op(&a, &b, *op)
            }
            VecNode::Map(n, op, _) => {
                let v = n.eval(vars, world)?;
                Ok(v.iter().map(|x| map_apply(*op, *x)).collect())
            }
            VecNode::Rel(n, args) => {
                let rotation = *n == 2;
                let carrot_count = args.iter().filter(|a| matches!(a, RelArg::Carrot(_))).count();
                let use_carrot = carrot_count == *n && *n == 3;
                if use_carrot {
                    let get = |i: usize| -> f64 {
                        match args.get(i) {
                            Some(RelArg::Carrot(x)) => *x,
                            _ => 0.0,
                        }
                    };
                    let (left, up, forwards) = (get(0), get(1), get(2));
                    return match world {
                        None => Ok(vec![left, up, forwards]),
                        Some(w) => {
                            let yaw = w.base(0, true);
                            let pitch = w.base(1, true);
                            let local = local_offset(yaw, pitch, left, up, forwards);
                            Ok(vec![
                                w.base(0, false) + local[0],
                                w.base(1, false) + local[1],
                                w.base(2, false) + local[2],
                            ])
                        }
                    };
                }
                let mut out = vec![0.0; *n];
                for i in 0..args.len().min(*n) {
                    match &args[i] {
                        RelArg::Tilde(off) => {
                            let basis = match world {
                                None => 0.0,
                                Some(w) => w.base(i as i32, rotation),
                            };
                            out[i] = basis + off;
                        }
                        RelArg::Expr(e) => {
                            let v = e.eval(vars, world)?;
                            out[i] = v[0];
                        }
                        // A ^ slot that never reached the useCarrot path is
                        // ignored by upstream (its lists are null): 0.
                        RelArg::Carrot(_) => {}
                    }
                }
                Ok(out)
            }
            VecNode::Terrain(up, arg) => {
                let p = arg.eval(vars, world)?;
                if p.len() != 3 {
                    return Err(());
                }
                let w = match world {
                    None => return Ok(p),
                    Some(w) => w,
                };
                let bx = p[0].floor() as i32;
                let bz = p[2].floor() as i32;
                let start_y = p[1].floor() as i32;
                let mut y = start_y;
                if *up {
                    if w.is_air(bx, y, bz) {
                        return Ok(p);
                    }
                    while y < w.max_y() && !w.is_air(bx, y, bz) {
                        y += 1;
                    }
                } else {
                    if !w.is_air(bx, y, bz) {
                        return Ok(p);
                    }
                    while y - 1 >= w.min_y() && w.is_air(bx, y - 1, bz) {
                        y -= 1;
                    }
                }
                if y == start_y {
                    Ok(p)
                } else {
                    Ok(vec![p[0], y as f64, p[2]])
                }
            }
        }
    }
}

/// Upstream localOffset: world-space offset from left / up / forwards in the
/// player's view basis.
pub fn local_offset(yaw_deg: f64, pitch_deg: f64, left: f64, up: f64, forwards: f64) -> [f64; 3] {
    let f = (yaw_deg + 90.0).to_radians().cos();
    let g = (yaw_deg + 90.0).to_radians().sin();
    let h = (-pitch_deg).to_radians().cos();
    let i = (-pitch_deg).to_radians().sin();
    let j = (-pitch_deg + 90.0).to_radians().cos();
    let k = (-pitch_deg + 90.0).to_radians().sin();
    let fwd_basis = [f * h, i, g * h];
    let up_basis = [f * j, k, g * j];
    let l = cross(&fwd_basis, &up_basis);
    let left_basis = [-l[0], -l[1], -l[2]];
    [
        fwd_basis[0] * forwards + up_basis[0] * up + left_basis[0] * left,
        fwd_basis[1] * forwards + up_basis[1] * up + left_basis[1] * left,
        fwd_basis[2] * forwards + up_basis[2] * up + left_basis[2] * left,
    ]
}

fn cross(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn combine_num_args(a: usize, b: usize) -> Result<usize, String> {
    if a == b {
        return Ok(a);
    }
    if a == 0 {
        return Ok(if b == 1 { 0 } else { b });
    }
    if b == 0 {
        return Ok(if a == 1 { 0 } else { a });
    }
    if a == 1 {
        return Ok(b);
    }
    if b == 1 {
        return Ok(a);
    }
    Err("cannot mix a position with a direction".to_string())
}

fn bin(l: VecNode, r: VecNode, op: BinOp) -> Result<VecNode, String> {
    let num_args = combine_num_args(l.num_args(), r.num_args())?;
    Ok(VecNode::Bin(Box::new(l), Box::new(r), op, num_args))
}

struct Parser<'a> {
    input: Vec<char>,
    pos: usize,
    strict: bool,
    required_num_args: usize,
    types: Option<&'a dyn Fn(&str) -> Option<RefType>>,
}

impl<'a> Parser<'a> {
    fn new(
        src: &str,
        strict: bool,
        required_num_args: usize,
        types: Option<&'a dyn Fn(&str) -> Option<RefType>>,
    ) -> Parser<'a> {
        Parser { input: src.chars().collect(), pos: 0, strict, required_num_args, types }
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

    fn expr(&mut self) -> Result<VecNode, String> {
        let mut v = self.term()?;
        loop {
            self.skip_ws();
            let c = self.peek();
            if c == '+' {
                self.pos += 1;
                let r = self.term()?;
                v = bin(v, r, BinOp::Add)?;
            } else if c == '-' {
                self.pos += 1;
                let r = self.term()?;
                v = bin(v, r, BinOp::Sub)?;
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn term(&mut self) -> Result<VecNode, String> {
        let mut v = self.unary()?;
        loop {
            self.skip_ws();
            let c = self.peek();
            if c == '*' {
                self.pos += 1;
                let r = self.unary()?;
                v = bin(v, r, BinOp::Mul)?;
            } else if c == '/' {
                self.pos += 1;
                let r = self.unary()?;
                v = bin(v, r, BinOp::Div)?;
            } else if c == '%' {
                self.pos += 1;
                let r = self.unary()?;
                v = bin(v, r, BinOp::Mod)?;
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn unary(&mut self) -> Result<VecNode, String> {
        self.skip_ws();
        if self.peek() == '-' {
            self.pos += 1;
            let n = self.unary()?;
            let na = n.num_args();
            return Ok(VecNode::Map(Box::new(n), MapOp::Neg, na));
        }
        if self.peek() == '+' {
            self.pos += 1;
            return self.unary();
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<VecNode, String> {
        self.skip_ws();
        let c = self.peek();
        if c == '(' {
            self.pos += 1;
            let v = self.expr()?;
            self.skip_ws();
            if self.peek() == ')' {
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

    fn variable(&mut self) -> Result<VecNode, String> {
        self.pos += 1;
        let start = self.pos;
        while !self.at_end() && self.input[self.pos] != '}' {
            self.pos += 1;
        }
        let raw: String = self.input[start..self.pos].iter().collect();
        let name = raw.trim().to_string();
        if self.peek() == '}' {
            self.pos += 1;
        } else if self.strict {
            return Err("expected }".to_string());
        }
        self.ref_node(&name)
    }

    fn ref_node(&mut self, name: &str) -> Result<VecNode, String> {
        let num_args = match self.types {
            None => 0,
            Some(t) => match t(name) {
                None => 0,
                Some(RefType::Position) => 3,
                Some(RefType::Rotation) => 2,
                Some(RefType::Number) => 1,
                Some(RefType::Other) => {
                    return Err(format!("{} is not a number, position, or direction", name));
                }
            },
        };
        Ok(VecNode::Ref(name.to_string(), num_args))
    }

    fn number(&mut self) -> Result<VecNode, String> {
        let start = self.pos;
        while !self.at_end()
            && (self.input[self.pos].is_ascii_digit() || self.input[self.pos] == '.')
        {
            self.pos += 1;
        }
        let text: String = self.input[start..self.pos].iter().collect();
        let val = text.parse::<f64>().map_err(|_| "bad number".to_string())?;
        Ok(VecNode::Const(vec![val], 1))
    }

    fn identifier(&mut self) -> Result<VecNode, String> {
        let start = self.pos;
        while !self.at_end()
            && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == '_')
        {
            self.pos += 1;
        }
        let name: String = self.input[start..self.pos].iter().collect();
        self.skip_ws();
        if self.peek() == '(' {
            return self.call_function(&name);
        }
        self.constant(&name)
    }

    fn constant(&mut self, name: &str) -> Result<VecNode, String> {
        if is_input_ref(name) {
            return self.ref_node(&canonical_input(name));
        }
        match name.to_lowercase().as_str() {
            "pi" => Ok(VecNode::Const(vec![std::f64::consts::PI], 1)),
            "e" => Ok(VecNode::Const(vec![std::f64::consts::E], 1)),
            "tau" => Ok(VecNode::Const(vec![std::f64::consts::TAU], 1)),
            "rand" | "random" => Ok(VecNode::Rand),
            _ => {
                if self.strict {
                    return Err(format!("unknown name: {}", name));
                }
                Ok(VecNode::Zero)
            }
        }
    }

    fn call_function(&mut self, name: &str) -> Result<VecNode, String> {
        let lower = name.to_lowercase();
        if lower == "pos" {
            return self.rel_construct(3);
        }
        if lower == "dir" {
            return self.rel_construct(2);
        }
        let mut a = self.args()?;
        match lower.as_str() {
            "floor" => self.per_component(a, MapOp::Floor),
            "ceil" => self.per_component(a, MapOp::Ceil),
            "abs" => self.per_component(a, MapOp::Abs),
            "sqrt" => self.per_component(a, MapOp::Sqrt),
            "sin" => self.per_component(a, MapOp::Sin),
            "cos" => self.per_component(a, MapOp::Cos),
            "tan" => self.per_component(a, MapOp::Tan),
            "asin" | "arcsin" => self.per_component(a, MapOp::Asin),
            "acos" | "arccos" => self.per_component(a, MapOp::Acos),
            "atan" | "arctan" => self.per_component(a, MapOp::Atan),
            "pow" => {
                if self.strict && a.len() != 2 {
                    return Err("pow needs 2 arguments".to_string());
                }
                if a.is_empty() {
                    return Ok(VecNode::Zero);
                }
                if a.len() < 2 {
                    return Ok(a.into_iter().next().unwrap());
                }
                if self.strict && (a[0].num_args() > 1 || a[1].num_args() > 1) {
                    return Err("pow arguments must be numbers".to_string());
                }
                let l = a.remove(0);
                let r = a.remove(0);
                bin(l, r, BinOp::Pow)
            }
            "climb" => self.terrain_node(a, true),
            "fall" => self.terrain_node(a, false),
            _ => {
                if self.strict {
                    return Err(format!("unknown function: {}", name));
                }
                Ok(VecNode::Zero)
            }
        }
    }

    fn args(&mut self) -> Result<Vec<VecNode>, String> {
        self.pos += 1; // '('
        let mut list = Vec::new();
        self.skip_ws();
        if self.peek() == ')' {
            self.pos += 1;
            return Ok(list);
        }
        list.push(self.expr()?);
        self.skip_ws();
        while self.peek() == ',' {
            self.pos += 1;
            list.push(self.expr()?);
            self.skip_ws();
        }
        if self.peek() == ')' {
            self.pos += 1;
        } else if self.strict {
            return Err("expected )".to_string());
        }
        Ok(list)
    }

    fn per_component(&mut self, a: Vec<VecNode>, op: MapOp) -> Result<VecNode, String> {
        if self.strict && a.len() != 1 {
            return Err("needs 1 argument".to_string());
        }
        if a.is_empty() {
            return Ok(VecNode::Zero);
        }
        let n = a.into_iter().next().unwrap();
        let na = n.num_args();
        Ok(VecNode::Map(Box::new(n), op, na))
    }

    fn terrain_node(&mut self, a: Vec<VecNode>, up: bool) -> Result<VecNode, String> {
        if self.strict {
            if self.required_num_args != 3 {
                return Err(format!(
                    "{} only works in pos calc",
                    if up { "climb" } else { "fall" }
                ));
            }
            if a.len() != 1 {
                return Err("needs 1 argument".to_string());
            }
            let na = a[0].num_args();
            if na != 0 && na != 3 {
                return Err("argument must be a position".to_string());
            }
        }
        if a.is_empty() {
            return Ok(VecNode::Zero);
        }
        Ok(VecNode::Terrain(up, Box::new(a.into_iter().next().unwrap())))
    }

    fn rel_construct(&mut self, n: usize) -> Result<VecNode, String> {
        self.pos += 1; // '('
        let mut parts: Vec<Option<VecNode>> = Vec::new();
        let mut offsets: Vec<Option<f64>> = Vec::new();
        let mut carrots: Vec<Option<f64>> = Vec::new();
        self.skip_ws();
        if self.peek() != ')' {
            self.parse_rel_arg(&mut parts, &mut offsets, &mut carrots)?;
            self.skip_ws();
            while self.peek() == ',' {
                self.pos += 1;
                self.parse_rel_arg(&mut parts, &mut offsets, &mut carrots)?;
                self.skip_ws();
            }
        }
        if self.peek() == ')' {
            self.pos += 1;
        } else if self.strict {
            return Err("expected )".to_string());
        }
        if self.strict && parts.len() != n {
            return Err(format!(
                "{} needs {} numbers",
                if n == 3 { "pos" } else { "dir" },
                n
            ));
        }
        let carrot_count = carrots.iter().filter(|c| c.is_some()).count();
        if carrot_count > 0 {
            if self.strict && n != 3 {
                return Err("^ local coordinates only work in pos calc".to_string());
            }
            if self.strict && carrot_count != n {
                return Err("cannot mix ^ with ~ or absolute coordinates".to_string());
            }
        }
        let mut args = Vec::with_capacity(parts.len());
        for i in 0..parts.len() {
            if let Some(o) = offsets[i] {
                args.push(RelArg::Tilde(o));
            } else if let Some(p) = parts[i].take() {
                args.push(RelArg::Expr(p));
            } else {
                args.push(RelArg::Carrot(carrots[i].unwrap_or(0.0)));
            }
        }
        Ok(VecNode::Rel(n, args))
    }

    #[allow(clippy::type_complexity)]
    fn parse_rel_arg(
        &mut self,
        parts: &mut Vec<Option<VecNode>>,
        offsets: &mut Vec<Option<f64>>,
        carrots: &mut Vec<Option<f64>>,
    ) -> Result<(), String> {
        self.skip_ws();
        let c = self.peek();
        if c == '^' || c == '~' {
            self.pos += 1;
            let mut offset = 0.0;
            let c2 = self.peek();
            if c2 == '-' || c2 == '+' || c2 == '.' || c2.is_ascii_digit() {
                let start = self.pos;
                if c2 == '-' || c2 == '+' {
                    self.pos += 1;
                }
                while !self.at_end()
                    && (self.input[self.pos].is_ascii_digit() || self.input[self.pos] == '.')
                {
                    self.pos += 1;
                }
                let text: String = self.input[start..self.pos].iter().collect();
                match text.parse::<f64>() {
                    Ok(v) => offset = v,
                    Err(_) => {
                        if self.strict {
                            return Err(if c == '^' {
                                "bad ^ offset".to_string()
                            } else {
                                "bad ~ offset".to_string()
                            });
                        }
                    }
                }
            }
            if c == '^' {
                parts.push(None);
                offsets.push(None);
                carrots.push(Some(offset));
            } else {
                parts.push(None);
                offsets.push(Some(offset));
                carrots.push(None);
            }
        } else {
            let arg = self.expr()?;
            if self.strict && arg.num_args() > 1 {
                return Err("arguments must be numbers".to_string());
            }
            parts.push(Some(arg));
            offsets.push(None);
            carrots.push(None);
        }
        Ok(())
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<String, VecNode>> = RefCell::new(HashMap::new());
}

/// Upstream eval: compile (bad expressions become ZERO), run, and fall back
/// to a zero vector of the required length on any mismatch or error.
pub fn eval(
    expression: &str,
    required_num_args: usize,
    vars: &mut dyn FnMut(&str) -> Option<VecVal>,
    world: Option<&dyn World>,
) -> Vec<f64> {
    let v = compile(expression).eval(vars, world).ok();
    match v {
        Some(v) if v.len() == required_num_args => v,
        _ => vec![0.0; required_num_args],
    }
}

/// Upstream isValid: strict parse plus arity check against the parameter
/// types map.
pub fn is_valid(
    expression: &str,
    required_num_args: usize,
    types: &dyn Fn(&str) -> Option<RefType>,
) -> bool {
    if expression.trim().is_empty() {
        return true;
    }
    let mut p = Parser::new(expression, true, required_num_args, Some(types));
    match p.expr() {
        Ok(n) => {
            p.skip_ws();
            if !p.at_end() {
                return false;
            }
            n.num_args() == 0 || n.num_args() == required_num_args
        }
        Err(_) => false,
    }
}

/// Upstream compile: cache of 256 entries, cleared wholesale when full.
pub fn compile(expression: &str) -> VecNode {
    if expression.trim().is_empty() {
        return VecNode::Zero;
    }
    if let Some(cached) = CACHE.with(|c| c.borrow().get(expression).cloned()) {
        return cached;
    }
    let mut compiled = VecNode::Zero;
    let mut p = Parser::new(expression, false, 0, None);
    if let Ok(n) = p.expr() {
        p.skip_ws();
        if p.at_end() {
            compiled = n;
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
