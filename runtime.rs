// HeroBot AI runtime core ported 1:1 from HerobaneNair/HeRoBot
// (herobot-common/.../ai/runtime/{Branch,ControlFrame,JoinState,
// RuntimeVariable,StepResult}.java + herobot-mod/.../ai/runtime/
// {ScriptRunner,ParamEval,ScriptDispatch}.java), CC-BY-NC-4.0.
// Game-bound pieces (ServerPlayer/BotPlayer, PlayerControllers,
// PathExecutor, DataExecutor.asItemStack) sit behind the BotHook
// trait; the real hook is the Horizon fake-player work (issue drafted
// in README). The per-category executors port over coming runs; the
// dispatch table below carries the game-independent control blocks
// until then, exactly like upstream when an executor is missing.

use crate::ai::{AiScript, BlockInstance, ParamValue, VarType, Wire};
use crate::block_registry::{def_for, BlockCategory, BlockType};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// The dynamic value flowing through params, variables and reporters.
/// Upstream keeps ; bool/number/string mirror ParamValue, and
/// position/rotation/item arrive from the executors.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeValue {
    Bool(bool),
    Int(i64),
    Double(f64),
    Str(String),
    Vec3(f64, f64, f64),
    Rot(f32, f32),
}

impl RuntimeValue {
    /// Upstream ParamEval.asBool.
    pub fn as_bool(&self) -> bool {
        match self {
            RuntimeValue::Bool(b) => *b,
            RuntimeValue::Int(i) => *i >= 1,
            RuntimeValue::Double(d) => *d >= 1.0,
            RuntimeValue::Str(s) => {
                let t = s.trim();
                match t.parse::<f64>() {
                    Ok(d) => d >= 1.0,
                    Err(_) => t.eq_ignore_ascii_case("true"),
                }
            }
            _ => true,
        }
    }
    /// Upstream ParamEval.asInt.
    pub fn as_int(&self) -> i64 {
        match self {
            RuntimeValue::Bool(b) => *b as i64,
            RuntimeValue::Int(i) => *i,
            RuntimeValue::Double(d) => *d as i64,
            RuntimeValue::Str(s) => s
                .trim()
                .parse::<i64>()
                .or_else(|_| s.trim().parse::<f64>().map(|d| d as i64))
                .unwrap_or(0),
            _ => 0,
        }
    }
    /// Upstream ParamEval.asDouble.
    pub fn as_double(&self) -> f64 {
        match self {
            RuntimeValue::Bool(b) => *b as i64 as f64,
            RuntimeValue::Int(i) => *i as f64,
            RuntimeValue::Double(d) => *d,
            RuntimeValue::Str(s) => s.trim().parse::<f64>().unwrap_or(0.0),
            _ => 0.0,
        }
    }
    /// Upstream ParamEval.asString.
    pub fn as_string(&self) -> String {
        match self {
            RuntimeValue::Str(s) => s.clone(),
            RuntimeValue::Bool(b) => b.to_string(),
            RuntimeValue::Int(i) => i.to_string(),
            RuntimeValue::Double(d) => d.to_string(),
            RuntimeValue::Vec3(x, y, z) => format!("{} {} {}", x, y, z),
            RuntimeValue::Rot(yaw, pitch) => format!("{} {}", yaw, pitch),
        }
    }
}

impl From<&ParamValue> for RuntimeValue {
    fn from(v: &ParamValue) -> Self {
        match v {
            ParamValue::Bool(b) => RuntimeValue::Bool(*b),
            ParamValue::Int(i) => RuntimeValue::Int(*i),
            ParamValue::Double(d) => RuntimeValue::Double(*d),
            ParamValue::Str(s) => RuntimeValue::Str(s.clone()),
        }
    }
}

/// Upstream StepResult.
#[derive(Clone, Debug, PartialEq)]
pub enum StepKind {
    Continue,
    Wait,
    Jump,
    End,
    Handled,
}

#[derive(Clone, Debug)]
pub struct StepResult {
    pub kind: StepKind,
    pub port: i32,
    pub wait_ticks: i32,
    pub jump_to: i32,
    pub push_frame: bool,
    pub pop_frame: bool,
    pub activation_id: i32,
    pub frame_data: Option<BTreeMap<String, RuntimeValue>>,
}

impl StepResult {
    pub fn continue_via(port: i32) -> Self {
        Self { kind: StepKind::Continue, port, wait_ticks: 0, jump_to: 0, push_frame: false, pop_frame: false, activation_id: -1, frame_data: None }
    }
    pub fn enter_body(port: i32, activation_id: i32) -> Self {
        Self { kind: StepKind::Continue, port, wait_ticks: 0, jump_to: 0, push_frame: true, pop_frame: false, activation_id, frame_data: None }
    }
    pub fn exit_body(port: i32) -> Self {
        Self { kind: StepKind::Continue, port, wait_ticks: 0, jump_to: 0, push_frame: false, pop_frame: true, activation_id: -1, frame_data: None }
    }
    pub fn push_and_jump(block_id: i32, activation_id: i32) -> Self {
        Self { kind: StepKind::Jump, port: 0, wait_ticks: 0, jump_to: block_id, push_frame: true, pop_frame: false, activation_id, frame_data: None }
    }
    pub fn push_and_jump_with(block_id: i32, activation_id: i32, data: BTreeMap<String, RuntimeValue>) -> Self {
        Self { kind: StepKind::Jump, port: 0, wait_ticks: 0, jump_to: block_id, push_frame: true, pop_frame: false, activation_id, frame_data: Some(data) }
    }
    pub fn pop_and_jump(block_id: i32) -> Self {
        Self { kind: StepKind::Jump, port: 0, wait_ticks: 0, jump_to: block_id, push_frame: false, pop_frame: true, activation_id: -1, frame_data: None }
    }
    pub fn wait(ticks: i32) -> Self {
        Self { kind: StepKind::Wait, port: 0, wait_ticks: ticks.max(0), jump_to: 0, push_frame: false, pop_frame: false, activation_id: -1, frame_data: None }
    }
    pub fn wait_then_continue(ticks: i32, port: i32) -> Self {
        Self { kind: StepKind::Continue, port, wait_ticks: ticks.max(0), jump_to: 0, push_frame: false, pop_frame: false, activation_id: -1, frame_data: None }
    }
    pub fn jump_to(block_id: i32) -> Self {
        Self { kind: StepKind::Jump, port: 0, wait_ticks: 0, jump_to: block_id, push_frame: false, pop_frame: false, activation_id: -1, frame_data: None }
    }
    pub fn end() -> Self {
        Self { kind: StepKind::End, port: 0, wait_ticks: 0, jump_to: 0, push_frame: false, pop_frame: false, activation_id: -1, frame_data: None }
    }
    pub fn handled() -> Self {
        Self { kind: StepKind::Handled, port: 0, wait_ticks: 0, jump_to: 0, push_frame: false, pop_frame: false, activation_id: -1, frame_data: None }
    }
}

