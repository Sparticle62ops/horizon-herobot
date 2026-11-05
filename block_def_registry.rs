// HeroBot BlockDefRegistry ported 1:1 from HerobaneNair/HeRoBot
// (herobot-common/.../ai/block/BlockDefRegistry.java, 358 lines), CC-BY-NC-4.0.
// One BlockDef per BlockType with upstream labels, categories, shapes,
// out-port counts and param slots, in upstream registration order.
// Upstream's static EnumMap becomes a LazyLock HashMap. get()'s
// IllegalArgumentException maps to def_for() returning None, which callers
// already fall back on like upstream on a null def.

use crate::block_registry::{BlockCategory, BlockDef, BlockShape, BlockType, ParamSlot, ParamType};
use crate::sensor::ITEM_SLOT_NAMES;
use std::collections::HashMap;
use std::sync::LazyLock;

const ACTION_MODES: &[&str] = &["once", "continuous", "interval"];
const TWICE_MODES: &[&str] = &["once", "twice", "continuous", "interval"];
const DIRECTIONS: &[&str] = &["north", "south", "east", "west", "up", "down"];
const LOOK_AT_MODES: &[&str] = &["eyes", "closest", "feet"];
const DISTANCE_SHAPES: &[&str] = &["position", "hitbox"];
const DISTANCE_MODES: &[&str] = &["normal", "horizontal", "vertical"];
const MOVE_TYPES: &[&str] = &["WALK", "SPRINT", "SPRINT_JUMP"];
const ACTION_TYPES: &[&str] =
    &["USE", "SWING", "JUMP", "ATTACK", "DROP_ITEM", "DROP_STACK", "SWAP_HANDS"];
const HANDEDNESS: &[&str] = &["left", "right"];
const DROP_AMOUNTS: &[&str] = &["1", "stack"];
const PICK_BLOCK_DATA: &[&str] = &["no data", "with data"];
const DOING_ACTIONS: &[&str] = &[
    "walking", "strafing", "sprinting", "sneaking", "using", "attacking", "swinging", "jumping",
    "dropping", "swapping",
];
const COMPARATORS: &[&str] = &["<", ">", "\u{2264}", "\u{2265}"];
const EQUALITY_OPS: &[&str] = &["=", "\u{2260}"];
const LOGIC_OPS: &[&str] = &["and", "or", "xor"];
const HOTBAR_SLOTS: &[&str] = &["1", "2", "3", "4", "5", "6", "7", "8", "9"];
const SWAP_TARGETS: &[&str] = &["offhand", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
const RECIPE_MODES: &[&str] = &["single", "max"];
const COUNT_KINDS: &[&str] = &["count", "durability"];
const COOLDOWN_KINDS: &[&str] = &["fraction", "ticksLeft"];
const MOVE_DIRS: &[&str] = &["forward", "backward", "stop"];
const STRAFE_DIRS: &[&str] = &["left", "right", "stop"];
const PATH_SETTINGS: &[&str] = &[
    "maxHorizontalDistance", "maxVerticalDistance",
    "nodeHorizontalDistance", "nodeVerticalDistance",
    "horizontalMoveCost", "verticalMoveCost", "swimCostMultiplier",
    "stopFollowing", "debug",
];
const MENUS: &[&str] = &["inventory", "container"];
const CLICK_MODES: &[&str] = &["click", "rightClick", "shiftClick", "throw", "throwAll"];
const DRAG_BUTTONS: &[&str] = &["left", "right"];
const LOOT_SOURCES: &[&str] = &["container", "inventory"];
const AXES: &[&str] = &["yaw", "pitch"];
const EQUIP_SLOTS: &[&str] = &[
    "weapon.mainhand", "weapon.offhand",
    "armor.head", "armor.chest", "armor.legs", "armor.feet",
];
/// Upstream FACES is public on the registry.
pub const FACES: &[&str] = &["north", "south", "east", "west", "up", "down", "any"];

fn def(
    ty: BlockType,
    category: BlockCategory,
    shape: BlockShape,
    label: &'static str,
    out_ports: i32,
    params: Vec<ParamSlot>,
) -> BlockDef {
    BlockDef { ty, category, shape, label, out_ports, params }
}

fn stmt(ty: BlockType, category: BlockCategory, label: &'static str, params: Vec<ParamSlot>) -> BlockDef {
    def(ty, category, BlockShape::Statement, label, 1, params)
}

fn action_block(ty: BlockType, label: &'static str, modes: &[&'static str]) -> BlockDef {
    def(
        ty,
        BlockCategory::Action,
        BlockShape::Statement,
        label,
        1,
        vec![
            ParamSlot::of_enum("mode", modes.to_vec(), "once"),
            ParamSlot::of_int("interval", 1),
            ParamSlot::of_int("ticks", 0),
        ],
    )
}

