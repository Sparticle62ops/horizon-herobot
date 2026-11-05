// HeroBot SensorExecutor ported from HerobaneNair/HeRoBot
// (herobot-mod/.../ai/runtime/executors/SensorExecutor.java, 265 lines),
// CC-BY-NC-4.0, plus the portable halves of the pieces it leans on:
// ItemSlots (herobot-common/.../ai/block/ItemSlots.java, 40 lines),
// SelectorExecutor.resolveFirstEntity's port-facing contract
// (herobot-mod/.../executors/SelectorExecutor.java, 64 lines) and
// DistanceCalculator's closest-point math
// (herobot-mod/.../util/DistanceCalculator.java, 86 lines).
//
// Sensor reporters are world-bound: they read entity state, block
// states, scoreboards and inventory from the live server. The port
// keeps every piece upstream computes from plain data (slot parsing,
// equipment-slot mapping, hitbox closest-point math, distance modes,
// subject defaulting, count/durability arithmetic) 1:1 and pushes the
// server reads behind the SensorHook seam the game layer supplies,
// the same shape as the World seam in vec_eval.rs. With no hook (the
// harness, or the game layer before the fake-player work lands) every
// reporter falls back to the value upstream produces with no live
// world: 0, false, an empty stack id string, or positive infinity for
// DISTANCE_TO with no target.

use crate::ai::BlockInstance;
use crate::block_registry::BlockType;
use crate::runtime::{ParamEval, RuntimeValue};

/// Upstream ItemSlots.NAMES: 9 hotbar, 27 inventory, 2 weapons, 4 armor.
pub const ITEM_SLOT_NAMES: [&str; 42] = [
    "hotbar.0", "hotbar.1", "hotbar.2", "hotbar.3", "hotbar.4", "hotbar.5",
    "hotbar.6", "hotbar.7", "hotbar.8", "inventory.0", "inventory.1",
    "inventory.2", "inventory.3", "inventory.4", "inventory.5",
    "inventory.6", "inventory.7", "inventory.8", "inventory.9",
    "inventory.10", "inventory.11", "inventory.12", "inventory.13",
    "inventory.14", "inventory.15", "inventory.16", "inventory.17",
    "inventory.18", "inventory.19", "inventory.20", "inventory.21",
    "inventory.22", "inventory.23", "inventory.24", "inventory.25",
    "inventory.26", "weapon.mainhand", "weapon.offhand", "armor.head",
    "armor.chest", "armor.legs", "armor.feet",
];

fn valid_slot(index: i32) -> i32 {
    if index >= 0 && (index as usize) < ITEM_SLOT_NAMES.len() {
        index
    } else {
        -1
    }
}

/// Upstream ItemSlots.index: a numeric value is taken as the slot
/// number (Number.intValue truncation), a string is looked up by name
/// first then parsed as a number, an empty string is -1, anything
/// unparseable is -1, and anything out of range is -1.
pub fn item_slot_index(value: &RuntimeValue) -> i32 {
    match value {
        RuntimeValue::Int(i) => valid_slot(*i as i32),
        RuntimeValue::Double(d) => valid_slot(*d as i64 as i32),
        other => {
            let s = value_string(other);
            if s.trim().is_empty() {
                return -1;
            }
            if let Some(idx) = ITEM_SLOT_NAMES.iter().position(|n| *n == s.trim()) {
                return idx as i32;
            }
            match s.trim().parse::<i32>() {
                Ok(n) => valid_slot(n),
                Err(_) => -1,
            }
        }
    }
}

/// String form of a value for upstream's String.valueOf calls.
fn value_string(v: &RuntimeValue) -> String {
    match v {
        RuntimeValue::Str(s) => s.clone(),
        RuntimeValue::Bool(b) => b.to_string(),
        RuntimeValue::Int(i) => i.to_string(),
        RuntimeValue::Double(d) => d.to_string(),
        RuntimeValue::Vec3(x, y, z) => format!("{} {} {}", x, y, z),
        RuntimeValue::Rot(y, p) => format!("{} {}", y, p),
    }
}

/// Upstream's EQUIP map keys mapped to equipment slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipSlot {
    Mainhand,
    Offhand,
    Head,
    Chest,
    Legs,
    Feet,
}