/// Upstream ControlFrame.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlFrame {
    pub container_block_id: i32,
    pub activation_id: i32,
    pub data: BTreeMap<String, RuntimeValue>,
}

impl ControlFrame {
    pub fn new(container_block_id: i32, activation_id: i32) -> Self {
        Self { container_block_id, activation_id, data: BTreeMap::new() }
    }
    pub fn int_data(&self, key: &str, default: i64) -> i64 {
        self.data.get(key).map(RuntimeValue::as_int).unwrap_or(default)
    }
    pub fn set_data(&mut self, key: impl Into<String>, value: RuntimeValue) {
        self.data.insert(key.into(), value);
    }
}

/// Upstream JoinState.
#[derive(Clone, Debug)]
pub struct JoinState {
    pub start_type: BlockType,
    pub start_block_id: i32,
    pub end_block_id: i32,
    pub remaining: i32,
    pub iteration: i32,
}

/// Upstream RuntimeVariable.
#[derive(Clone, Debug)]
pub struct RuntimeVariable {
    pub ty: VarType,
    pub value: RuntimeValue,
}

/// Upstream Branch.
#[derive(Clone, Debug)]
pub struct Branch {
    pub current_block_id: i32,
    pub wait_ticks: i32,
    pub dead: bool,
    pub parked: bool,
    pub turns: i32,
    pub calls_this_tick: i32,
    pub origin_id: i32,
    pub path_wait_for: i32,
    pub event_message: Option<String>,
    pub frames: Vec<ControlFrame>,
}

impl Branch {
    pub fn new(start_block_id: i32) -> Self {
        Self { current_block_id: start_block_id, wait_ticks: 0, dead: false, parked: false, turns: 0, calls_this_tick: 0, origin_id: -1, path_wait_for: -1, event_message: None, frames: Vec::new() }
    }
    pub fn decrement_wait(&mut self) -> bool {
        if self.wait_ticks <= 0 {
            return false;
        }
        self.wait_ticks -= 1;
        true
    }
    pub fn in_activation(&self, activation_id: i32) -> bool {
        self.frames.iter().any(|f| f.activation_id == activation_id)
    }
    /// Upstream Branch.forkAt.
    pub fn fork_at(&self, block_id: i32) -> Branch {
        Branch {
            current_block_id: block_id,
            wait_ticks: 0,
            dead: false,
            parked: false,
            turns: self.turns,
            calls_this_tick: self.calls_this_tick,
            origin_id: self.origin_id,
            path_wait_for: self.path_wait_for,
            event_message: self.event_message.clone(),
            frames: self.frames.clone(),
        }
    }
}

/// The game hooks the runtime and executors call. Implemented by the
/// bot framework once the Horizon fake-player hook lands (see README).
pub trait BotHook {
    /// Position of the acting player (upstream player().position()).
    fn position(&self) -> (f64, f64, f64) {
        (0.0, 0.0, 0.0)
    }
}

/// Reporter evaluation hook (upstream ScriptRunner.evalReporter).
pub trait ReporterEval {
    fn eval_reporter(&self, reporter: &BlockInstance, branch: &Branch) -> Option<RuntimeValue>;
}

/// Upstream ParamEval.
pub struct ParamEval;

impl ParamEval {
    /// raw(): reporter wins over the literal param, then the slot
    /// default. Slot defaults are stored as strings in the ported
    /// registry; coercions below match upstream behavior on them.
    pub fn raw<H: ReporterEval + ?Sized>(
        block: &BlockInstance,
        name: &str,
        host: &H,
        branch: &Branch,
    ) -> RuntimeValue {
        let slot = def_for(BlockType::from_index(block.block_type))
            .and_then(|d| d.param_slot(name));
        let ty = BlockType::from_index(block.block_type);
        let reporter = block.reporter_params.get(name);
        let v = if let Some(rep) = reporter {
            host.eval_reporter(rep, branch).unwrap_or(RuntimeValue::Str(String::new()))
        } else if let Some(p) = block.params.get(name) {
            RuntimeValue::from(p)
        } else if let Some(s) = slot {
            RuntimeValue::Str(s.default_value.clone().unwrap_or_default())
        } else {
            RuntimeValue::Str(String::new())
        };
        Self::coerce_enum(&ty, name, v)
    }

    /// Upstream coerceEnum: numeric index into a non-numeric choice list
    /// resolves to that choice.
    fn coerce_enum(ty: &BlockType, name: &str, v: RuntimeValue) -> RuntimeValue {
        let slot = match def_for(*ty).and_then(|d| d.param_slot(name)) {
            Some(s) if s.ty == crate::block_registry::ParamType::Enum => s,
            _ => return v,
        };
        let choices = &slot.enum_choices;
        if choices.is_empty() || choices.iter().all(|c| c.trim().parse::<i64>().is_ok()) {
            return v;
        }
        if let RuntimeValue::Str(s) = &v {
            if choices.contains(&s.trim()) {
                return v;
            }
        }
        let idx = v.as_int();
        if idx < 0 || idx >= choices.len() as i64 {
            return v;
        }
        RuntimeValue::Str(choices[idx as usize].to_string())
    }

    pub fn eval_bool<H: ReporterEval + ?Sized>(b: &BlockInstance, name: &str, h: &H, br: &Branch) -> bool {
        Self::raw(b, name, h, br).as_bool()
    }
    pub fn eval_int<H: ReporterEval + ?Sized>(b: &BlockInstance, name: &str, h: &H, br: &Branch) -> i64 {
        Self::raw(b, name, h, br).as_int()
    }
    pub fn eval_double<H: ReporterEval + ?Sized>(b: &BlockInstance, name: &str, h: &H, br: &Branch) -> f64 {
        Self::raw(b, name, h, br).as_double()
    }
    pub fn eval_string<H: ReporterEval + ?Sized>(b: &BlockInstance, name: &str, h: &H, br: &Branch) -> String {
        Self::raw(b, name, h, br).as_string()
    }

    /// Upstream ParamEval.parseVec3 with ~-relative tokens.
    pub fn parse_vec3(s: &str, base: (f64, f64, f64)) -> (f64, f64, f64) {
        let parts: Vec<&str> = s.trim().split_whitespace().collect();
        if parts.len() < 3 {
            return base;
        }
        let rel = |token: &str, b: f64| -> f64 {
            match token.strip_prefix('~') {
                Some(rest) if rest.is_empty() => b,
                Some(rest) => rest.parse::<f64>().map(|d| b + d).unwrap_or(b),
                None => token.parse::<f64>().unwrap_or(b),
            }
        };
        (rel(parts[0], base.0), rel(parts[1], base.1), rel(parts[2], base.2))
    }

    pub fn as_rotation(v: &RuntimeValue) -> (f32, f32) {
        if let RuntimeValue::Rot(y, p) = v {
            return (*y, *p);
        }
        if let RuntimeValue::Str(s) = v {
            let parts: Vec<&str> = s.trim().split_whitespace().collect();
            if parts.len() >= 2 {
                if let (Ok(y), Ok(p)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                    return (y, p);
                }
            }
        }
        (0.0, 0.0)
    }
}

