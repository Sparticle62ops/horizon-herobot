// HeroBot AI script JSON codec ported 1:1 from HerobaneNair/HeRoBot
// (herobot-common/.../ai/AiScriptCodec.java), CC-BY-NC-4.0.
// Field layout matches upstream Gson output exactly: version, name,
// optional sorted, variables, varFolders, functions, blocks, wires,
// comments. Style runs are RLE pairs (len, flag). Legacy block names
// migrate like upstream blockFromJson.

use crate::ai::{AiScript, BlockInstance, Comment, FuncDecl, ParamValue, VarDecl, VarType, Wire};
use crate::block_registry::{find_block_type, BlockType};
use serde_json::{json, Map, Value};

pub fn to_json(script: &AiScript, omit_positions: bool) -> String {
    let mut root = Map::new();
    root.insert("version".into(), json!(script.version));
    root.insert("name".into(), json!(script.name));
    if omit_positions {
        root.insert("sorted".into(), json!(true));
    }

    let vars: Vec<Value> = script
        .variables
        .iter()
        .map(|v: &VarDecl| {
            let mut o = Map::new();
            o.insert("name".into(), json!(v.name));
            o.insert("type".into(), json!(var_type_name(v.ty)));
            o.insert("default".into(), match &v.default_value {
                Some(d) => default_to_json(v.ty, d),
                None => Value::Null,
            });
            if !v.folder.is_empty() {
                o.insert("folder".into(), json!(v.folder));
            }
            Value::Object(o)
        })
        .collect();
    root.insert("variables".into(), Value::Array(vars));

    root.insert("varFolders".into(), json!(script.var_folders));

    let funcs: Vec<Value> = script
        .functions
        .iter()
        .map(|f: &FuncDecl| {
            let mut o = Map::new();
            o.insert("name".into(), json!(f.name));
            if !f.folder.is_empty() {
                o.insert("folder".into(), json!(f.folder));
            }
            o.insert(
                "params".into(),
                Value::Array(f.params.iter().map(|t| json!(var_type_name(*t))).collect()),
            );
            Value::Object(o)
        })
        .collect();
    root.insert("functions".into(), Value::Array(funcs));

    let blocks: Vec<Value> = script
        .blocks
        .iter()
        .map(|b| block_to_json(b, omit_positions))
        .collect();
    root.insert("blocks".into(), Value::Array(blocks));

    let wires: Vec<Value> = script
        .wires
        .iter()
        .map(|w: &Wire| {
            let mut o = Map::new();
            o.insert("from".into(), json!(w.from_block_id));
            o.insert("out".into(), json!(w.out_port));
            o.insert("to".into(), json!(w.to_block_id));
            if w.to_port != 0 {
                o.insert("toPort".into(), json!(w.to_port));
            }
            Value::Object(o)
        })
        .collect();
    root.insert("wires".into(), Value::Array(wires));

    let comments: Vec<Value> = script
        .comments
        .iter()
        .map(|c: &Comment| {
            let mut o = Map::new();
            o.insert("id".into(), json!(c.id));
            o.insert("x".into(), json!(c.x));
            o.insert("y".into(), json!(c.y));
            o.insert("text".into(), json!(c.text));
            if let Some(styles) = encode_styles(c) {
                o.insert("styles".into(), styles);
            }
            if c.attached_to >= 0 {
                o.insert("attachedTo".into(), json!(c.attached_to));
                o.insert("offX".into(), json!(c.off_x));
                o.insert("offY".into(), json!(c.off_y));
            }
            Value::Object(o)
        })
        .collect();
    root.insert("comments".into(), Value::Array(comments));

    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default()
}

fn var_type_name(t: VarType) -> &'static str {
    match t {
        VarType::Int => "INT",
        VarType::Double => "DOUBLE",
        VarType::Bool => "BOOL",
        VarType::Position => "POSITION",
        VarType::Rotation => "ROTATION",
        VarType::String => "STRING",
        VarType::Uuid => "UUID",
        VarType::Item => "ITEM",
    }
}

fn var_type_from_name(s: &str) -> Option<VarType> {
    Some(match s {
        "INT" => VarType::Int,
        "DOUBLE" => VarType::Double,
        "BOOL" => VarType::Bool,
        "POSITION" => VarType::Position,
        "ROTATION" => VarType::Rotation,
        "STRING" => VarType::String,
        "UUID" => VarType::Uuid,
        "ITEM" => VarType::Item,
        _ => return None,
    })
}

/// Upstream paramToJson: bool stays bool, numbers stay numbers,
/// everything else becomes its string.
fn default_to_json(t: VarType, d: &str) -> Value {
    match t {
        VarType::Bool => json!(d == "true"),
        VarType::Int => d.parse::<i64>().map(|v| json!(v)).unwrap_or_else(|_| json!(d)),
        VarType::Double => d.parse::<f64>().map(|v| json!(v)).unwrap_or_else(|_| json!(d)),
        _ => json!(d),
    }
}

