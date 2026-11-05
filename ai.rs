// HeroBot AI script model ported 1:1 from HerobaneNair/HeRoBot
// (herobot-common/.../ai: AiScript, VarType, VarDecl, FuncDecl, Comment),
// CC-BY-NC-4.0. Java records/annotations become plain structs; the
// JSON codec (AiScriptCodec.java, 547 lines) lands next.

pub const CURRENT_VERSION: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VarType { Int, Double, Bool, Position, Rotation, String, Uuid, Item }

impl VarType {
    pub fn display_name(self) -> &'static str {
        match self {
            VarType::Bool => "boolean", VarType::Int => "int",
            VarType::Double => "decimal", VarType::Position => "position",
            VarType::Rotation => "direction", VarType::String => "String",
            VarType::Uuid => "UUID", VarType::Item => "Item",
        }
    }
    pub fn chip_label(self) -> &'static str {
        match self {
            VarType::Bool => "bool", VarType::Int => "int",
            VarType::Double => "deci", VarType::Position => "Pos",
            VarType::Rotation => "Dir", VarType::String => "String",
            VarType::Uuid => "UUID", VarType::Item => "Item",
        }
    }
}

/// A script variable slot. Java's  becomes Value;
/// the folder prefix matches upstream "folder/name" qualified names.
#[derive(Clone, Debug)]
pub struct VarDecl {
    pub name: String,
    pub ty: VarType,
    pub default_value: Option<String>,
    pub folder: String,
}

impl VarDecl {
    pub fn qualified_name(&self) -> String {
        if self.folder.is_empty() { self.name.clone() } else { format!("{}/{}", self.folder, self.name) }
    }
}

#[derive(Clone, Debug)]
pub struct FuncDecl {
    pub name: String,
    pub params: Vec<VarType>,
    pub folder: String,
}

impl FuncDecl {
    pub fn qualified_name(&self) -> String {
        if self.folder.is_empty() { self.name.clone() } else { format!("{}/{}", self.folder, self.name) }
    }
    pub fn num_params(&self) -> usize { self.params.len() }
    pub fn param_type(&self, index: usize) -> Option<VarType> { self.params.get(index).copied() }
    pub fn with_param_added(mut self, ty: VarType) -> Self { self.params.push(ty); self }
    pub fn with_param_removed(mut self, index: usize) -> Self {
        if index < self.params.len() { self.params.remove(index); }
        self
    }
    pub fn with_param_type(mut self, index: usize, ty: VarType) -> Self {
        if let Some(p) = self.params.get_mut(index) { *p = ty; }
        self
    }
}

/// Comment style flags, same bit values as upstream.
pub const STYLE_BOLD: u8 = 1;
pub const STYLE_ITALIC: u8 = 2;
pub const STYLE_UNDERLINE: u8 = 4;
pub const STYLE_STRIKE: u8 = 8;

#[derive(Clone, Debug)]
pub struct Comment {
    pub id: i32,
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub styles: Vec<u8>,
    pub attached_to: i32,
    pub off_x: f64,
    pub off_y: f64,
}

impl Comment {
    pub fn new(id: i32, x: f64, y: f64, text: &str) -> Self {
        let text = text.to_string();
        let styles = vec![0u8; text.len()];
        Self { id, x, y, text, styles, attached_to: -1, off_x: 0.0, off_y: 0.0 }
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.styles = vec![0u8; self.text.len()];
    }

    pub fn style_at(&self, i: usize) -> u8 { self.styles.get(i).copied().unwrap_or(0) }

    pub fn set_styles(&mut self, s: Vec<u8>) {
        if s.len() == self.text.len() { self.styles = s; }
    }

    pub fn insert(&mut self, idx: usize, s: &str, style: u8) {
        if s.is_empty() { return; }
        let idx = idx.min(self.text.len());
        self.text.insert_str(idx, s);
        let mut ns = Vec::with_capacity(self.styles.len() + s.len());
        ns.extend_from_slice(&self.styles[..idx]);
        ns.extend(std::iter::repeat(style).take(s.len()));
        ns.extend_from_slice(&self.styles[idx..]);
        self.styles = ns;
    }