/// Flow executor signature (upstream Executor); reporters use the
/// ReporterEval hook. Registered by the per-category executor ports.
pub type FlowExec = fn(&mut ScriptRunner, &BlockInstance, &mut Branch) -> StepResult;

fn flow_dispatch(ty: BlockType) -> Option<FlowExec> {
    // Partial until the executor files land; upstream logs and ends the
    // branch on a missing executor, and so does this.
    match ty {
        BlockType::Wait => Some(exec_wait),
        BlockType::WaitUntil => Some(exec_wait_until),
        BlockType::StopScript => Some(exec_stop_script),
        BlockType::StopAll => Some(exec_end),
        BlockType::PauseScript => Some(exec_pause_script),
        BlockType::SetScript => Some(exec_set_script),
        BlockType::SetVar => Some(exec_set_var),
        BlockType::ChangeVar => Some(exec_change_var),
        BlockType::Break => Some(exec_break),
        BlockType::If => Some(exec_if),
        BlockType::ElseIf => Some(exec_else_if),
        BlockType::For => Some(exec_for),
        BlockType::While => Some(exec_while),
        BlockType::BlockEnd => Some(exec_block_end),
        _ => None,
    }
}

/// Upstream ControlExecutor WAIT: waitThenContinue(max(1, ticks), 0).
fn exec_wait(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    let ticks = ParamEval::eval_int(b, "ticks", r, br) as i32;
    StepResult::wait_then_continue(ticks.max(1), 0)
}

fn exec_end(_: &mut ScriptRunner, _: &BlockInstance, _: &mut Branch) -> StepResult {
    StepResult::end()
}

// ===== Upstream ControlExecutor (executors/ControlExecutor.java) =====

/// Upstream LOOP_ITER reporter: iteration count of the active loop whose
/// container block is this reporter's paired block.
pub fn eval_control_reporter(b: &BlockInstance, r: &ScriptRunner, br: &Branch) -> Option<RuntimeValue> {
    if BlockType::from_index(b.block_type) != BlockType::LoopIter {
        return None;
    }
    for f in &br.frames {
        if f.container_block_id != b.paired_id {
            continue;
        }
        if let Some(js) = r.join_state(f.activation_id) {
            return Some(RuntimeValue::Int(js.iteration as i64));
        }
    }
    Some(RuntimeValue::Int(0))
}

/// Upstream FunctionExecutor.isCallFrame.
pub fn is_call_frame(f: &ControlFrame) -> bool {
    f.data.contains_key("fnCall")
}

fn skip_to_end(start: &BlockInstance, activation_id: i32) -> StepResult {
    if start.paired_id < 0 {
        return StepResult::end();
    }
    StepResult::push_and_jump(start.paired_id, activation_id)
}

/// Upstream ControlExecutor.sideTarget: the ELSE_IF chained off port 1.
fn side_target(r: &ScriptRunner, block_id: i32) -> Option<BlockInstance> {
    for w in r.outgoing(block_id, 1) {
        if let Some(t) = r.script.block(w.to_block_id) {
            if BlockType::from_index(t.block_type) == BlockType::ElseIf {
                return Some(t.clone());
            }
        }
    }
    None
}

/// Upstream ControlExecutor.chainRoot: walk the ELSE_IF side chain back
/// to its IF over port-1 incoming wires.
fn chain_root(r: &ScriptRunner, elif_id: i32) -> Option<BlockInstance> {
    let mut visited = std::collections::HashSet::new();
    let mut current = elif_id;
    while visited.insert(current) {
        let mut source: Option<BlockInstance> = None;
        for w in r.script.wires.iter().filter(|w| w.to_block_id == current) {
            if w.out_port != 1 {
                continue;
            }
            source = r.script.block(w.from_block_id).cloned();
            break;
        }
        let source = match source {
            Some(s) => s,
            None => return None,
        };
        match BlockType::from_index(source.block_type) {
            BlockType::If => return Some(source),
            BlockType::ElseIf => current = source.id,
            _ => return None,
        }
    }
    None
}

/// Upstream ControlExecutor WAIT_UNTIL.
fn exec_wait_until(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    if ParamEval::eval_bool(b, "condition", r, br) {
        return StepResult::continue_via(0);
    }
    StepResult::wait(1)
}

/// Upstream ControlExecutor BREAK: unwind to the nth enclosing loop,
/// killing every activation it passes, then jump past the loop end.
fn exec_break(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    let want = ParamEval::eval_int(b, "count", r, br).max(1) as usize;
    let mut target: Option<ControlFrame> = None;
    let mut end_id = -1;
    let mut found = 0;
    for f in br.frames.iter() {
        if is_call_frame(f) {
            break;
        }
        if let Some(js) = r.join_state(f.activation_id) {
            target = Some(f.clone());
            end_id = js.end_block_id;
            found += 1;
            if found >= want {
                break;
            }
        }
    }
    let target = match target {
        Some(t) => t,
        None => {
            return if r.return_from_function(br) {
                StepResult::handled()
            } else {
                StepResult::end()
            };
        }
    };
    while !br.frames.is_empty() {
        let f = br.frames.pop().expect("frames nonempty");
        if r.join_state(f.activation_id).is_some() {
            r.kill_activation(f.activation_id);
            r.remove_join(f.activation_id);
        }
        if f == target {
            break;
        }
    }
    for w in r.outgoing(end_id, 0) {
        return StepResult::jump_to(w.to_block_id);
    }
    if r.return_from_function(br) {
        StepResult::handled()
    } else {
        StepResult::end()
    }
}

/// Upstream STOP_SCRIPT clears the player's script registry; the
/// runner-level equivalent is stopping the runner (the registry layer
/// lands with the bot framework).
fn exec_stop_script(_: &mut ScriptRunner, _: &BlockInstance, _: &mut Branch) -> StepResult {
    // Upstream clears the player's script registry (game-bound, lands
    // with the bot framework), then ends the branch.
    StepResult::end()
}

/// Upstream PAUSE_SCRIPT.
fn exec_pause_script(r: &mut ScriptRunner, _: &BlockInstance, _: &mut Branch) -> StepResult {
    r.pause();
    StepResult::continue_via(0)
}

/// Upstream SET_SCRIPT swaps the running script through AiScriptRegistry
/// (load/assign/fireStart on the player). The registry is game-bound and
/// lands with the bot framework, so until then this ends the branch the
/// way an unknown script name would.
// ===== Upstream DataExecutor (executors/DataExecutor.java) =====
// UUID/ITEM coercion, entity name lookup and item-stack parsing are
// game-bound and land with the BotHook fake-player work.

