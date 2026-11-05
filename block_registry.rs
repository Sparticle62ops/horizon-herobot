// HeroBot AI block registry core ported 1:1 from HerobaneNair/HeRoBot
// (herobot-common/.../ai/block: BlockType, BlockCategory, BlockShape,
// BlockDef, ParamSlot, ParamType), CC-BY-NC-4.0.
// Java records/enums become structs; the 358-line BlockDefRegistry
// (one BlockDef per type with params) ports next.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ParamType { Boolean, Int, Double, String, Position, Rotation, Uuid, VarRef, Enum, BlockState, Item }

#[derive(Clone, Debug)]
pub struct ParamSlot {
    pub name: &'static str,
    pub ty: ParamType,
    pub default_value: Option<String>,
    pub enum_choices: Vec<&'static str>,
}

impl ParamSlot {
    pub fn of(name: &'static str, ty: ParamType, default_value: impl Into<String>) -> Self {
        Self { name, ty, default_value: Some(default_value.into()), enum_choices: Vec::new() }
    }
    pub fn of_bool(name: &'static str, d: bool) -> Self { Self::of(name, ParamType::Boolean, d.to_string()) }
    pub fn of_int(name: &'static str, d: i64) -> Self { Self::of(name, ParamType::Int, d.to_string()) }
    pub fn of_double(name: &'static str, d: f64) -> Self { Self::of(name, ParamType::Double, d.to_string()) }
    pub fn of_string(name: &'static str, d: &str) -> Self { Self::of(name, ParamType::String, d) }
    pub fn of_enum(name: &'static str, choices: Vec<&'static str>, d: &str) -> Self {
        Self { name, ty: ParamType::Enum, default_value: Some(d.to_string()), enum_choices: choices }
    }
    pub fn of_var_ref(name: &'static str, d: &str) -> Self { Self::of(name, ParamType::VarRef, d) }
    pub fn of_uuid(name: &'static str) -> Self {
        Self::of(name, ParamType::Uuid, "@e[limit=1,sort=nearest,isSelf=false]")
    }
    pub fn of_pos(name: &'static str) -> Self { Self::of(name, ParamType::Position, "~ ~ ~") }
    pub fn of_rot(name: &'static str) -> Self { Self::of(name, ParamType::Rotation, "~ ~") }
    pub fn of_item(name: &'static str, d: &str) -> Self { Self::of(name, ParamType::Item, d) }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockShape { Hat, Statement, CShape, CEnd, Reporter, Boolean }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockCategory {
    Event, Action, Motion, Path, Look, Voice, Control, Variable,
    Operator, Inventory, DataManipulation, Sensor, Functions,
}

impl BlockCategory {
    /// Upstream display text and argb color, verbatim.
    pub fn display(self) -> &'static str {
        match self {
            Self::Event => "EVENT", Self::Action => "ACTION", Self::Motion => "MOTION",
            Self::Path => "PATH", Self::Look => "LOOK", Self::Voice => "VOICE",
            Self::Control => "CONTROL", Self::Variable => "VARIABLE",
            Self::Operator => "OPERATOR", Self::Inventory => "INVENTORY",
            Self::DataManipulation => "DATA MANIP", Self::Sensor => "SENSOR",
            Self::Functions => "FUNCTIONS",
        }
    }
    pub fn color(self) -> u32 {
        match self {
            Self::Event => 0xFFEEBB22, Self::Action => 0xFFFFAA11, Self::Motion => 0xFF4499FF,
            Self::Path => 0xFF55CC55, Self::Look => 0xFF55BBDD, Self::Voice => 0xFFE0556E,
            Self::Control => 0xFFFFBB00, Self::Variable => 0xFFFF8811,
            Self::Operator => 0xFF44BB44, Self::Inventory => 0xFFCC66CC,
            Self::DataManipulation => 0xFF11AAAA, Self::Sensor => 0xFF55BBDD,
            Self::Functions => 0xFF9B6BEF,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BlockType {
    Start, OnToggle, OnMessage, MsgText,
    Move, Strafe, StopMovement, Sneak, Sprint, Autojump,
    Use, Swing, Jump, AttemptAutojump, Attack, DropItem, SwapHands,
    PickBlock, SendMessage, StopAction, StopAll,
    LookDirection, LookCardinal, LookAtPos, LookAtEntity, Turn,
    GotoPos, GotoEntity, StopPath, PathSetting, PathMoveType, InPath, PathReached, PathFailed,
    PlaySound, StopSound, IsSpeaking, Bluetooth, StopBluetooth, IsBluetoothed,
    SelectHotbar, OpenInventory, CloseScreen, Handedness, InvClick, InvSwapHotbar,
    InvHeldThrow, InvHeldDrag, QuickLoot, RecipeBook, InventoryOpen, ContainerSize,
    TradeSelect, TradeRestock, TradeCheck,
    If, ElseIf, For, While, LoopIter, BlockEnd, Break, Wait, WaitUntil,
    StopScript, PauseScript, SetScript,
    SetVar, ChangeVar, ReadVar,
    Scoreboard, HasTag, Health, Position, Yaw, BlockAt, IsTouchingBlock,
    DistanceTo, TimeOfDay, EveryXTicks, OnDamage, AttackCooldown, HurtTime,
    Ping, ItemInSlot, Equipment, GetCount, MaxCount, CommandResult,
    Compare, Equality, Logic, And, Or, Not, Contains, RandomInt, RandomDouble,
    BoolCalc, NumCalc, DirCalc, PosCalc, StringCalc, Vec3, Rot, ToString,
    Ternary, PlaceBlock, BreakBlock, DoingAction,
    FuncDefine, FuncCall, FuncParam,
}

impl BlockType {
    /// Registry index order is the upstream enum declaration order.
    pub const ALL: &'static [BlockType] = &[BlockType::Start, BlockType::OnToggle, BlockType::OnMessage, BlockType::MsgText, BlockType::Move, BlockType::Strafe, BlockType::StopMovement, BlockType::Sneak, BlockType::Sprint, BlockType::Autojump, BlockType::Use, BlockType::Swing, BlockType::Jump, BlockType::AttemptAutojump, BlockType::Attack, BlockType::DropItem, BlockType::SwapHands, BlockType::PickBlock, BlockType::SendMessage, BlockType::StopAction, BlockType::StopAll, BlockType::LookDirection, BlockType::LookCardinal, BlockType::LookAtPos, BlockType::LookAtEntity, BlockType::Turn, BlockType::GotoPos, BlockType::GotoEntity, BlockType::StopPath, BlockType::PathSetting, BlockType::PathMoveType, BlockType::InPath, BlockType::PathReached, BlockType::PathFailed, BlockType::PlaySound, BlockType::StopSound, BlockType::IsSpeaking, BlockType::Bluetooth, BlockType::StopBluetooth, BlockType::IsBluetoothed, BlockType::SelectHotbar, BlockType::OpenInventory, BlockType::CloseScreen, BlockType::Handedness, BlockType::InvClick, BlockType::InvSwapHotbar, BlockType::InvHeldThrow, BlockType::InvHeldDrag, BlockType::QuickLoot, BlockType::RecipeBook, BlockType::InventoryOpen, BlockType::ContainerSize, BlockType::TradeSelect, BlockType::TradeRestock, BlockType::TradeCheck, BlockType::If, BlockType::ElseIf, BlockType::For, BlockType::While, BlockType::LoopIter, BlockType::BlockEnd, BlockType::Break, BlockType::Wait, BlockType::WaitUntil, BlockType::StopScript, BlockType::PauseScript, BlockType::SetScript, BlockType::SetVar, BlockType::ChangeVar, BlockType::ReadVar, BlockType::Scoreboard, BlockType::HasTag, BlockType::Health, BlockType::Position, BlockType::Yaw, BlockType::BlockAt, BlockType::IsTouchingBlock, BlockType::DistanceTo, BlockType::TimeOfDay, BlockType::EveryXTicks, BlockType::OnDamage, BlockType::AttackCooldown, BlockType::HurtTime, BlockType::Ping, BlockType::ItemInSlot, BlockType::Equipment, BlockType::GetCount, BlockType::MaxCount, BlockType::CommandResult, BlockType::Compare, BlockType::Equality, BlockType::Logic, BlockType::And, BlockType::Or, BlockType::Not, BlockType::Contains, BlockType::RandomInt, BlockType::RandomDouble, BlockType::BoolCalc, BlockType::NumCalc, BlockType::DirCalc, BlockType::PosCalc, BlockType::StringCalc, BlockType::Vec3, BlockType::Rot, BlockType::ToString, BlockType::Ternary, BlockType::PlaceBlock, BlockType::BreakBlock, BlockType::DoingAction, BlockType::FuncDefine, BlockType::FuncCall, BlockType::FuncParam,];
    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0) as i32
    }
    pub fn from_index(i: i32) -> BlockType {
        Self::ALL.get(i.max(0) as usize).copied().unwrap_or(BlockType::Start)
    }
}

/// Upstream BlockType.valueOf by json name.
pub fn find_block_type(json_name: &str) -> Option<BlockType> {
    BlockType::ALL.iter().copied().find(|t| t.json_name() == json_name)
}

impl BlockType {
    /// Upstream Wire-JSON name: the Java enum name in SCREAMING_SNAKE.
    pub fn json_name(self) -> &'static str {
        let s = match self {
            Self::Start => "START", Self::OnToggle => "ON_TOGGLE", Self::OnMessage => "ON_MESSAGE",
            Self::MsgText => "MSG_TEXT", Self::Move => "MOVE", Self::Strafe => "STRAFE",
            Self::StopMovement => "STOP_MOVEMENT", Self::Sneak => "SNEAK", Self::Sprint => "SPRINT",
            Self::Autojump => "AUTOJUMP", Self::Use => "USE", Self::Swing => "SWING",
            Self::Jump => "JUMP", Self::AttemptAutojump => "ATTEMPT_AUTOJUMP",
            Self::Attack => "ATTACK", Self::DropItem => "DROP_ITEM",
            Self::SwapHands => "SWAP_HANDS", Self::PickBlock => "PICK_BLOCK",
            Self::SendMessage => "SEND_MESSAGE", Self::StopAction => "STOP_ACTION",
            Self::StopAll => "STOP_ALL", Self::LookDirection => "LOOK_DIRECTION",
            Self::LookCardinal => "LOOK_CARDINAL", Self::LookAtPos => "LOOK_AT_POS",
            Self::LookAtEntity => "LOOK_AT_ENTITY", Self::Turn => "TURN",
            Self::GotoPos => "GOTO_POS", Self::GotoEntity => "GOTO_ENTITY",
            Self::StopPath => "STOP_PATH", Self::PathSetting => "PATH_SETTING",
            Self::PathMoveType => "PATH_MOVE_TYPE", Self::InPath => "IN_PATH",
            Self::PathReached => "PATH_REACHED", Self::PathFailed => "PATH_FAILED",
            Self::PlaySound => "PLAY_SOUND", Self::StopSound => "STOP_SOUND",
            Self::IsSpeaking => "IS_SPEAKING", Self::Bluetooth => "BLUETOOTH",
            Self::StopBluetooth => "STOP_BLUETOOTH", Self::IsBluetoothed => "IS_BLUETOOTHED",
            Self::SelectHotbar => "SELECT_HOTBAR", Self::OpenInventory => "OPEN_INVENTORY",
            Self::CloseScreen => "CLOSE_SCREEN", Self::Handedness => "HANDEDNESS",
            Self::InvClick => "INV_CLICK", Self::InvSwapHotbar => "INV_SWAP_HOTBAR",
            Self::InvHeldThrow => "INV_HELD_THROW", Self::InvHeldDrag => "INV_HELD_DRAG",
            Self::QuickLoot => "QUICK_LOOT", Self::RecipeBook => "RECIPE_BOOK",
            Self::InventoryOpen => "INVENTORY_OPEN", Self::ContainerSize => "CONTAINER_SIZE",
            Self::TradeSelect => "TRADE_SELECT", Self::TradeRestock => "TRADE_RESTOCK",
            Self::TradeCheck => "TRADE_CHECK", Self::If => "IF", Self::ElseIf => "ELSE_IF",
            Self::For => "FOR", Self::While => "WHILE", Self::LoopIter => "LOOP_ITER",
            Self::BlockEnd => "BLOCK_END", Self::Break => "BREAK", Self::Wait => "WAIT",
            Self::WaitUntil => "WAIT_UNTIL", Self::StopScript => "STOP_SCRIPT",
            Self::PauseScript => "PAUSE_SCRIPT", Self::SetScript => "SET_SCRIPT",
            Self::SetVar => "SET_VAR", Self::ChangeVar => "CHANGE_VAR", Self::ReadVar => "READ_VAR",
            Self::Scoreboard => "SCOREBOARD", Self::HasTag => "HAS_TAG", Self::Health => "HEALTH",
            Self::Position => "POSITION", Self::Yaw => "YAW", Self::BlockAt => "BLOCK_AT",
            Self::IsTouchingBlock => "IS_TOUCHING_BLOCK", Self::DistanceTo => "DISTANCE_TO",
            Self::TimeOfDay => "TIME_OF_DAY", Self::EveryXTicks => "EVERY_X_TICKS",
            Self::OnDamage => "ON_DAMAGE", Self::AttackCooldown => "ATTACK_COOLDOWN",
            Self::HurtTime => "HURT_TIME", Self::Ping => "PING",
            Self::ItemInSlot => "ITEM_IN_SLOT", Self::Equipment => "EQUIPMENT",
            Self::GetCount => "GET_COUNT", Self::MaxCount => "MAX_COUNT",
            Self::CommandResult => "COMMAND_RESULT", Self::Compare => "COMPARE",
            Self::Equality => "EQUALITY", Self::Logic => "LOGIC", Self::And => "AND",
            Self::Or => "OR", Self::Not => "NOT", Self::Contains => "CONTAINS",
            Self::RandomInt => "RANDOM_INT", Self::RandomDouble => "RANDOM_DOUBLE",
            Self::BoolCalc => "BOOL_CALC", Self::NumCalc => "NUM_CALC",
            Self::DirCalc => "DIR_CALC", Self::PosCalc => "POS_CALC",
            Self::StringCalc => "STRING_CALC", Self::Vec3 => "VEC3", Self::Rot => "ROT",
            Self::ToString => "TO_STRING", Self::Ternary => "TERNARY",
            Self::PlaceBlock => "PLACE_BLOCK", Self::BreakBlock => "BREAK_BLOCK",
            Self::DoingAction => "DOING_ACTION", Self::FuncDefine => "FUNC_DEFINE",
            Self::FuncCall => "FUNC_CALL", Self::FuncParam => "FUNC_PARAM",
        };
        s
    }

    /// Upstream OWNER_REFS: LoopIter, FuncParam, MsgText.
    pub fn refs_owner(self) -> bool {
        matches!(self, Self::LoopIter | Self::FuncParam | Self::MsgText)
    }
}

#[derive(Clone, Debug)]
pub struct BlockDef {
    pub ty: BlockType,
    pub category: BlockCategory,
    pub shape: BlockShape,
    pub label: &'static str,
    pub out_ports: i32,
    pub params: Vec<ParamSlot>,
}

impl BlockDef {
    pub fn has_input(&self) -> bool {
        !matches!(self.shape, BlockShape::Hat | BlockShape::Reporter | BlockShape::Boolean)
    }
    pub fn is_reporter(&self) -> bool {
        matches!(self.shape, BlockShape::Reporter | BlockShape::Boolean)
    }
    pub fn param_slot(&self, name: &str) -> Option<&ParamSlot> {
        self.params.iter().find(|p| p.name == name)
    }
}

/// Upstream BlockDefRegistry.get. Upstream throws IllegalArgumentException
/// on an unknown type; here an unknown type resolves to None and callers
/// fall back gracefully, like upstream on a null def.
pub fn def_for(ty: BlockType) -> Option<&'static BlockDef> {
    crate::block_def_registry::defs().get(&ty)
}