    pub fn delete(&mut self, from: usize, to: usize) {
        let from = from.min(self.text.len());
        let to = to.max(from).min(self.text.len());
        if from == to { return; }
        self.text.replace_range(from..to, "");
        self.styles.drain(from..to);
    }

    pub fn toggle_style(&mut self, from: usize, to: usize, flag: u8) {
        let from = from.min(self.styles.len());
        let to = to.max(from).min(self.styles.len());
        if from == to { return; }
        let all = self.styles[from..to].iter().all(|s| s & flag != 0);
        for s in &mut self.styles[from..to] {
            *s = if all { *s & !flag } else { *s | flag };
        }
    }

    pub fn range_has_style(&self, from: usize, to: usize, flag: u8) -> bool {
        let from = from.min(self.styles.len());
        let to = to.max(from).min(self.styles.len());
        if from == to { return false; }
        self.styles[from..to].iter().all(|s| s & flag != 0)
    }
}

/// Wire between two blocks: from block/port to block (and optional port).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wire {
    pub from_block_id: i32,
    pub out_port: i32,
    pub to_block_id: i32,
    pub to_port: i32,
}

impl Wire {
    pub fn new(from_block_id: i32, out_port: i32, to_block_id: i32) -> Self {
        Self { from_block_id, out_port, to_block_id, to_port: -1 }
    }
    pub fn with_to_port(from_block_id: i32, out_port: i32, to_block_id: i32, to_port: i32) -> Self {
        Self { from_block_id, out_port, to_block_id, to_port }
    }
}

/// A parameter value: what upstream keeps as  in
/// BlockInstance.params. jsonToParam's shape: bool, int, double, string.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    Bool(bool),
    Int(i64),
    Double(f64),
    Str(String),
}

impl ParamValue {
    /// Upstream jsonToParam fallback: numbers that are whole become int.
    pub fn from_json(v: &serde_json::Value) -> ParamValue {
        match v {
            serde_json::Value::Bool(b) => ParamValue::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ParamValue::Int(i)
                } else if let Some(d) = n.as_f64() {
                    if d == d.floor() && d.is_finite() {
                        ParamValue::Int(d as i64)
                    } else {
                        ParamValue::Double(d)
                    }
                } else {
                    ParamValue::Str(n.to_string())
                }
            }
            serde_json::Value::Null => ParamValue::Str(String::new()),
            other => ParamValue::Str(match other {
                serde_json::Value::String(s) => s.clone(),
                _ => other.to_string(),
            }),
        }
    }
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            ParamValue::Bool(b) => serde_json::Value::Bool(*b),
            ParamValue::Int(i) => serde_json::json!(i),
            ParamValue::Double(d) => serde_json::json!(d),
            ParamValue::Str(s) => serde_json::json!(s),
        }
    }
}

/// A placed block in the editor graph. block_type is the BlockType
/// registry index; params/reporter_params/paired_id match upstream
/// BlockInstance.java (pairedId >= 0 links paired C-shape blocks).
#[derive(Clone, Debug)]
pub struct BlockInstance {
    pub id: i32,
    pub block_type: i32,
    pub x: f64,
    pub y: f64,
    pub paired_id: i32,
    pub params: std::collections::BTreeMap<String, ParamValue>,
    pub reporter_params: std::collections::BTreeMap<String, BlockInstance>,
}

impl BlockInstance {
    pub fn new(id: i32, block_type: i32, x: f64, y: f64) -> Self {
        Self {
            id,
            block_type,
            x,
            y,
            paired_id: -1,
            params: Default::default(),
            reporter_params: Default::default(),
        }
    }
    pub fn get_param(&self, key: &str) -> Option<&ParamValue> { self.params.get(key) }
    pub fn set_param(&mut self, key: impl Into<String>, value: ParamValue) {
        self.params.insert(key.into(), value);
    }
    pub fn set_reporter(&mut self, key: impl Into<String>, b: BlockInstance) {
        self.reporter_params.insert(key.into(), b);
    }
    /// clone with fresh ids (upstream cloneWithNewIds); id map filled by
    /// the clipboard module.
    pub fn clone_at(&self, id: i32, dx: f64, dy: f64) -> Self {
        let mut c = self.clone();
        c.id = id;
        c.x += dx;
        c.y += dy;
        c
    }
}