/// Portable subset of upstream ParamEval.asVec3: literal triples.
fn as_vec3_value(v: &RuntimeValue) -> (f64, f64, f64) {
    match v {
        RuntimeValue::Vec3(x, y, z) => (*x, *y, *z),
        RuntimeValue::Str(s) => {
            let parts: Vec<f64> = s.split_whitespace().filter_map(|q| q.parse::<f64>().ok()).collect();
            if parts.len() >= 3 { (parts[0], parts[1], parts[2]) } else { (0.0, 0.0, 0.0) }
        }
        _ => (0.0, 0.0, 0.0),
    }
}

/// Upstream DataExecutor.coerce, portable subset.
fn data_coerce(ty: VarType, v: &RuntimeValue) -> RuntimeValue {
    match ty {
        VarType::Bool => RuntimeValue::Bool(v.as_bool()),
        VarType::Int => RuntimeValue::Int(v.as_int()),
        VarType::Double => RuntimeValue::Double(v.as_double()),
        VarType::String => RuntimeValue::Str(v.as_string()),
        VarType::Position => { let (x, y, z) = as_vec3_value(v); RuntimeValue::Vec3(x, y, z) }
        VarType::Rotation => { let (yaw, pitch) = ParamEval::as_rotation(v); RuntimeValue::Rot(yaw, pitch) }
        _ => v.clone(),
    }
}

/// Upstream DataExecutor.stringify, portable subset (item and entity
/// names resolve in the game layer).
fn stringify_value(v: &RuntimeValue) -> String {
    match v {
        RuntimeValue::Vec3(x, y, z) => format!("{} {} {}", x, y, z),
        RuntimeValue::Rot(yaw, pitch) => format!("{} {}", yaw, pitch),
        other => other.as_string(),
    }
}

/// Upstream DataExecutor SET_VAR.
fn exec_set_var(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    let name = ParamEval::eval_string(b, "name", r, br);
    let value = ParamEval::raw(b, "value", r, br);
    match r.variable(&name).map(|v| v.ty) {
        Some(ty) => {
            let nv = data_coerce(ty, &value);
            if let Some(v) = r.variable_mut(&name) { v.value = nv; }
        }
        None => {
            r.define_variable(name, RuntimeVariable { ty: VarType::String, value: RuntimeValue::Str(value.as_string()) });
        }
    }
    StepResult::continue_via(0)
}

/// Upstream DataExecutor CHANGE_VAR.
fn exec_change_var(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    let name = ParamEval::eval_string(b, "name", r, br);
    let existing = r.variable(&name).map(|v| (v.ty, v.value.clone()));
    match existing {
        None => {
            let d = ParamEval::eval_double(b, "delta", r, br);
            r.define_variable(name, RuntimeVariable { ty: VarType::Double, value: RuntimeValue::Double(d) });
        }
        Some((ty, cur)) => {
            let nv = match ty {
                VarType::Bool => RuntimeValue::Bool(!cur.as_bool()),
                VarType::Int => RuntimeValue::Int(cur.as_int() + ParamEval::eval_int(b, "delta", r, br)),
                VarType::Double => RuntimeValue::Double(cur.as_double() + ParamEval::eval_double(b, "delta", r, br)),
                VarType::Position => {
                    let (dx, dy, dz) = as_vec3_value(&ParamEval::raw(b, "delta", r, br));
                    let (cx, cy, cz) = as_vec3_value(&cur);
                    RuntimeValue::Vec3(cx + dx, cy + dy, cz + dz)
                }
                VarType::Rotation => {
                    let (dy, dp) = ParamEval::as_rotation(&ParamEval::raw(b, "delta", r, br));
                    let (cy, cp) = ParamEval::as_rotation(&cur);
                    RuntimeValue::Rot(cy + dy, cp + dp)
                }
                VarType::String => RuntimeValue::Str(cur.as_string() + &ParamEval::raw(b, "delta", r, br).as_string()),
                VarType::Uuid | VarType::Item => cur.clone(),
            };
            if let Some(v) = r.variable_mut(&name) { v.value = nv; }
        }
    }
    StepResult::continue_via(0)
}

fn exec_set_script(_: &mut ScriptRunner, _: &BlockInstance, _: &mut Branch) -> StepResult {
    StepResult::end()
}

/// Upstream ControlExecutor IF.
fn exec_if(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    let act = r.open_activation(BlockType::If, b.id, b.paired_id, 0);
    if ParamEval::eval_bool(b, "condition", r, br) {
        return StepResult::enter_body(0, act);
    }
    let elif = side_target(r, b.id);
    if elif.is_some() && b.paired_id >= 0 {
        return StepResult::push_and_jump(elif.expect("checked").id, act);
    }
    skip_to_end(b, act)
}

/// Upstream ControlExecutor ELSE_IF.
fn exec_else_if(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    if ParamEval::eval_bool(b, "condition", r, br) {
        let act = r.open_activation(BlockType::ElseIf, b.id, b.paired_id, 0);
        return StepResult::enter_body(0, act);
    }
    if let Some(next) = side_target(r, b.id) {
        return StepResult::jump_to(next.id);
    }
    if let Some(root) = chain_root(r, b.id) {
        if root.paired_id >= 0 {
            return StepResult::jump_to(root.paired_id);
        }
    }
    let act = r.open_activation(BlockType::ElseIf, b.id, b.paired_id, 0);
    skip_to_end(b, act)
}

/// Upstream ControlExecutor FOR.
fn exec_for(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    let count = ParamEval::eval_int(b, "count", r, br) as i32;
    let act = r.open_activation(BlockType::For, b.id, b.paired_id, count);
    if count <= 0 {
        return skip_to_end(b, act);
    }
    StepResult::enter_body(0, act)
}

/// Upstream ControlExecutor WHILE.
fn exec_while(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    let act = r.open_activation(BlockType::While, b.id, b.paired_id, 0);
    if !ParamEval::eval_bool(b, "condition", r, br) {
        return skip_to_end(b, act);
    }
    StepResult::enter_body(0, act)
}

/// Upstream ControlExecutor BLOCK_END / stepEnd.
fn exec_block_end(r: &mut ScriptRunner, b: &BlockInstance, br: &mut Branch) -> StepResult {
    let act = br.frames.last().map(|f| f.activation_id).unwrap_or(-1);
    let js = match r.join_state(act) {
        Some(js) => js.clone(),
        None => return StepResult::continue_via(0),
    };
    if r.running_count(act) != 0 {
        return StepResult::end();
    }
    r.kill_activation(act);
    let mut repeat = false;
    match js.start_type {
        BlockType::For => {
            let rem = js.remaining - 1;
            if let Some(j) = r.join_state_mut(act) {
                j.remaining = rem;
            }
            repeat = rem > 0;
        }
        BlockType::While => {
            let start = r.script.block(js.start_block_id).cloned();
            repeat = match start {
                Some(s) => ParamEval::eval_bool(&s, "condition", r, br),
                None => false,
            };
        }
        _ => {}
    }
    if repeat {
        if let Some(j) = r.join_state_mut(act) {
            j.iteration += 1;
        }
        let js_now = r.join_state(act).cloned().unwrap_or_else(|| js.clone());
        r.relaunch_body(&js_now, br);
        return StepResult::handled();
    }
    r.remove_join(act);
    if b.paired_id >= 0 {
        if let Some(start) = r.script.block(b.paired_id).cloned() {
            if BlockType::from_index(start.block_type) == BlockType::ElseIf {
                if let Some(root) = chain_root(r, start.id) {
                    if root.paired_id >= 0 {
                        return StepResult::pop_and_jump(root.paired_id);
                    }
                }
            }
        }
    }
    StepResult::exit_body(0)
}