fn reporter(ty: BlockType, category: BlockCategory, label: &'static str, params: Vec<ParamSlot>) -> BlockDef {
    def(ty, category, BlockShape::Reporter, label, 0, params)
}

fn bool_block(ty: BlockType, category: BlockCategory, label: &'static str, params: Vec<ParamSlot>) -> BlockDef {
    def(ty, category, BlockShape::Boolean, label, 0, params)
}

fn enum_slot(name: &'static str, choices: &[&'static str], default: &'static str) -> ParamSlot {
    ParamSlot::of_enum(name, choices.to_vec(), default)
}

fn uuid_slot(name: &'static str, default: &'static str) -> ParamSlot {
    ParamSlot::of(name, ParamType::Uuid, default)
}

/// Upstream static { start(); register(...); ... } initializer, in order.
fn register_all() -> HashMap<BlockType, BlockDef> {
    let mut v: Vec<BlockDef> = Vec::new();

    // start()
    v.push(def(BlockType::Start, BlockCategory::Event, BlockShape::Hat, "start", 1, vec![]));

    v.push(def(BlockType::OnToggle, BlockCategory::Event, BlockShape::Hat, "when toggled true", 1,
        vec![ParamSlot::of_bool("condition", false)]));
    v.push(def(BlockType::OnMessage, BlockCategory::Event, BlockShape::Hat, "on message", 1, vec![]));
    v.push(reporter(BlockType::MsgText, BlockCategory::Event, "message", vec![]));

    v.push(stmt(BlockType::Move, BlockCategory::Motion, "walk",
        vec![enum_slot("direction", MOVE_DIRS, "forward")]));
    v.push(stmt(BlockType::Strafe, BlockCategory::Motion, "strafe",
        vec![enum_slot("direction", STRAFE_DIRS, "left")]));
    v.push(stmt(BlockType::StopMovement, BlockCategory::Motion, "stop movement", vec![]));
    v.push(stmt(BlockType::Sneak, BlockCategory::Motion, "set sneaking",
        vec![ParamSlot::of_bool("value", true)]));
    v.push(stmt(BlockType::Sprint, BlockCategory::Motion, "set sprinting",
        vec![ParamSlot::of_bool("value", true)]));
    v.push(stmt(BlockType::Autojump, BlockCategory::Motion, "set auto-jump",
        vec![ParamSlot::of_bool("value", true)]));

    v.push(action_block(BlockType::Use, "use", TWICE_MODES));
    v.push(action_block(BlockType::Swing, "swing", ACTION_MODES));
    v.push(action_block(BlockType::Jump, "jump", ACTION_MODES));
    v.push(stmt(BlockType::AttemptAutojump, BlockCategory::Action, "auto jump", vec![]));
    v.push(action_block(BlockType::Attack, "attack", TWICE_MODES));
    v.push(def(BlockType::DropItem, BlockCategory::Action, BlockShape::Statement, "drop", 1,
        vec![
            enum_slot("amount", DROP_AMOUNTS, "1"),
            enum_slot("mode", ACTION_MODES, "once"),
            ParamSlot::of_int("interval", 1),
            ParamSlot::of_int("ticks", 0),
        ]));
    v.push(action_block(BlockType::SwapHands, "swap hands", ACTION_MODES));
    v.push(stmt(BlockType::PickBlock, BlockCategory::Action, "pick block",
        vec![enum_slot("data", PICK_BLOCK_DATA, "no data")]));
    v.push(stmt(BlockType::SendMessage, BlockCategory::Action, "send message",
        vec![ParamSlot::of_string("message", "Hello World")]));
    v.push(stmt(BlockType::StopAction, BlockCategory::Action, "stop action",
        vec![enum_slot("action", ACTION_TYPES, "USE")]));
    v.push(stmt(BlockType::StopAll, BlockCategory::Action, "stop all actions", vec![]));
    v.push(stmt(BlockType::PlaceBlock, BlockCategory::Action, "place",
        vec![
            ParamSlot::of_pos("position"),
            enum_slot("face", FACES, "any"),
            ParamSlot::of_bool("force", false),
        ]));
    v.push(stmt(BlockType::BreakBlock, BlockCategory::Action, "mine",
        vec![ParamSlot::of_pos("position"), ParamSlot::of_bool("force", false)]));
    v.push(bool_block(BlockType::DoingAction, BlockCategory::Action, "doing action",
        vec![enum_slot("action", DOING_ACTIONS, "walking")]));

    v.push(stmt(BlockType::LookDirection, BlockCategory::Look, "look direction",
        vec![
            ParamSlot::of_rot("direction"),
            ParamSlot::of_double("yawOffset", 0.0),
            ParamSlot::of_double("pitchOffset", 0.0),
            ParamSlot::of_int("ticks", 0),
        ]));
    v.push(stmt(BlockType::LookCardinal, BlockCategory::Look, "look cardinal",
        vec![
            enum_slot("direction", DIRECTIONS, "north"),
            ParamSlot::of_double("yawOffset", 0.0),
            ParamSlot::of_double("pitchOffset", 0.0),
            ParamSlot::of_int("ticks", 0),
        ]));
    v.push(stmt(BlockType::LookAtPos, BlockCategory::Look, "look position",
        vec![
            ParamSlot::of_pos("position"),
            ParamSlot::of_double("yawOffset", 0.0),
            ParamSlot::of_double("pitchOffset", 0.0),
            ParamSlot::of_int("ticks", 0),
        ]));
    v.push(stmt(BlockType::LookAtEntity, BlockCategory::Look, "look entity",
        vec![
            ParamSlot::of_uuid("target"),
            enum_slot("mode", LOOK_AT_MODES, "eyes"),
            ParamSlot::of_double("yawOffset", 0.0),
            ParamSlot::of_double("pitchOffset", 0.0),
            ParamSlot::of_int("ticks", 0),
        ]));
    v.push(stmt(BlockType::Turn, BlockCategory::Look, "turn",
        vec![
            ParamSlot::of_double("dYaw", 0.0),
            ParamSlot::of_double("dPitch", 0.0),
            ParamSlot::of_double("yawOffset", 0.0),
            ParamSlot::of_double("pitchOffset", 0.0),
            ParamSlot::of_int("ticks", 0),
        ]));

    v.push(stmt(BlockType::GotoPos, BlockCategory::Path, "path pos",
        vec![ParamSlot::of_pos("position")]));
    v.push(stmt(BlockType::GotoEntity, BlockCategory::Path, "path entity",
        vec![ParamSlot::of_uuid("target")]));
    v.push(stmt(BlockType::StopPath, BlockCategory::Path, "stop pathing", vec![]));
    v.push(stmt(BlockType::PathSetting, BlockCategory::Path, "path setting",
        vec![enum_slot("key", PATH_SETTINGS, "horizontalMoveCost"), ParamSlot::of_string("value", "1.0")]));
    v.push(stmt(BlockType::PathMoveType, BlockCategory::Path, "path move type",
        vec![enum_slot("type", MOVE_TYPES, "WALK")]));
    v.push(bool_block(BlockType::InPath, BlockCategory::Path, "in path", vec![]));
    v.push(bool_block(BlockType::PathReached, BlockCategory::Path, "reached end", vec![]));
    v.push(bool_block(BlockType::PathFailed, BlockCategory::Path, "failed path", vec![]));

    v.push(stmt(BlockType::PlaySound, BlockCategory::Voice, "play sound",
        vec![ParamSlot::of_string("sound", "hello.wav"), ParamSlot::of_bool("loop", false)]));
    v.push(stmt(BlockType::StopSound, BlockCategory::Voice, "stop sound", vec![]));
    v.push(bool_block(BlockType::IsSpeaking, BlockCategory::Voice, "is speaking", vec![]));
    v.push(stmt(BlockType::Bluetooth, BlockCategory::Voice, "bluetooth",
        vec![ParamSlot::of_uuid("source")]));
    v.push(stmt(BlockType::StopBluetooth, BlockCategory::Voice, "stop bluetooth", vec![]));
    v.push(bool_block(BlockType::IsBluetoothed, BlockCategory::Voice, "is bluetoothed", vec![]));

    v.push(stmt(BlockType::SelectHotbar, BlockCategory::Inventory, "hotbar",
        vec![enum_slot("slot", HOTBAR_SLOTS, "1")]));
    v.push(stmt(BlockType::OpenInventory, BlockCategory::Inventory, "open inventory", vec![]));
    v.push(stmt(BlockType::CloseScreen, BlockCategory::Inventory, "close screen", vec![]));
    v.push(stmt(BlockType::Handedness, BlockCategory::Look, "handedness",
        vec![enum_slot("side", HANDEDNESS, "right")]));
    v.push(stmt(BlockType::InvClick, BlockCategory::Inventory, "menu click",
        vec![
            enum_slot("menu", MENUS, "inventory"),
            enum_slot("mode", CLICK_MODES, "click"),
            ParamSlot::of_int("slot", 0),
        ]));
    v.push(stmt(BlockType::InvSwapHotbar, BlockCategory::Inventory, "swap slot",
        vec![
            enum_slot("menu", MENUS, "inventory"),
            ParamSlot::of_int("slot", 0),
            enum_slot("with", SWAP_TARGETS, "1"),
        ]));
    v.push(stmt(BlockType::InvHeldThrow, BlockCategory::Inventory, "throw held",
        vec![enum_slot("menu", MENUS, "inventory")]));
    v.push(stmt(BlockType::InvHeldDrag, BlockCategory::Inventory, "drag held",
        vec![
            enum_slot("menu", MENUS, "inventory"),
            enum_slot("button", DRAG_BUTTONS, "left"),
            ParamSlot::of_string("slots", "1,2,3"),
        ]));
    v.push(stmt(BlockType::QuickLoot, BlockCategory::Inventory, "quick loot",
        vec![enum_slot("from", LOOT_SOURCES, "container"), ParamSlot::of_int("slot", 0)]));
    v.push(stmt(BlockType::RecipeBook, BlockCategory::Inventory, "place recipe",
        vec![
            ParamSlot::of_item("item", "minecraft:crafting_table"),
            enum_slot("mode", RECIPE_MODES, "single"),
        ]));
    v.push(reporter(BlockType::InventoryOpen, BlockCategory::Inventory, "menu open", vec![]));
    v.push(reporter(BlockType::ContainerSize, BlockCategory::Inventory, "container size", vec![]));
    v.push(stmt(BlockType::TradeSelect, BlockCategory::Inventory, "trade select",
        vec![ParamSlot::of_int("index", 1)]));
    v.push(stmt(BlockType::TradeRestock, BlockCategory::Inventory, "trade restock", vec![]));
    v.push(reporter(BlockType::TradeCheck, BlockCategory::Inventory, "trade check",
        vec![ParamSlot::of_int("index", 0)]));

    v.push(def(BlockType::If, BlockCategory::Control, BlockShape::Statement, "if", 2,
        vec![ParamSlot::of_bool("condition", false)]));
    v.push(def(BlockType::ElseIf, BlockCategory::Control, BlockShape::Statement, "else (if)", 2,
        vec![ParamSlot::of_bool("condition", true)]));
    v.push(def(BlockType::For, BlockCategory::Control, BlockShape::Statement, "for", 1,
        vec![ParamSlot::of_int("count", 10)]));
    v.push(def(BlockType::While, BlockCategory::Control, BlockShape::Statement, "while", 1,
        vec![ParamSlot::of_bool("condition", true)]));
    v.push(reporter(BlockType::LoopIter, BlockCategory::Control, "i", vec![]));
    v.push(def(BlockType::BlockEnd, BlockCategory::Control, BlockShape::CEnd, "end", 1, vec![]));
    v.push(def(BlockType::Break, BlockCategory::Control, BlockShape::Statement, "break", 0,
        vec![ParamSlot::of_int("count", 1)]));
    v.push(stmt(BlockType::Wait, BlockCategory::Control, "wait",
        vec![ParamSlot::of_int("ticks", 20)]));
    v.push(stmt(BlockType::WaitUntil, BlockCategory::Control, "wait until",
        vec![ParamSlot::of_bool("condition", false)]));
    v.push(def(BlockType::StopScript, BlockCategory::Event, BlockShape::Statement, "stop", 0, vec![]));
    v.push(stmt(BlockType::PauseScript, BlockCategory::Event, "pause", vec![]));
    v.push(def(BlockType::SetScript, BlockCategory::Event, BlockShape::Statement, "run script", 0,
        vec![ParamSlot::of_string("script", "")]));

    v.push(stmt(BlockType::SetVar, BlockCategory::Variable, "set",
        vec![ParamSlot::of_var_ref("name", ""), ParamSlot::of_double("value", 0.0)]));
    v.push(stmt(BlockType::ChangeVar, BlockCategory::Variable, "change",
        vec![ParamSlot::of_var_ref("name", ""), ParamSlot::of_double("delta", 1.0)]));
    v.push(reporter(BlockType::ReadVar, BlockCategory::Variable, "var",
        vec![ParamSlot::of_var_ref("name", "")]));

    v.push(reporter(BlockType::Scoreboard, BlockCategory::Sensor, "scoreboard",
        vec![ParamSlot::of_string("objective", "score"), uuid_slot("target", "@s")]));
    v.push(bool_block(BlockType::HasTag, BlockCategory::Sensor, "has tag",
        vec![uuid_slot("target", "@s"), ParamSlot::of_string("tag", "tag")]));
    v.push(reporter(BlockType::Health, BlockCategory::Sensor, "health", vec![]));
    v.push(reporter(BlockType::Position, BlockCategory::Sensor, "position", vec![]));
    v.push(reporter(BlockType::Yaw, BlockCategory::Sensor, "",
        vec![enum_slot("axis", AXES, "yaw")]));
    v.push(reporter(BlockType::BlockAt, BlockCategory::Sensor, "block at",
        vec![ParamSlot::of_pos("position")]));
    v.push(bool_block(BlockType::IsTouchingBlock, BlockCategory::Sensor, "is touching block",
        vec![ParamSlot::of_string("block", "minecraft:water")]));
    v.push(reporter(BlockType::DistanceTo, BlockCategory::Sensor, "distance from",
        vec![
            enum_slot("fromShape", DISTANCE_SHAPES, "position"),
            enum_slot("toShape", DISTANCE_SHAPES, "position"),
            ParamSlot::of_uuid("target"),
            enum_slot("mode", DISTANCE_MODES, "normal"),
        ]));
    v.push(reporter(BlockType::TimeOfDay, BlockCategory::Sensor, "time of day", vec![]));

    v.push(bool_block(BlockType::EveryXTicks, BlockCategory::Sensor, "every",
        vec![ParamSlot::of_int("ticks", 1)]));
    v.push(bool_block(BlockType::OnDamage, BlockCategory::Sensor, "on damage", vec![]));
    v.push(reporter(BlockType::AttackCooldown, BlockCategory::Sensor, "attack cooldown",
        vec![enum_slot("kind", COOLDOWN_KINDS, "fraction")]));
    v.push(reporter(BlockType::HurtTime, BlockCategory::Sensor, "hurt time", vec![]));
    v.push(reporter(BlockType::Ping, BlockCategory::Sensor, "ping", vec![]));

    v.push(reporter(BlockType::ItemInSlot, BlockCategory::Inventory, "inventorySlot",
        vec![ParamSlot::of_int("slot", 0)]));
    v.push(reporter(BlockType::Equipment, BlockCategory::Inventory, "equipment",
        vec![uuid_slot("target", "@s"), enum_slot("slot", EQUIP_SLOTS, "weapon.mainhand")]));
    v.push(reporter(BlockType::GetCount, BlockCategory::Inventory, "get",
        vec![
            enum_slot("kind", COUNT_KINDS, "count"),
            enum_slot("slot", &ITEM_SLOT_NAMES, "weapon.mainhand"),
        ]));
    v.push(reporter(BlockType::MaxCount, BlockCategory::Inventory, "max",
        vec![
            enum_slot("kind", COUNT_KINDS, "count"),
            ParamSlot::of_item("item", "minecraft:diamond_sword"),
        ]));
    v.push(reporter(BlockType::CommandResult, BlockCategory::Sensor, "command result",
        vec![ParamSlot::of_string("command", "data get entity @s Health")]));

    v.push(reporter(BlockType::NumCalc, BlockCategory::DataManipulation, "num calc",
        vec![ParamSlot::of_string("expression", "0")]));
    v.push(reporter(BlockType::StringCalc, BlockCategory::DataManipulation, "String calc",
        vec![ParamSlot::of_string("expression", "\"\"")]));
    v.push(bool_block(BlockType::BoolCalc, BlockCategory::DataManipulation, "bool calc",
        vec![ParamSlot::of_string("expression", "true")]));
    v.push(reporter(BlockType::PosCalc, BlockCategory::DataManipulation, "Pos calc",
        vec![ParamSlot::of_string("expression", "pos(0,0,0)")]));
    v.push(reporter(BlockType::DirCalc, BlockCategory::DataManipulation, "Dir calc",
        vec![ParamSlot::of_string("expression", "dir(0,0)")]));
    v.push(reporter(BlockType::Ternary, BlockCategory::Operator, "if",
        vec![
            ParamSlot::of_bool("condition", false),
            ParamSlot::of_int("trueValue", 0),
            ParamSlot::of_int("falseValue", 0),
        ]));
    v.push(bool_block(BlockType::Compare, BlockCategory::Operator, "compare",
        vec![
            ParamSlot::of_double("a", 0.0),
            enum_slot("op", COMPARATORS, "<"),
            ParamSlot::of_double("b", 0.0),
        ]));
    v.push(bool_block(BlockType::Equality, BlockCategory::Operator, "equals",
        vec![
            ParamSlot::of_double("a", 0.0),
            enum_slot("op", EQUALITY_OPS, "="),
            ParamSlot::of_double("b", 0.0),
        ]));
    v.push(bool_block(BlockType::Logic, BlockCategory::Operator, "logic",
        vec![
            ParamSlot::of_bool("a", false),
            enum_slot("op", LOGIC_OPS, "and"),
            ParamSlot::of_bool("b", false),
        ]));
    v.push(bool_block(BlockType::Not, BlockCategory::Operator, "not",
        vec![ParamSlot::of_bool("a", false)]));
    v.push(bool_block(BlockType::Contains, BlockCategory::Operator, "contains",
        vec![
            ParamSlot::of_string("a", ""),
            ParamSlot::of_string("b", ""),
            ParamSlot::of_bool("checkCase", false),
        ]));
    v.push(bool_block(BlockType::And, BlockCategory::Operator, "and",
        vec![ParamSlot::of_bool("a", false), ParamSlot::of_bool("b", false)]));
    v.push(bool_block(BlockType::Or, BlockCategory::Operator, "or",
        vec![ParamSlot::of_bool("a", false), ParamSlot::of_bool("b", false)]));
    v.push(reporter(BlockType::RandomInt, BlockCategory::Operator, "random int",
        vec![ParamSlot::of_int("min", 0), ParamSlot::of_int("max", 10)]));
    v.push(reporter(BlockType::RandomDouble, BlockCategory::Operator, "random double",
        vec![ParamSlot::of_double("min", 0.0), ParamSlot::of_double("max", 1.0)]));

    v.push(reporter(BlockType::Vec3, BlockCategory::DataManipulation, "make position",
        vec![
            ParamSlot::of_double("x", 0.0),
            ParamSlot::of_double("y", 0.0),
            ParamSlot::of_double("z", 0.0),
        ]));
    v.push(reporter(BlockType::Rot, BlockCategory::DataManipulation, "make direction",
        vec![ParamSlot::of_double("yaw", 0.0), ParamSlot::of_double("pitch", 0.0)]));
    v.push(reporter(BlockType::ToString, BlockCategory::DataManipulation, "toString",
        vec![ParamSlot::of_string("value", "")]));

    v.push(def(BlockType::FuncDefine, BlockCategory::Functions, BlockShape::Hat, "define", 1,
        vec![enum_slot("name", &[], "")]));
    v.push(stmt(BlockType::FuncCall, BlockCategory::Functions, "call",
        vec![enum_slot("name", &[], "")]));
    v.push(reporter(BlockType::FuncParam, BlockCategory::Functions, "input",
        vec![ParamSlot::of_string("func", ""), ParamSlot::of_int("index", 0)]));

    let mut map: HashMap<BlockType, BlockDef> = HashMap::new();
    for d in v {
        map.insert(d.ty, d);
    }
    map
}