/// The script graph itself: variables, functions, blocks, wires, comments,
/// and the nextId allocator, 1:1 with AiScript.java.
#[derive(Clone, Debug)]
pub struct AiScript {
    pub name: String,
    pub version: i32,
    pub variables: Vec<VarDecl>,
    pub var_folders: Vec<String>,
    pub functions: Vec<FuncDecl>,
    pub blocks: Vec<BlockInstance>,
    pub wires: Vec<Wire>,
    pub comments: Vec<Comment>,
    next_id: i32,
}

impl AiScript {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: CURRENT_VERSION,
            variables: Vec::new(),
            var_folders: Vec::new(),
            functions: Vec::new(),
            blocks: Vec::new(),
            wires: Vec::new(),
            comments: Vec::new(),
            next_id: 1,
        }
    }

    pub fn function(&self, qualified_name: &str) -> Option<&FuncDecl> {
        self.functions.iter().find(|f| f.qualified_name() == qualified_name)
    }

    pub fn function_index(&self, qualified_name: &str) -> Option<usize> {
        self.functions.iter().position(|f| f.qualified_name() == qualified_name)
    }

    pub fn add_comment(&mut self, x: f64, y: f64, text: &str) -> &Comment {
        let id = self.next_id;
        self.next_id += 1;
        self.comments.push(Comment::new(id, x, y, text));
        self.comments.last().unwrap()
    }

    pub fn remove_comment(&mut self, id: i32) {
        self.comments.retain(|c| c.id != id);
    }

    pub fn comment(&self, id: i32) -> Option<&Comment> {
        self.comments.iter().find(|c| c.id == id)
    }

    pub fn add_block(&mut self, block_type: i32, x: f64, y: f64) -> BlockInstance {
        let b = BlockInstance::new(self.next_id, block_type, x, y);
        self.next_id += 1;
        self.blocks.push(b.clone());
        b
    }

    /// Upstream putBlock: insert/replace and bump the id allocator.
    pub fn put_block(&mut self, b: BlockInstance) {
        self.bump_next_id(b.id);
        if let Some(e) = self.blocks.iter_mut().find(|e| e.id == b.id) {
            *e = b;
        } else {
            self.blocks.push(b);
        }
    }

    /// Upstream registerNestedIds: reporter children bump nextId too.
    pub fn register_nested_ids(&mut self, b: &BlockInstance) {
        self.bump_next_id(b.id);
        for child in b.reporter_params.values() {
            self.register_nested_ids(child);
        }
    }

    pub fn fresh_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn bump_next_id(&mut self, id: i32) {
        if id >= self.next_id { self.next_id = id + 1; }
    }

    pub fn remove_block(&mut self, id: i32) {
        self.blocks.retain(|b| b.id != id);
        self.wires.retain(|w| w.from_block_id != id && w.to_block_id != id);
    }

    pub fn add_wire(&mut self, w: Wire) {
        self.wires.push(w);
    }

    pub fn outgoing(&self, block_id: i32, out_port: i32) -> Vec<&Wire> {
        self.wires.iter()
            .filter(|w| w.from_block_id == block_id && w.out_port == out_port)
            .collect()
    }

    /// Hat blocks of one type, sorted by x then y like upstream
    /// hatBlocks. block_type here is the BlockDef registry index.
    pub fn hat_blocks(&self, block_type: i32) -> Vec<&BlockInstance> {
        let mut r: Vec<&BlockInstance> = self.blocks.iter()
            .filter(|b| b.block_type == block_type).collect();
        r.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)));
        r
    }

    pub fn block(&self, id: i32) -> Option<&BlockInstance> {
        self.blocks.iter().find(|b| b.id == id)
    }
}