const MAX_CHAT_QUEUE: usize = 256;
const STEP_BUDGET: usize = 1_000_000;
const MAX_TURNS_PER_TICK: i32 = 256;

/// Upstream ScriptRunner.
pub struct ScriptRunner {
    pub script: AiScript,
    pub branches: Vec<Branch>,
    joins: HashMap<i32, JoinState>,
    variables: HashMap<String, RuntimeVariable>,
    activation_seq: i32,
    call_seq: u64,
    out_index: HashMap<(i32, i32), Vec<Wire>>,
    in_index: HashMap<i32, Vec<Wire>>,
    hat_index: HashMap<i32, Vec<BlockInstance>>,
    paused: bool,
    damaged_this_tick: bool,
    path_reached_this_tick: bool,
    pub world: Option<Box<dyn crate::vec_eval::World>>,
    pub sensor: Option<Box<dyn crate::sensor::SensorHook>>,
    path_failed_this_tick: bool,
    chat_queue: VecDeque<String>,
}

// ===== Upstream OperatorExecutor, portable reporters =====
// NUM_CALC is live through the expr.rs port of upstream's ExprEval;
// BOOL_CALC/STRING_CALC through bool_eval.rs and str_eval.rs (upstream
// BoolEval/StrEval); POS_CALC/DIR_CALC through vec_eval.rs (upstream
// VecEval). VecEval's world seam is a World trait object the game layer
// supplies (player level, position and rotation); the harness runs worldless.
// RANDOM_* uses a thread-local xorshift* seeded from the clock,
// standing in for ThreadLocalRandom.

thread_local! {
    static RNG_STATE: std::cell::Cell<u64> = std::cell::Cell::new(0);
}

fn rng_next_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    RNG_STATE.with(|st| {
        let mut x = st.get();
        if x == 0 {
            x = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E3779B97F4A7C15);
        }
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        st.set(x);
        x.wrapping_mul(0x2545F4914F6CDD1D)
    })
}

/// Upstream OperatorExecutor.valuesEqual, portable subset (item stacks
/// and entity refs live in the game layer).
fn op_values_equal(a: &RuntimeValue, b: &RuntimeValue) -> bool {
    match (a, b) {
        (RuntimeValue::Int(x), RuntimeValue::Int(y)) => x == y,
        (RuntimeValue::Double(x), RuntimeValue::Double(y)) => x == y,
        (RuntimeValue::Int(x), RuntimeValue::Double(y)) | (RuntimeValue::Double(y), RuntimeValue::Int(x)) => (*x as f64) == *y,
        (RuntimeValue::Bool(x), RuntimeValue::Bool(y)) => x == y,
        (RuntimeValue::Str(x), RuntimeValue::Str(y)) => x == y,
        (RuntimeValue::Vec3(x1, y1, z1), RuntimeValue::Vec3(x2, y2, z2)) => x1 == x2 && y1 == y2 && z1 == z2,
        (RuntimeValue::Rot(a1, a2), RuntimeValue::Rot(b1, b2)) => a1 == b1 && a2 == b2,
        _ => false,
    }
}