fn encode_styles(c: &Comment) -> Option<Value> {
    if c.styles.iter().all(|s| *s == 0) {
        return None;
    }
    let mut arr = Vec::new();
    let mut i = 0usize;
    while i < c.styles.len() {
        let flag = c.styles[i];
        let mut j = i + 1;
        while j < c.styles.len() && c.styles[j] == flag {
            j += 1;
        }
        arr.push(json!((j - i) as i64));
        arr.push(json!(flag as i64));
        i = j;
    }
    Some(Value::Array(arr))
}

fn decode_styles(arr: &[Value], c: &mut Comment) {
    let mut styles = vec![0u8; c.text.len()];
    let mut pos = 0usize;
    let mut k = 0usize;
    while k + 1 < arr.len() {
        let len = arr[k].as_i64().unwrap_or(0) as usize;
        let flag = arr[k + 1].as_i64().unwrap_or(0) as u8;
        for _ in 0..len {
            if pos >= styles.len() {
                break;
            }
            styles[pos] = flag;
            pos += 1;
        }
        k += 2;
    }
    c.set_styles(styles);
}

fn block_to_json(b: &BlockInstance, omit_positions: bool) -> Value {
    let mut o = Map::new();
    o.insert("id".into(), json!(b.id));
    o.insert("type".into(), json!(BlockType::from_index(b.block_type).json_name()));
    if !omit_positions {
        o.insert("x".into(), json!(b.x));
        o.insert("y".into(), json!(b.y));
    }
    if b.paired_id >= 0 {
        o.insert("pairedId".into(), json!(b.paired_id));
    }
    let mut params = Map::new();
    for (k, v) in &b.params {
        params.insert(k.clone(), v.to_json());
    }
    o.insert("params".into(), Value::Object(params));
    if !b.reporter_params.is_empty() {
        let mut reps = Map::new();
        for (k, v) in &b.reporter_params {
            reps.insert(k.clone(), block_to_json(v, omit_positions));
        }
        o.insert("reporters".into(), Value::Object(reps));
    }
    Value::Object(o)
}

pub fn from_json(json_str: &str, fallback_name: &str) -> Result<AiScript, String> {
    let root: Value = serde_json::from_str(json_str).map_err(|e| format!("bad AI script json: {}", e))?;
    let name = root.get("name").and_then(Value::as_str).unwrap_or(fallback_name);
    let mut s = AiScript::new(name);
    s.version = root.get("version").and_then(Value::as_i64).unwrap_or(1) as i32;

    if let Some(vars) = root.get("variables").and_then(Value::as_array) {
        for vo in vars {
            let vname = vo.get("name").and_then(Value::as_str).unwrap_or("");
            let ty = var_type_from_name(vo.get("type").and_then(Value::as_str).unwrap_or(""))
                .unwrap_or(VarType::String);
            let def = vo.get("default").and_then(|d| json_to_var_default(d, ty));
            let folder = vo.get("folder").and_then(Value::as_str).unwrap_or("");
            s.variables.push(VarDecl {
                name: vname.to_string(),
                ty,
                default_value: def,
                folder: folder.to_string(),
            });
        }
    }

    if let Some(folders) = root.get("varFolders").and_then(Value::as_array) {
        for f in folders {
            if let Some(fs) = f.as_str() {
                s.var_folders.push(fs.to_string());
            }
        }
    }

    if let Some(funcs) = root.get("functions").and_then(Value::as_array) {
        for fo in funcs {
            let mut params = Vec::new();
            if let Some(ps) = fo.get("params").and_then(Value::as_array) {
                for pe in ps {
                    params.push(
                        var_type_from_name(pe.as_str().unwrap_or("")).unwrap_or(VarType::String),
                    );
                }
            }
            let ffolder = fo.get("folder").and_then(Value::as_str).unwrap_or("");
            s.functions.push(FuncDecl {
                name: fo.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                params,
                folder: ffolder.to_string(),
            });
        }
    }

    if let Some(blocks) = root.get("blocks").and_then(Value::as_array) {
        for be in blocks {
            let b = block_from_json(be).map_err(|e| e)?;
            s.put_block(b.clone());
            s.register_nested_ids(&b);
        }
    }

    if let Some(wires) = root.get("wires").and_then(Value::as_array) {
        for wo in wires {
            let (Some(from), Some(out), Some(to)) = (
                wo.get("from").and_then(Value::as_i64),
                wo.get("out").and_then(Value::as_i64),
                wo.get("to").and_then(Value::as_i64),
            ) else {
                continue; // upstream logs and skips a wire missing from/out/to
            };
            if s.block(from as i32).is_none() || s.block(to as i32).is_none() {
                continue; // upstream drops a wire to a missing block
            }
            let to_port = wo.get("toPort").and_then(Value::as_i64).unwrap_or(0);
            s.add_wire(Wire::with_to_port(from as i32, out as i32, to as i32, to_port as i32));
        }
    }

    let orphaned: Vec<i32> = s
        .blocks
        .iter()
        .filter(|b| b.paired_id >= 0 && s.block(b.paired_id).is_none())
        .map(|b| b.id)
        .collect();
    for bid in orphaned {
        if let Some(b) = s.blocks.iter_mut().find(|b| b.id == bid) {
            b.paired_id = -1;
        }
    }

    if let Some(comments) = root.get("comments").and_then(Value::as_array) {
        for co in comments {
            let id = co.get("id").and_then(Value::as_i64).map(|v| v as i32);
            let x = co.get("x").and_then(Value::as_f64).unwrap_or(0.0);
            let y = co.get("y").and_then(Value::as_f64).unwrap_or(0.0);
            let text = co.get("text").and_then(Value::as_str).unwrap_or("");
            let id = match id {
                Some(id) => id,
                None => s.fresh_id(),
            };
            let mut c = Comment::new(id, x, y, text);
            if let Some(styles) = co.get("styles").and_then(Value::as_array) {
                decode_styles(styles, &mut c);
            }
            if let Some(att) = co.get("attachedTo").and_then(Value::as_i64) {
                c.attached_to = att as i32;
                c.off_x = co.get("offX").and_then(Value::as_f64).unwrap_or(0.0);
                c.off_y = co.get("offY").and_then(Value::as_f64).unwrap_or(0.0);
            }
            s.comments.push(c);
            s.bump_next_id(id);
        }
    }
    Ok(s)
}