/// Upstream SensorExecutor.EQUIP: only those six exact keys resolve;
/// anything else is an empty stack upstream and None here.
pub fn equip_slot(name: &str) -> Option<EquipSlot> {
    match name {
        "weapon.mainhand" => Some(EquipSlot::Mainhand),
        "weapon.offhand" => Some(EquipSlot::Offhand),
        "armor.head" => Some(EquipSlot::Head),
        "armor.chest" => Some(EquipSlot::Chest),
        "armor.legs" => Some(EquipSlot::Legs),
        "armor.feet" => Some(EquipSlot::Feet),
        _ => None,
    }
}

/// Upstream net.minecraft.world.phys.AABB, the fields the port's math
/// needs (Mth.clamp bounds and getCenter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

impl Aabb {
    /// Upstream AABB.inflate: grow (or shrink with a negative amount)
    /// by d on every side.
    pub fn inflate(&self, d: f64) -> Aabb {
        Aabb {
            min_x: self.min_x - d,
            min_y: self.min_y - d,
            min_z: self.min_z - d,
            max_x: self.max_x + d,
            max_y: self.max_y + d,
            max_z: self.max_z + d,
        }
    }

    /// Upstream AABB.getCenter.
    pub fn center(&self) -> [f64; 3] {
        [
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
            (self.min_z + self.max_z) / 2.0,
        ]
    }
}

/// Upstream DistanceCalculator.closestPointOnHitbox: clamp the point
/// into the box on every axis.
pub fn closest_point_on_hitbox(aabb: &Aabb, p: [f64; 3]) -> [f64; 3] {
    [
        p[0].clamp(aabb.min_x, aabb.max_x),
        p[1].clamp(aabb.min_y, aabb.max_y),
        p[2].clamp(aabb.min_z, aabb.max_z),
    ]
}

/// Upstream DistanceCalculator.closestPointsBetween: walk the closest
/// point back and forth between the two boxes until it settles.
pub fn closest_points_between(from: &Aabb, to: &Aabb) -> ([f64; 3], [f64; 3]) {
    let center_from = from.center();
    let on_to = closest_point_on_hitbox(to, center_from);
    let on_from = closest_point_on_hitbox(from, on_to);
    let on_to_final = closest_point_on_hitbox(to, on_from);
    (on_from, on_to_final)
}