impl ReporterEval for ScriptRunner {
    fn eval_reporter(&self, reporter: &BlockInstance, _branch: &Branch) -> Option<RuntimeValue> {
        // Upstream ControlExecutor.register reporter table (LOOP_ITER).
        if let Some(v) = eval_control_reporter(reporter, self, _branch) {
            return Some(v);
        }
        let ty = BlockType::from_index(reporter.block_type);
        match ty {
            BlockType::ReadVar => {
                let name = ParamEval::eval_string(reporter, "name", self, _branch);
                return self.variable(&name).map(|v| v.value.clone());
            }
            BlockType::ToString => {
                let v = ParamEval::raw(reporter, "value", self, _branch);
                return Some(RuntimeValue::Str(stringify_value(&v)));
            }
            BlockType::Ternary => {
                let cond = ParamEval::eval_bool(reporter, "condition", self, _branch);
                let pick = if cond { "trueValue" } else { "falseValue" };
                return Some(ParamEval::raw(reporter, pick, self, _branch));
            }
            BlockType::Compare => {
                let a = ParamEval::eval_double(reporter, "a", self, _branch);
                let bv = ParamEval::eval_double(reporter, "b", self, _branch);
                let op = ParamEval::eval_string(reporter, "op", self, _branch);
                let res = match op.as_str() {
                    ">" => a > bv,
                    "=" => a == bv,
                    "\u{2260}" => a != bv,
                    "\u{2264}" => a <= bv,
                    "\u{2265}" => a >= bv,
                    _ => a < bv,
                };
                return Some(RuntimeValue::Bool(res));
            }
            BlockType::Equality => {
                let a = ParamEval::raw(reporter, "a", self, _branch);
                let b = ParamEval::raw(reporter, "b", self, _branch);
                let eq = op_values_equal(&a, &b);
                let neq = ParamEval::eval_string(reporter, "op", self, _branch) == "\u{2260}";
                return Some(RuntimeValue::Bool(neq != eq));
            }
            BlockType::Logic => {
                let op = ParamEval::eval_string(reporter, "op", self, _branch);
                let a = ParamEval::eval_bool(reporter, "a", self, _branch);
                let bv = ParamEval::eval_bool(reporter, "b", self, _branch);
                let res = match op.as_str() {
                    "or" => a || bv,
                    "xor" => a ^ bv,
                    _ => a && bv,
                };
                return Some(RuntimeValue::Bool(res));
            }
            BlockType::And => {
                let res = ParamEval::eval_bool(reporter, "a", self, _branch) && ParamEval::eval_bool(reporter, "b", self, _branch);
                return Some(RuntimeValue::Bool(res));
            }
            BlockType::Or => {
                let res = ParamEval::eval_bool(reporter, "a", self, _branch) || ParamEval::eval_bool(reporter, "b", self, _branch);
                return Some(RuntimeValue::Bool(res));
            }
            BlockType::Not => {
                let res = !ParamEval::eval_bool(reporter, "a", self, _branch);
                return Some(RuntimeValue::Bool(res));
            }
            BlockType::Contains => {
                let mut a = ParamEval::eval_string(reporter, "a", self, _branch);
                let mut needle = ParamEval::eval_string(reporter, "b", self, _branch);
                if !ParamEval::eval_bool(reporter, "checkCase", self, _branch) {
                    a = a.to_lowercase();
                    needle = needle.to_lowercase();
                }
                return Some(RuntimeValue::Bool(a.contains(&needle)));
            }
            BlockType::RandomInt => {
                let mut lo = ParamEval::eval_int(reporter, "min", self, _branch);
                let mut hi = ParamEval::eval_int(reporter, "max", self, _branch);
                if hi < lo { std::mem::swap(&mut lo, &mut hi); }
                let span = (hi as i128 - lo as i128 + 1).max(1);
                let val = lo as i128 + (rng_next_u64() as i128 % span);
                return Some(RuntimeValue::Int(val as i64));
            }
            BlockType::RandomDouble => {
                let lo = ParamEval::eval_double(reporter, "min", self, _branch);
                let hi = ParamEval::eval_double(reporter, "max", self, _branch);
                let (lo, hi) = if hi < lo { (hi, lo) } else { (lo, hi) };
                let u = (rng_next_u64() >> 11) as f64 / (1u64 << 53) as f64;
                return Some(RuntimeValue::Double(lo + (hi - lo) * u));
            }
            BlockType::Vec3 => {
                let x = ParamEval::eval_double(reporter, "x", self, _branch);
                let y = ParamEval::eval_double(reporter, "y", self, _branch);
                let z = ParamEval::eval_double(reporter, "z", self, _branch);
                return Some(RuntimeValue::Vec3(x, y, z));
            }
            BlockType::Rot => {
                let yaw = ParamEval::eval_double(reporter, "yaw", self, _branch) as f32;
                let pitch = ParamEval::eval_double(reporter, "pitch", self, _branch) as f32;
                return Some(RuntimeValue::Rot(yaw, pitch));
            }
            BlockType::NumCalc => {
                // Upstream OperatorExecutor NUM_CALC: {name} resolves
                // against the runner's variables, Input<n> against this
                // block's own params; [i] indexes positions, rotations
                // (upstream float[]), and nothing else.
                let expr_text = ParamEval::eval_string(reporter, "expression", self, _branch);
                let mut vars = |name: &str, idx: i32| -> f64 {
                    let raw = if crate::expr::is_input_ref(name) {
                        ParamEval::raw(reporter, name, self, _branch)
                    } else {
                        match self.variable(name) {
                            Some(v) => v.value.clone(),
                            None => return 0.0,
                        }
                    };
                    if idx >= 0 {
                        match raw {
                            RuntimeValue::Vec3(x, y, z) => match idx {
                                0 => x,
                                1 => y,
                                2 => z,
                                _ => 0.0,
                            },
                            RuntimeValue::Rot(yaw, pitch) => match idx {
                                0 => yaw as f64,
                                1 => pitch as f64,
                                _ => 0.0,
                            },
                            _ => 0.0,
                        }
                    } else {
                        raw.as_double()
                    }
                };
                return Some(RuntimeValue::Double(crate::expr::eval(
                    &expr_text,
                    &mut vars,
                )));
            }
            BlockType::BoolCalc => {
                // Upstream OperatorExecutor BOOL_CALC.
                let expr_text = ParamEval::eval_string(reporter, "expression", self, _branch);
                let mut vars = |name: &str| -> Option<crate::bool_eval::BoolVal> {
                    let raw = if crate::expr::is_input_ref(name) {
                        ParamEval::raw(reporter, name, self, _branch)
                    } else {
                        match self.variable(name) {
                            Some(v) => v.value.clone(),
                            None => return None,
                        }
                    };
                    Some(match &raw {
                        RuntimeValue::Bool(b) => crate::bool_eval::BoolVal::Bool(*b),
                        RuntimeValue::Int(i) => crate::bool_eval::BoolVal::Num(*i as f64),
                        RuntimeValue::Double(d) => crate::bool_eval::BoolVal::Num(*d),
                        RuntimeValue::Str(s) => crate::bool_eval::BoolVal::Str(s.clone()),
                        RuntimeValue::Rot(y, p) => {
                            crate::bool_eval::BoolVal::Arr(vec![*y as f64, *p as f64])
                        }
                        RuntimeValue::Vec3(x, y, z) => crate::bool_eval::BoolVal::Other(format!(
                            "{} {} {}",
                            x, y, z
                        )),
                    })
                };
                return Some(RuntimeValue::Bool(crate::bool_eval::eval(
                    &expr_text,
                    &mut vars,
                )));
            }
            BlockType::StringCalc => {
                // Upstream OperatorExecutor STRING_CALC: {var} and Input<n>
                // resolve through DataExecutor.stringify; missing variables
                // stringify to the empty string.
                let expr_text = ParamEval::eval_string(reporter, "expression", self, _branch);
                let mut vars = |name: &str| -> Option<String> {
                    let raw = if crate::expr::is_input_ref(name) {
                        ParamEval::raw(reporter, name, self, _branch)
                    } else {
                        match self.variable(name) {
                            Some(v) => v.value.clone(),
                            None => return None,
                        }
                    };
                    Some(stringify_value(&raw))
                };
                return Some(RuntimeValue::Str(crate::str_eval::eval(
                    &expr_text,
                    &mut vars,
                )));
            }
            BlockType::PosCalc => {
                // Upstream OperatorExecutor POS_CALC: VecEval over the level
                // world. The world hook is a World trait object the game
                // layer attaches (upstream levelWorld: the player's level,
                // position and rotation); without it ~ bases resolve to 0,
                // ^ locals skip the world basis and climb/fall pass through.
                let expr_text = ParamEval::eval_string(reporter, "expression", self, _branch);
                let mut vars = |name: &str| -> Option<crate::vec_eval::VecVal> {
                    let raw = if crate::expr::is_input_ref(name) {
                        ParamEval::raw(reporter, name, self, _branch)
                    } else {
                        match self.variable(name) {
                            Some(v) => v.value.clone(),
                            None => return None,
                        }
                    };
                    Some(match raw {
                        RuntimeValue::Vec3(x, y, z) => crate::vec_eval::VecVal::Arr(vec![x, y, z]),
                        RuntimeValue::Rot(y, p) => crate::vec_eval::VecVal::Rot([y as f64, p as f64]),
                        RuntimeValue::Int(i) => crate::vec_eval::VecVal::Num(i as f64),
                        RuntimeValue::Double(d) => crate::vec_eval::VecVal::Num(d),
                        RuntimeValue::Bool(b) => crate::vec_eval::VecVal::Bool(b),
                        RuntimeValue::Str(_) => crate::vec_eval::VecVal::Other,
                    })
                };
                let world = self.world.as_deref();
                let v = crate::vec_eval::eval(&expr_text, 3, &mut vars, world);
                return Some(RuntimeValue::Vec3(v[0], v[1], v[2]));
            }
            BlockType::DirCalc => {
                // Upstream OperatorExecutor DIR_CALC: same vars hook, required
                // arity 2, result cast to floats (upstream float[]).
                let expr_text = ParamEval::eval_string(reporter, "expression", self, _branch);
                let mut vars = |name: &str| -> Option<crate::vec_eval::VecVal> {
                    let raw = if crate::expr::is_input_ref(name) {
                        ParamEval::raw(reporter, name, self, _branch)
                    } else {
                        match self.variable(name) {
                            Some(v) => v.value.clone(),
                            None => return None,
                        }
                    };
                    Some(match raw {
                        RuntimeValue::Vec3(x, y, z) => crate::vec_eval::VecVal::Arr(vec![x, y, z]),
                        RuntimeValue::Rot(y, p) => crate::vec_eval::VecVal::Rot([y as f64, p as f64]),
                        RuntimeValue::Int(i) => crate::vec_eval::VecVal::Num(i as f64),
                        RuntimeValue::Double(d) => crate::vec_eval::VecVal::Num(d),
                        RuntimeValue::Bool(b) => crate::vec_eval::VecVal::Bool(b),
                        RuntimeValue::Str(_) => crate::vec_eval::VecVal::Other,
                    })
                };
                let world = self.world.as_deref();
                let v = crate::vec_eval::eval(&expr_text, 2, &mut vars, world);
                return Some(RuntimeValue::Rot(v[0] as f32, v[1] as f32));
            }
            // The Sensor and Function reporters land with those executors.
            // Upstream returns null on a missing reporter.
            _ => {
                // Upstream SensorExecutor: the world-bound sensor reporters
                // dispatch to the sensor port; the hookless fallback matches
                // upstream with no live world.
                if crate::sensor::is_sensor_type(ty) {
                    return crate::sensor::sensor_report(ty, reporter, _branch, self);
                }
            }
        }
        None
    }
}