fn json_to_var_default(el: &Value, ty: VarType) -> Option<String> {
    if el.is_null() {
        return None;
    }
    Some(match ty {
        VarType::Bool => el.as_bool().map(|b| b.to_string()).unwrap_or_default(),
        VarType::Int => el.as_i64().map(|i| i.to_string()).unwrap_or_default(),
        VarType::Double => el.as_f64().map(|d| d.to_string()).unwrap_or_default(),
        _ => param_value_to_string(&ParamValue::from_json(el)),
    })
}

fn param_value_to_string(v: &ParamValue) -> String {
    match v {
        ParamValue::Bool(b) => b.to_string(),
        ParamValue::Int(i) => i.to_string(),
        ParamValue::Double(d) => d.to_string(),
        ParamValue::Str(s) => s.clone(),
    }
}

fn block_from_json(o: &Value) -> Result<BlockInstance, String> {
    let id = o.get("id").and_then(Value::as_i64).ok_or("block entry is missing id/type")? as i32;
    let type_name = o
        .get("type")
        .and_then(Value::as_str)
        .ok_or("block entry is missing id/type")?;
    let x = o.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let y = o.get("y").and_then(Value::as_f64).unwrap_or(0.0);

    // Legacy migrations, upstream blockFromJson verbatim.
    if let Some(op) = legacy_arith_op(type_name) {
        let mut calc = BlockInstance::new(id, BlockType::NumCalc.index(), x, y);
        calc.set_param("expression", ParamValue::Str(legacy_expr(o, op)));
        return Ok(calc);
    }
    if type_name == "TRADE_EXECUTE" {
        let mut click = BlockInstance::new(id, BlockType::InvClick.index(), x, y);
        click.set_param("menu", ParamValue::Str("container".into()));
        click.set_param("mode", ParamValue::Str(legacy_trade_click_mode(o).into()));
        click.set_param("slot", ParamValue::Int(2));
        return Ok(click);
    }

    let legacy_pitch = type_name == "PITCH";
    let mut type_name = if legacy_pitch { "YAW" } else { type_name };
    let legacy_offhand = type_name == "INV_SWAP_OFFHAND";
    if legacy_offhand {
        type_name = "INV_SWAP_HOTBAR";
    }
    if type_name == "LOOK_YAW_PITCH" {
        type_name = "LOOK_DIRECTION";
    } else if type_name == "LOOK_DIRECTION" && is_legacy_cardinal(o) {
        type_name = "LOOK_CARDINAL";
    }
    if type_name == "OPEN_CONTAINER" {
        type_name = "OPEN_INVENTORY";
    }
    if type_name == "CONTAINER_OPEN" {
        type_name = "INVENTORY_OPEN";
    }

    let legacy_drop_stack = type_name == "DROP_STACK";
    let block_type = find_block_type(if legacy_drop_stack { "DROP_ITEM" } else { type_name })
        .ok_or_else(|| format!("unknown block type {}", type_name))?;
    let mut b = BlockInstance::new(id, block_type.index(), x, y);
    if legacy_drop_stack {
        b.set_param("amount", ParamValue::Str("stack".into()));
    }
    if legacy_pitch {
        b.set_param("axis", ParamValue::Str("pitch".into()));
    }
    if let Some(paired) = o.get("pairedId").and_then(Value::as_i64) {
        b.paired_id = paired as i32;
    }
    if let Some(params) = o.get("params").and_then(Value::as_object) {
        for (k, v) in params {
            b.set_param(k.clone(), ParamValue::from_json(v));
        }
    }
    if block_type == BlockType::LookDirection
        && b.get_param("direction").is_none()
        && (b.get_param("yaw").is_some() || b.get_param("pitch").is_some())
    {
        let yaw = b.get_param("yaw").map(num_of).unwrap_or_else(|| "0".into());
        let pitch = b.get_param("pitch").map(num_of).unwrap_or_else(|| "0".into());
        b.set_param("direction", ParamValue::Str(format!("{} {}", yaw, pitch)));
    }
    if block_type == BlockType::InvSwapHotbar {
        if legacy_offhand {
            b.set_param("with", ParamValue::Str("offhand".into()));
        } else if b.get_param("with").is_none() && b.get_param("hotbarSlot").is_some() {
            let hs = b.get_param("hotbarSlot").unwrap().clone();
            b.set_param("with", ParamValue::Str(param_value_to_string(&hs)));
        }
    }
    if block_type == BlockType::RecipeBook
        && b.get_param("mode").is_none()
        && b.get_param("max").is_some()
    {
        let is_max = matches!(b.get_param("max"), Some(ParamValue::Bool(true)))
            || param_value_to_string(b.get_param("max").unwrap()) == "true";
        b.set_param("mode", ParamValue::Str(if is_max { "max".into() } else { "single".into() }));
    }
    if block_type == BlockType::Contains
        && b.get_param("checkCase").is_none()
        && b.get_param("ci").is_some()
    {
        let insensitive = matches!(b.get_param("ci"), Some(ParamValue::Bool(true)))
            || param_value_to_string(b.get_param("ci").unwrap()) == "true";
        b.set_param("checkCase", ParamValue::Bool(!insensitive));
        b.params.remove("ci");
    }
    if let Some(reps) = o.get("reporters").and_then(Value::as_object) {
        for (k, v) in reps {
            b.set_reporter(k.clone(), block_from_json(v)?);
        }
    }
    Ok(b)
}