/// DISTANCE_TO's mode switch, upstream SensorExecutor lines ~167-174:
/// horizontal is the ground-plane distance, vertical the height
/// difference, anything else (including the empty default) the full
/// 3D distance.
pub fn distance_mode_value(a: [f64; 3], c: [f64; 3], mode: &str) -> f64 {
    match mode.trim().to_lowercase().as_str() {
        "horizontal" => ((a[0] - c[0]) * (a[0] - c[0]) + (a[2] - c[2]) * (a[2] - c[2])).sqrt(),
        "vertical" => (a[1] - c[1]).abs(),
        _ => {
            let dx = a[0] - c[0];
            let dy = a[1] - c[1];
            let dz = a[2] - c[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        }
    }
}

/// One inventory or equipment stack as the port's value model sees it.
/// Upstream keeps live ItemStack objects; the game layer fills this in
/// from the real stacks and the port does the arithmetic on it.
#[derive(Clone, Debug, Default)]
pub struct StackData {
    pub item_id: String,
    pub count: i64,
    pub damage: i64,
    pub max_damage: i64,
    pub max_stack: i64,
    pub damageable: bool,
}

/// ATTACK_COOLDOWN's two reads off a Player: the raw attack-strength
/// delay in ticks and the charge scale at 0 and 0.5 partial ticks
/// (upstream getCurrentItemAttackStrengthDelay / getAttackStrengthScale).
#[derive(Clone, Copy, Debug)]
pub struct AttackCooldown {
    pub delay: f64,
    pub scale_zero: f64,
    pub scale_half: f64,
}

/// The world seam behind every world-bound sensor reporter. The game
/// layer supplies it (player level, entity lookups, inventory,
/// scoreboard, command dispatch); the harness runs hookless and every
/// method defaults to None, which each reporter maps to the neutral
/// value upstream produces with no live world.
pub trait SensorHook {
    fn player_entity(&self) -> i64 {
        0
    }
    /// Upstream SelectorExecutor.resolveFirstEntity: an entity value
    /// resolves to itself, a UUID string to the entity with that UUID,
    /// anything else to the first entity a /select-style expression
    /// matches (selectors run with all permissions upstream). None
    /// covers the empty and failed cases.
    fn resolve_first_entity(&self, _sel: &RuntimeValue) -> Option<i64> {
        None
    }
    fn entity_position(&self, _entity: i64) -> Option<[f64; 3]> {
        None
    }
    fn entity_yaw_pitch(&self, _entity: i64) -> Option<(f32, f32)> {
        None
    }
    /// Health of a living entity; None is the non-living case
    /// (upstream HEALTH reports 0.0 for those).
    fn entity_health(&self, _entity: i64) -> Option<f64> {
        None
    }
    /// hurtTime of a living entity; None is the non-living case.
    fn entity_hurt_time(&self, _entity: i64) -> Option<i64> {
        None
    }
    /// Upstream ON_DAMAGE's other-entity half: hurtTime > 0 and
    /// hurtTime == hurtDuration, i.e. the hurt flash just started.
    fn entity_on_damage(&self, _entity: i64) -> Option<bool> {
        None
    }
    fn attack_cooldown(&self, _entity: i64) -> Option<AttackCooldown> {
        None
    }
    fn ping(&self, _entity: i64) -> Option<i64> {
        None
    }
    /// Upstream DOING_ACTION: the action pack reads for a bot player
    /// (forward/strafing/active actions), the entity flags otherwise.
    /// The action string is already validated by the port.
    fn doing_action(&self, _entity: i64, _action: &str) -> Option<bool> {
        None
    }
    fn day_time(&self) -> Option<i64> {
        None
    }
    fn game_time(&self) -> Option<i64> {
        None
    }
    /// Upstream IN_PATH: the bot's path follower still active, or the
    /// remote-path state for a real player.
    fn in_path(&self) -> Option<bool> {
        None
    }
    fn block_at(&self, _pos: [f64; 3]) -> Option<String> {
        None
    }
    fn entity_aabb(&self, _entity: i64) -> Option<Aabb> {
        None
    }
    /// IS_TOUCHING_BLOCK's scan: the port inflates the box itself and
    /// hands over the inflated area plus the raw block id string; the
    /// hook returns false for an unparseable id like upstream.
    fn any_block_in(&self, _area: &Aabb, _target: &str) -> Option<bool> {
        None
    }
    fn entity_has_tag(&self, _entity: i64, _tag: &str) -> Option<bool> {
        None
    }
    fn scoreboard_score(&self, _objective: &str, _entity: i64) -> Option<f64> {
        None
    }
    /// The acting player's stack in slot index 0..41 (upstream
    /// slotStack), None for an empty slot.
    fn slot_stack(&self, _index: i32) -> Option<StackData> {
        None
    }
    /// A living entity's equipment stack, None when empty.
    fn equipment(&self, _entity: i64, _slot: EquipSlot) -> Option<StackData> {
        None
    }
    /// Upstream DataExecutor.asItemStack for MAX_COUNT's item param.
    fn item_stack(&self, _item: &RuntimeValue) -> Option<StackData> {
        None
    }
    /// COMMAND_RESULT: run the command with all permissions and
    /// suppressed output, return its callback value.
    fn command_result(&self, _command: &str) -> Option<f64> {
        None
    }
}

pub struct NoSensorHook;
impl SensorHook for NoSensorHook {}

static NO_SENSOR: NoSensorHook = NoSensorHook;

/// The sensor reporters as a set, so the runtime dispatch can fall
/// through to sensor_report without naming them one by one.
pub fn is_sensor_type(ty: BlockType) -> bool {
    matches!(
        ty,
        BlockType::Scoreboard
            | BlockType::HasTag
            | BlockType::Health
            | BlockType::Position
            | BlockType::Yaw
            | BlockType::BlockAt
            | BlockType::IsTouchingBlock
            | BlockType::DistanceTo
            | BlockType::TimeOfDay
            | BlockType::EveryXTicks
            | BlockType::OnDamage
            | BlockType::AttackCooldown
            | BlockType::HurtTime
            | BlockType::Ping
            | BlockType::ItemInSlot
            | BlockType::Equipment
            | BlockType::GetCount
            | BlockType::MaxCount
            | BlockType::CommandResult
            | BlockType::InPath
            | BlockType::PathReached
            | BlockType::PathFailed
            | BlockType::DoingAction
    )
}

/// Upstream SensorExecutor.subject: with neither a literal param nor a
/// nested reporter in the slot the subject is the acting player;
/// otherwise the slot value resolves through the selector and a
/// failed resolve falls back to the player too.
fn subject(b: &BlockInstance, r: &crate::runtime::ScriptRunner, br: &crate::runtime::Branch, hook: &dyn SensorHook, slot: &str) -> i64 {
    if !b.params.contains_key(slot) && !b.reporter_params.contains_key(slot) {
        return hook.player_entity();
    }
    let raw = ParamEval::raw(b, slot, r, br);
    hook.resolve_first_entity(&raw).unwrap_or_else(|| hook.player_entity())
}

/// Upstream SensorExecutor.distanceShape: "hitbox" (any case, after
/// trim) takes the entity's bounding box, anything else a zero-size
/// box at the entity's position.
fn distance_shape(hook: &dyn SensorHook, entity: i64, shape: &str) -> Option<Aabb> {
    if shape.trim().eq_ignore_ascii_case("hitbox") {
        hook.entity_aabb(entity)
    } else {
        hook.entity_position(entity).map(|p| Aabb {
            min_x: p[0],
            min_y: p[1],
            min_z: p[2],
            max_x: p[0],
            max_y: p[1],
            max_z: p[2],
        })
    }
}

/// The sensor reporter dispatch, mirroring upstream's register() table
/// entry by entry. Game-bound reads go through the hook; everything
/// computed from plain data stays in the port.
pub fn sensor_report(
    ty: BlockType,
    b: &BlockInstance,
    br: &crate::runtime::Branch,
    r: &crate::runtime::ScriptRunner,
) -> Option<RuntimeValue> {
    let hook: &dyn SensorHook = match r.sensor.as_deref() {
        Some(h) => h,
        None => &NO_SENSOR,
    };
    match ty {
        BlockType::Health => {
            let e = subject(b, r, br, hook, "target");
            Some(RuntimeValue::Double(hook.entity_health(e).unwrap_or(0.0)))
        }
        BlockType::Position => {
            let e = subject(b, r, br, hook, "target");
            let p = hook.entity_position(e).unwrap_or([0.0, 0.0, 0.0]);
            Some(RuntimeValue::Vec3(p[0], p[1], p[2]))
        }
        BlockType::Yaw => {
            let e = subject(b, r, br, hook, "target");
            let axis = ParamEval::eval_string(b, "axis", r, br);
            let (yaw, pitch) = hook.entity_yaw_pitch(e).unwrap_or((0.0, 0.0));
            Some(RuntimeValue::Double(if axis == "pitch" {
                pitch as f64
            } else {
                yaw as f64
            }))
        }
        BlockType::TimeOfDay => Some(RuntimeValue::Double(
            (hook.day_time().unwrap_or(0) % 24000) as f64,
        )),
        BlockType::EveryXTicks => {
            let n = ParamEval::eval_int(b, "ticks", r, br).max(1);
            Some(RuntimeValue::Bool(match hook.game_time() {
                Some(t) => t % n == 0,
                None => true,
            }))
        }
        BlockType::DoingAction => {
            let action = ParamEval::eval_string(b, "action", r, br);
            let e = subject(b, r, br, hook, "target");
            Some(RuntimeValue::Bool(match action.as_str() {
                "walking" | "strafing" | "sprinting" | "sneaking" | "using" | "attacking"
                | "swinging" | "jumping" | "dropping" | "swapping" => {
                    hook.doing_action(e, &action).unwrap_or(false)
                }
                _ => false,
            }))
        }
        BlockType::OnDamage => {
            let e = subject(b, r, br, hook, "target");
            if e == hook.player_entity() {
                Some(RuntimeValue::Bool(r.was_damaged()))
            } else {
                Some(RuntimeValue::Bool(hook.entity_on_damage(e).unwrap_or(false)))
            }
        }
        BlockType::AttackCooldown => {
            let e = subject(b, r, br, hook, "target");
            let ticks = ParamEval::eval_string(b, "kind", r, br) == "ticksLeft";
            match hook.attack_cooldown(e) {
                Some(cd) => {
                    if ticks {
                        let v = (cd.delay * (1.0 - cd.scale_zero)).ceil() as i64;
                        Some(RuntimeValue::Double(v.max(0) as f64))
                    } else {
                        Some(RuntimeValue::Double(cd.scale_half))
                    }
                }
                None => Some(RuntimeValue::Double(if ticks { 0.0 } else { 1.0 })),
            }
        }
        BlockType::HurtTime => {
            let e = subject(b, r, br, hook, "target");
            Some(RuntimeValue::Double(
                hook.entity_hurt_time(e).unwrap_or(0) as f64,
            ))
        }
        BlockType::Ping => {
            let e = subject(b, r, br, hook, "target");
            Some(RuntimeValue::Double(hook.ping(e).unwrap_or(0) as f64))
        }
        BlockType::InPath => Some(RuntimeValue::Bool(hook.in_path().unwrap_or(false))),
        BlockType::PathReached => Some(RuntimeValue::Bool(r.path_reached())),
        BlockType::PathFailed => Some(RuntimeValue::Bool(r.path_failed())),
        BlockType::BlockAt => {
            let e = subject(b, r, br, hook, "target");
            let base = hook.entity_position(e).unwrap_or([0.0, 0.0, 0.0]);
            let raw = ParamEval::raw(b, "position", r, br);
            let pos = match raw {
                RuntimeValue::Vec3(x, y, z) => [x, y, z],
                other => {
                    let (x, y, z) = ParamEval::parse_vec3(&value_string(&other), (base[0], base[1], base[2]));
                    [x, y, z]
                }
            };
            Some(RuntimeValue::Str(hook.block_at(pos).unwrap_or_default()))
        }
        BlockType::IsTouchingBlock => {
            let block_id = ParamEval::eval_string(b, "block", r, br);
            let e = subject(b, r, br, hook, "target");
            match hook.entity_aabb(e) {
                Some(bb) => Some(RuntimeValue::Bool(
                    hook.any_block_in(&bb.inflate(0.001), &block_id).unwrap_or(false),
                )),
                None => Some(RuntimeValue::Bool(false)),
            }
        }
        BlockType::HasTag => {
            let sel = ParamEval::raw(b, "target", r, br);
            let tag = ParamEval::eval_string(b, "tag", r, br);
            Some(RuntimeValue::Bool(match hook.resolve_first_entity(&sel) {
                Some(e) => hook.entity_has_tag(e, &tag).unwrap_or(false),
                None => false,
            }))
        }
        BlockType::DistanceTo => {
            let sel = ParamEval::raw(b, "target", r, br);
            let e = match hook.resolve_first_entity(&sel) {
                Some(e) => e,
                None => return Some(RuntimeValue::Double(f64::INFINITY)),
            };
            let from = subject(b, r, br, hook, "from");
            let from_shape = ParamEval::eval_string(b, "fromShape", r, br);
            let to_shape = ParamEval::eval_string(b, "toShape", r, br);
            let (a, c) = match (
                distance_shape(hook, from, &from_shape),
                distance_shape(hook, e, &to_shape),
            ) {
                (Some(fa), Some(ta)) => closest_points_between(&fa, &ta),
                _ => return Some(RuntimeValue::Double(f64::INFINITY)),
            };
            let mode = ParamEval::eval_string(b, "mode", r, br);
            Some(RuntimeValue::Double(distance_mode_value(a, c, &mode)))
        }
        BlockType::Scoreboard => {
            let obj = ParamEval::eval_string(b, "objective", r, br);
            let sel = ParamEval::raw(b, "target", r, br);
            Some(RuntimeValue::Double(match hook.resolve_first_entity(&sel) {
                Some(e) => hook.scoreboard_score(&obj, e).unwrap_or(0.0),
                None => 0.0,
            }))
        }
        BlockType::ItemInSlot => {
            let slot = item_slot_index(&ParamEval::raw(b, "slot", r, br));
            let stack = if slot < 0 { None } else { hook.slot_stack(slot) };
            Some(RuntimeValue::Str(match stack {
                Some(sd) => sd.item_id,
                None => String::new(),
            }))
        }
        BlockType::Equipment => {
            let sel = ParamEval::raw(b, "target", r, br);
            let slot = ParamEval::eval_string(b, "slot", r, br);
            let stack = match (hook.resolve_first_entity(&sel), equip_slot(&slot)) {
                (Some(e), Some(es)) => hook.equipment(e, es),
                _ => None,
            };
            Some(RuntimeValue::Str(match stack {
                Some(sd) => sd.item_id,
                None => String::new(),
            }))
        }
        BlockType::GetCount => {
            let slot = item_slot_index(&ParamEval::raw(b, "slot", r, br));
            let kind = ParamEval::eval_string(b, "kind", r, br);
            let stack = if slot < 0 { None } else { hook.slot_stack(slot) };
            Some(RuntimeValue::Double(match stack {
                Some(sd) => {
                    if kind == "durability" {
                        if sd.damageable {
                            (sd.max_damage - sd.damage) as f64
                        } else {
                            0.0
                        }
                    } else {
                        sd.count as f64
                    }
                }
                None => 0.0,
            }))
        }
        BlockType::MaxCount => {
            let raw = ParamEval::raw(b, "item", r, br);
            let kind = ParamEval::eval_string(b, "kind", r, br);
            Some(RuntimeValue::Double(match hook.item_stack(&raw) {
                Some(sd) => {
                    if kind == "durability" {
                        if sd.damageable {
                            sd.max_damage as f64
                        } else {
                            0.0
                        }
                    } else {
                        sd.max_stack as f64
                    }
                }
                None => 0.0,
            }))
        }
        BlockType::CommandResult => {
            let mut cmd = ParamEval::eval_string(b, "command", r, br).trim().to_string();
            if cmd.starts_with('/') {
                cmd.remove(0);
            }
            if cmd.is_empty() {
                return Some(RuntimeValue::Double(0.0));
            }
            Some(RuntimeValue::Double(hook.command_result(&cmd).unwrap_or(0.0)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_slots_index_matches_upstream() {
        // hotbar.0..8 are 0..8, inventory.0..26 are 9..35, weapons 36/37,
        // armor 38..41; unknown names, empty strings and out-of-range
        // numbers are -1.
        assert_eq!(item_slot_index(&RuntimeValue::Str("hotbar.0".into())), 0);
        assert_eq!(item_slot_index(&RuntimeValue::Str("inventory.0".into())), 9);
        assert_eq!(item_slot_index(&RuntimeValue::Str("inventory.26".into())), 35);
        assert_eq!(item_slot_index(&RuntimeValue::Str("weapon.mainhand".into())), 36);
        assert_eq!(item_slot_index(&RuntimeValue::Str("weapon.offhand".into())), 37);
        assert_eq!(item_slot_index(&RuntimeValue::Str("armor.head".into())), 38);
        assert_eq!(item_slot_index(&RuntimeValue::Str("armor.feet".into())), 41);
        assert_eq!(item_slot_index(&RuntimeValue::Int(5)), 5);
        assert_eq!(item_slot_index(&RuntimeValue::Int(41)), 41);
        assert_eq!(item_slot_index(&RuntimeValue::Int(42)), -1);
        assert_eq!(item_slot_index(&RuntimeValue::Int(-1)), -1);
        assert_eq!(item_slot_index(&RuntimeValue::Double(7.9)), 7);
        assert_eq!(item_slot_index(&RuntimeValue::Str("  12  ".into())), 12);
        assert_eq!(item_slot_index(&RuntimeValue::Str("".into())), -1);
        assert_eq!(item_slot_index(&RuntimeValue::Str("nope".into())), -1);
    }

    #[test]
    fn equip_slots_map_exactly() {
        assert_eq!(equip_slot("weapon.mainhand"), Some(EquipSlot::Mainhand));
        assert_eq!(equip_slot("armor.head"), Some(EquipSlot::Head));
        assert_eq!(equip_slot("mainhand"), None);
        assert_eq!(equip_slot(""), None);
    }

    #[test]
    fn closest_points_between_matches_upstream() {
        let from = Aabb { min_x: 0.0, min_y: 0.0, min_z: 0.0, max_x: 2.0, max_y: 2.0, max_z: 2.0 };
        let to = Aabb { min_x: 10.0, min_y: 0.0, min_z: 10.0, max_x: 12.0, max_y: 2.0, max_z: 12.0 };
        let (a, c) = closest_points_between(&from, &to);
        assert_eq!(a, [2.0, 1.0, 2.0]);
        assert_eq!(c, [10.0, 1.0, 10.0]);
        // Overlapping boxes clamp the walk-through point inside both.
        let (a, c) = closest_points_between(&from, &from);
        assert_eq!(a, c);
    }

    #[test]
    fn distance_modes_match_upstream() {
        let a = [0.0, 5.0, 0.0];
        let c = [3.0, 1.0, 4.0];
        assert!((distance_mode_value(a, c, "horizontal") - 5.0).abs() < 1e-9);
        assert_eq!(distance_mode_value(a, c, "vertical"), 4.0);
        assert!((distance_mode_value(a, c, "") - (41.0f64).sqrt()).abs() < 1e-9);
        assert!((distance_mode_value(a, c, "NORMAL") - (41.0f64).sqrt()).abs() < 1e-9);
    }
}