impl ScriptRunner {
    pub fn new(script: AiScript) -> Self {
        let mut r = Self {
            script,
            branches: Vec::new(),
            joins: HashMap::new(),
            variables: HashMap::new(),
            activation_seq: 0,
            call_seq: 0,
            out_index: HashMap::new(),
            in_index: HashMap::new(),
            hat_index: HashMap::new(),
            paused: false,
            damaged_this_tick: false,
            path_reached_this_tick: false,
            world: None,
            sensor: None,
            path_failed_this_tick: false,
            chat_queue: VecDeque::new(),
        };
        r.build_indexes();
        r.init_variables();
        r
    }

    fn build_indexes(&mut self) {
        let wires = self.script.wires.clone();
        for w in wires {
            self.out_index.entry((w.from_block_id, w.out_port)).or_default().push(w.clone());
            self.in_index.entry(w.to_block_id).or_default().push(w);
        }
        for bucket in self.out_index.values_mut() {
            if bucket.len() > 1 {
                let script = &self.script;
                bucket.sort_by(|a, b| {
                    let ba = script.block(a.to_block_id);
                    let bb = script.block(b.to_block_id);
                    match (ba, bb) {
                        (Some(ba), Some(bb)) => ba.x.partial_cmp(&bb.x).unwrap_or(std::cmp::Ordering::Equal).then(ba.y.partial_cmp(&bb.y).unwrap_or(std::cmp::Ordering::Equal)),
                        _ => std::cmp::Ordering::Equal,
                    }
                });
            }
        }
    }

    pub fn outgoing(&self, block_id: i32, out_port: i32) -> Vec<Wire> {
        self.out_index.get(&(block_id, out_port)).cloned().unwrap_or_default()
    }

    pub fn hats(&mut self, ty: BlockType) -> Vec<BlockInstance> {
        if let Some(v) = self.hat_index.get(&ty.index()) {
            return v.clone();
        }
        let v: Vec<BlockInstance> = self.script.hat_blocks(ty.index()).into_iter().cloned().collect();
        self.hat_index.insert(ty.index(), v.clone());
        v
    }

    fn init_variables(&mut self) {
        let vars = self.script.variables.clone();
        for v in vars {
            // ITEM defaults resolve through DataExecutor.asItemStack once
            // that executor lands; the raw string stands until then.
            self.variables.insert(
                v.qualified_name(),
                RuntimeVariable { ty: v.ty, value: RuntimeValue::Str(v.default_value.clone().unwrap_or_default()) },
            );
        }
    }

    pub fn variable(&self, name: &str) -> Option<&RuntimeVariable> {
        self.variables.get(name)
    }

    pub fn define_variable(&mut self, name: impl Into<String>, v: RuntimeVariable) {
        self.variables.insert(name.into(), v);
    }

    pub fn remove_variable(&mut self, name: &str) {
        self.variables.remove(name);
    }

    pub fn variable_mut(&mut self, name: &str) -> Option<&mut RuntimeVariable> {
        self.variables.get_mut(name)
    }

    pub fn next_call_seq(&mut self) -> u64 {
        self.call_seq += 1;
        self.call_seq
    }

    pub fn open_activation(&mut self, start_type: BlockType, start_id: i32, end_id: i32, remaining: i32) -> i32 {
        self.activation_seq += 1;
        let act = self.activation_seq;
        self.joins.insert(act, JoinState { start_type, start_block_id: start_id, end_block_id: end_id, remaining, iteration: 0 });
        act
    }

    pub fn join_state(&self, activation_id: i32) -> Option<&JoinState> {
        self.joins.get(&activation_id)
    }

    pub fn remove_join(&mut self, activation_id: i32) {
        self.joins.remove(&activation_id);
    }

    pub fn join_state_mut(&mut self, activation_id: i32) -> Option<&mut JoinState> {
        self.joins.get_mut(&activation_id)
    }

    /// Upstream runningCount over live branches still inside an activation.
    pub fn running_count(&self, activation_id: i32) -> i32 {
        self.branches
            .iter()
            .filter(|b| !b.dead && b.in_activation(activation_id))
            .count() as i32
    }

    pub fn kill_activation(&mut self, activation_id: i32) {
        for b in &mut self.branches {
            if !b.dead && b.in_activation(activation_id) {
                b.dead = true;
            }
        }
    }

    pub fn relaunch_body(&mut self, js: &JoinState, br: &mut Branch) {
        let start = js.start_block_id;
        self.navigate(br, start, 0, &mut Vec::new());
    }

    pub fn fire_start(&mut self) {
        for start in self.hats(BlockType::Start) {
            self.spawn_from_event(&start, None);
        }
    }

    pub fn fire_event(&mut self, ty: BlockType) {
        if self.paused {
            return;
        }
        for b in self.hats(ty) {
            if self.is_event_active(b.id) {
                continue;
            }
            self.spawn_from_event(&b, None);
        }
    }

    pub fn enqueue_chat_message(&mut self, message: impl Into<String>) {
        let message = message.into();
        while self.chat_queue.len() >= MAX_CHAT_QUEUE {
            self.chat_queue.pop_front();
        }
        self.chat_queue.push_back(message);
    }

    fn dispatch_message(&mut self, message: String) {
        for b in self.hats(BlockType::OnMessage) {
            self.spawn_from_event(&b, Some(message.clone()));
        }
    }