/// Upstream BlockDefRegistry.DEFS.
pub fn defs() -> &'static HashMap<BlockType, BlockDef> {
    static DEFS: LazyLock<HashMap<BlockType, BlockDef>> = LazyLock::new(register_all);
    &DEFS
}

/// Upstream BlockDefRegistry.all(): unmodifiable list in enum order.
pub fn all() -> Vec<&'static BlockDef> {
    BlockType::ALL.iter().filter_map(|t| defs().get(t)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_registry::find_block_type;

    #[test]
    fn every_block_type_has_a_def() {
        assert_eq!(defs().len(), 113, "registry size matches upstream's 113 block types");
        for t in BlockType::ALL {
            assert!(defs().contains_key(t), "missing BlockDef for {}", t.json_name());
        }
    }

    #[test]
    fn spot_checks_match_upstream() {
        let wait = defs().get(&find_block_type("WAIT").unwrap()).unwrap();
        assert_eq!(wait.label, "wait");
        assert_eq!(wait.category, BlockCategory::Control);
        assert_eq!(wait.out_ports, 1);
        let ticks = wait.param_slot("ticks").unwrap();
        assert_eq!(ticks.ty, ParamType::Int);
        assert_eq!(ticks.default_value.as_deref(), Some("20"));

        let yaw = defs().get(&find_block_type("YAW").unwrap()).unwrap();
        assert_eq!(yaw.label, "");
        assert_eq!(yaw.shape, BlockShape::Reporter);

        let scoreboard = defs().get(&find_block_type("SCOREBOARD").unwrap()).unwrap();
        let target = scoreboard.param_slot("target").unwrap();
        assert_eq!(target.ty, ParamType::Uuid);
        assert_eq!(target.default_value.as_deref(), Some("@s"));

        let get_count = defs().get(&find_block_type("GET_COUNT").unwrap()).unwrap();
        let slot = get_count.param_slot("slot").unwrap();
        assert_eq!(slot.enum_choices, ITEM_SLOT_NAMES.to_vec());
        assert_eq!(slot.default_value.as_deref(), Some("weapon.mainhand"));

        let if_def = defs().get(&find_block_type("IF").unwrap()).unwrap();
        assert_eq!(if_def.out_ports, 2);
        assert_eq!(if_def.param_slot("condition").unwrap().default_value.as_deref(), Some("false"));

        let string_calc = defs().get(&find_block_type("STRING_CALC").unwrap()).unwrap();
        assert_eq!(string_calc.param_slot("expression").unwrap().default_value.as_deref(), Some("\"\""));

        let use_def = defs().get(&find_block_type("USE").unwrap()).unwrap();
        assert_eq!(use_def.param_slot("mode").unwrap().enum_choices, TWICE_MODES.to_vec());
        assert_eq!(use_def.param_slot("ticks").unwrap().default_value.as_deref(), Some("0"));

        let func_define = defs().get(&find_block_type("FUNC_DEFINE").unwrap()).unwrap();
        assert_eq!(func_define.shape, BlockShape::Hat);
        assert!(func_define.param_slot("name").unwrap().enum_choices.is_empty());

        let faces = defs().get(&find_block_type("PLACE_BLOCK").unwrap()).unwrap();
        assert_eq!(faces.param_slot("face").unwrap().enum_choices, FACES.to_vec());
    }
}