/// Upstream legacyArithOp.
fn legacy_arith_op(type_name: &str) -> Option<&'static str> {
    Some(match type_name {
        "ADD" => "+",
        "SUB" => "-",
        "MUL" => "*",
        "DIV" => "/",
        "MOD" => "%",
        _ => return None,
    })
}

fn legacy_expr(o: &Value, op: &str) -> String {
    let get = |k: &str| {
        o.get("params")
            .and_then(|p| p.get(k))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    format!("{} {} {}", num_of(&ParamValue::Double(get("a"))), op, num_of(&ParamValue::Double(get("b"))))
}

/// Upstream num(): whole doubles print without a decimal point.
fn num_of(v: &ParamValue) -> String {
    match v {
        ParamValue::Double(d) if *d == d.floor() && d.is_finite() => format!("{}", *d as i64),
        other => param_value_to_string(other),
    }
}

fn is_legacy_cardinal(o: &Value) -> bool {
    matches!(
        o.get("params").and_then(|p| p.get("direction")).and_then(Value::as_str),
        Some("north" | "south" | "east" | "west" | "up" | "down")
    )
}

fn legacy_trade_click_mode(o: &Value) -> &'static str {
    let mode = o
        .get("params")
        .and_then(|p| p.get("mode"))
        .and_then(Value::as_str)
        .unwrap_or("once");
    match mode {
        "shiftClick" => "shiftClick",
        "drop" => "throw",
        "dropMax" => "throwAll",
        _ => "click",
    }
}

/// Upstream wasSorted.
pub fn was_sorted(json_str: &str) -> bool {
    serde_json::from_str::<Value>(json_str)
        .ok()
        .and_then(|r| r.get("sorted").and_then(Value::as_bool))
        .unwrap_or(false)
}

/// Upstream minCorner: [minX, minY] over blocks, None when empty.
pub fn min_corner(json_str: &str) -> Option<[f64; 2]> {
    let root: Value = serde_json::from_str(json_str).ok()?;
    let mut any = false;
    let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
    for e in root.get("blocks")?.as_array()? {
        let x = e.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let y = e.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        any = true;
    }
    if any {
        Some([min_x, min_y])
    } else {
        None
    }
}