    fn fire_toggle_events(&mut self) {
        let toggles = self.hats(BlockType::OnToggle);
        if toggles.is_empty() {
            return;
        }
        for hat in toggles {
            if self.is_event_active(hat.id) {
                continue;
            }
            let tmp = Branch::new(hat.id);
            if ParamEval::eval_bool(&hat, "condition", self, &tmp) {
                self.spawn_from_event(&hat, None);
            }
        }
    }

    fn is_event_active(&self, event_id: i32) -> bool {
        self.branches.iter().any(|br| !br.dead && br.origin_id == event_id)
    }

    fn spawn_from_event(&mut self, event: &BlockInstance, payload: Option<String>) {
        let outs = self.outgoing(event.id, 0);
        for w in outs {
            let mut br = Branch::new(w.to_block_id);
            br.origin_id = event.id;
            br.event_message = payload.clone();
            self.branches.push(br);
        }
    }

    pub fn mark_damaged(&mut self) {
        self.damaged_this_tick = true;
    }
    pub fn was_damaged(&self) -> bool {
        self.damaged_this_tick
    }
    pub fn mark_path_reached(&mut self) {
        self.path_reached_this_tick = true;
    }
    pub fn mark_path_failed(&mut self) {
        self.path_failed_this_tick = true;
    }
    pub fn path_reached(&self) -> bool {
        self.path_reached_this_tick
    }
    pub fn path_failed(&self) -> bool {
        self.path_failed_this_tick
    }

    pub fn pause(&mut self) {
        if self.paused {
            return;
        }
        self.paused = true;
        for b in &mut self.branches {
            b.path_wait_for = -1;
        }
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn stop(&mut self) {
        for b in &mut self.branches {
            b.dead = true;
        }
        self.branches.clear();
        self.joins.clear();
        self.chat_queue.clear();
        self.paused = false;
    }

    /// Upstream tick.
    pub fn tick(&mut self) {
        if self.paused {
            self.damaged_this_tick = false;
            self.path_reached_this_tick = false;
            self.path_failed_this_tick = false;
            return;
        }
        self.fire_toggle_events();
        while let Some(chat) = self.chat_queue.pop_front() {
            self.dispatch_message(chat);
        }
        if self.branches.is_empty() {
            self.damaged_this_tick = false;
            self.path_reached_this_tick = false;
            self.path_failed_this_tick = false;
            return;
        }

        for br in &mut self.branches {
            if br.dead {
                continue;
            }
            br.turns = 0;
            br.calls_this_tick = 0;
            br.parked = br.decrement_wait();
        }

        let mut any_ran = true;
        while any_ran {
            any_ran = false;
            let count = self.branches.len();
            for i in 0..count {
                let (dead, parked, over) = {
                    let br = &self.branches[i];
                    (br.dead, br.parked, br.turns >= MAX_TURNS_PER_TICK)
                };
                if dead || parked || over {
                    continue;
                }
                self.branches[i].turns += 1;
                let mut br = std::mem::replace(&mut self.branches[i], Branch::new(-1));
                let mut spawned: Vec<Branch> = Vec::new();
                self.run_slice(&mut br, &mut spawned);
                // Always write the branch back: a dead branch must mark its
                // slot dead too, or the placeholder re-runs forever.
                self.branches[i] = br;
                self.branches.extend(spawned);
                any_ran = true;
            }
        }
        self.branches.retain(|b| !b.dead);
        self.damaged_this_tick = false;
        self.path_reached_this_tick = false;
        self.path_failed_this_tick = false;
    }

    fn run_slice(&mut self, br: &mut Branch, spawned: &mut Vec<Branch>) {
        for _budget in 0..STEP_BUDGET {
            let b = match self.script.block(br.current_block_id) {
                Some(b) => b.clone(),
                None => {
                    br.dead = true;
                    return;
                }
            };
            let exec = match flow_dispatch(BlockType::from_index(b.block_type)) {
                Some(e) => e,
                None => {
                    // Upstream: no flow executor, end the branch.
                    br.dead = true;
                    return;
                }
            };
            let r = exec(self, &b, br);
            self.apply_result(br, &b, r, spawned);
            if br.dead {
                return;
            }
            if self.paused || br.wait_ticks > 0 {
                br.parked = true;
                return;
            }
            if Self::yields_after(BlockType::from_index(b.block_type)) {
                return;
            }
        }
    }

    fn yields_after(ty: BlockType) -> bool {
        match ty {
            BlockType::Break => false,
            BlockType::GotoPos | BlockType::GotoEntity | BlockType::FuncCall => true,
            _ => def_for(ty).map(|d| d.category == BlockCategory::Control).unwrap_or(false),
        }
    }

    fn apply_result(&mut self, br: &mut Branch, current: &BlockInstance, r: StepResult, spawned: &mut Vec<Branch>) {
        match r.kind {
            StepKind::End => br.dead = true,
            StepKind::Wait => br.wait_ticks = r.wait_ticks,
            StepKind::Handled => {}
            StepKind::Jump => {
                self.apply_frames(br, current, &r);
                br.current_block_id = r.jump_to;
                if r.wait_ticks > 0 {
                    br.wait_ticks = r.wait_ticks;
                }
            }
            StepKind::Continue => {
                self.apply_frames(br, current, &r);
                let from = current.id;
                let port = r.port;
                self.navigate(br, from, port, spawned);
                if r.wait_ticks > 0 {
                    br.wait_ticks = r.wait_ticks;
                }
            }
        }
    }

    fn apply_frames(&self, br: &mut Branch, current: &BlockInstance, r: &StepResult) {
        if r.pop_frame && !br.frames.is_empty() {
            br.frames.pop();
        }
        if r.push_frame {
            let mut f = ControlFrame::new(current.id, r.activation_id);
            if let Some(data) = &r.frame_data {
                f.data.extend(data.clone());
            }
            br.frames.push(f);
        }
    }

    /// Upstream navigate: first wire continues the branch, further
    /// wires fork.
    pub fn navigate(&mut self, br: &mut Branch, from_block_id: i32, out_port: i32, spawned: &mut Vec<Branch>) {
        let wires = self.outgoing(from_block_id, out_port);
        if wires.is_empty() {
            if !self.return_from_function(br) {
                br.dead = true;
            }
            return;
        }
        br.current_block_id = wires[0].to_block_id;
        for w in &wires[1..] {
            spawned.push(br.fork_at(w.to_block_id));
        }
    }

    /// Upstream returnFromFunction: call frames carry the FuncCall id.
    pub fn return_from_function(&mut self, br: &mut Branch) -> bool {
        let top = match br.frames.last() {
            Some(f) => f.clone(),
            None => return false,
        };
        // isCallFrame: container is a FuncCall block.
        if BlockType::from_index(
            self.script.block(top.container_block_id).map(|b| b.block_type).unwrap_or(-1),
        ) != BlockType::FuncCall
        {
            return false;
        }
        br.frames.pop();
        let call_id = top.container_block_id;
        if self.script.block(call_id).is_none() {
            return false;
        }
        self.navigate(br, call_id, 0, &mut Vec::new());
        true
    }
}
