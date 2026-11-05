// HeroBot settings/rules ported 1:1 from HerobaneNair/HeRoBot
// (herobot-common/.../rule/HeroBotSettings.java, CC-BY-NC-4.0).
// Java @Rule annotations become a static registry table: same names,
// defaults, bounds and descriptions, ordered as upstream lists them.
// The /herobot command reads this table via native_register_commands
// (server hooks 8): /herobot <setting> shows value+description,
// /herobot <setting> <value> sets for the session,
// /herobot <setting> <value> <perm> sets and saves.

pub enum RuleType { Bool, Int, Float, Enum }

pub struct Rule {
    pub name: &'static str,
    pub desc: &'static str,
    pub ty: RuleType,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

// Values live in a separate mutable table so the command layer can read
// and write them generically (Java used static fields + reflection).
#[derive(Clone, Copy, PartialEq)]
pub enum RuleValue {
    Bool(bool),
    Int(i64),
    Float(f64),
}

#[derive(Clone, Copy, PartialEq)]
pub enum ExplosionNoDmgMode { True, False, Most }

impl ExplosionNoDmgMode {
    pub fn enabled(self) -> bool { self != ExplosionNoDmgMode::False }
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "TRUE" => Some(Self::True),
            "FALSE" => Some(Self::False),
            "MOST" => Some(Self::Most),
            _ => None,
        }
    }
}

pub struct Settings {
    pub creative_no_clip: bool,
    pub creative_fly_speed: f64,
    pub creative_fly_drag: f64,
    pub allow_spawning_offline_players: bool,
    pub allow_listing_bot_players: bool,
    pub bot_ping_to_ticks: i64,
    pub bot_leave_on_death: bool,
    pub shield_stunning: bool,
    pub shield_stunning_window: i64,
    pub shield_delay_ticks: i64,
    pub kb_scaling: bool,
    pub damage_ticks: i64,
    pub explosion_no_block_damage: ExplosionNoDmgMode,
    pub explosion_no_fire: bool,
    pub wind_charge_no_trigger: bool,
    pub editable_player_nbt: bool,
    pub clients_ignore_slow_tick_rate: bool,
    pub xp_no_cooldown: bool,
    pub no_projectile_random: bool,
    pub delete_chunk_entities: bool,
    pub disable_experimental_screen: bool,
    pub server_has_herobot: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            creative_no_clip: false,
            creative_fly_speed: 1.0,
            creative_fly_drag: 0.09,
            allow_spawning_offline_players: true,
            allow_listing_bot_players: true,
            bot_ping_to_ticks: 25,
            bot_leave_on_death: false,
            shield_stunning: false,
            shield_stunning_window: 3,
            shield_delay_ticks: 5,
            kb_scaling: true,
            damage_ticks: 10,
            explosion_no_block_damage: ExplosionNoDmgMode::False,
            explosion_no_fire: false,
            wind_charge_no_trigger: false,
            editable_player_nbt: false,
            clients_ignore_slow_tick_rate: false,
            xp_no_cooldown: false,
            no_projectile_random: false,
            delete_chunk_entities: false,
            disable_experimental_screen: false,
            server_has_herobot: true,
        }
    }
}

impl Settings {
    // Upstream: damageInvulnerableTicks() = damageTicks * 2
    pub fn damage_invulnerable_ticks(&self) -> i64 { self.damage_ticks * 2 }

    pub fn rules() -> &'static [Rule] {
        &RULES
    }
}

pub static RULES: &[Rule] = &[
    Rule { name: "creativeNoClip", desc: "Creative No Clip, allows to client player to phase through blocks", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "creativeFlySpeed", desc: "Changes creative flying speed multiplier (Default 1.0), how quickly the client flies", ty: RuleType::Float, min: Some(0.0), max: None },
    Rule { name: "creativeFlyDrag", desc: "Changes creative air drag (Default 0.09), how quickly the air stops the client while flying", ty: RuleType::Float, min: Some(0.0), max: Some(1.0) },
    Rule { name: "allowSpawningOfflinePlayers", desc: "Spawn offline players in online mode if online-mode player with specified name does not exist", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "allowListingBotPlayers", desc: "Allows listing bot players on the multiplayer screen", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "botPingToTicks", desc: "Change the ping to tick conversion for Bot Players (default 25)", ty: RuleType::Int, min: Some(1.0), max: None },
    Rule { name: "botLeaveOnDeath", desc: "Bots disconnect on death rather than respawning", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "shieldStunning", desc: "Enables shield stunning, where the shielding player can be damaged immediately after the shield is disabled", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "shieldStunningWindow", desc: "Makes shield stuns use decreased [paper] kb during the window, default 5", ty: RuleType::Int, min: Some(0.0), max: None },
    Rule { name: "shieldDelayTicks", desc: "Change the delay of bringing up the shield", ty: RuleType::Int, min: Some(0.0), max: None },
    Rule { name: "kbScaling", desc: "Disable knockback scaling, as in entities will take the same vertical knockback regardless of their previous vertical velocity", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "damageTicks", desc: "Change how many ticks the hurt animation/red flash and damage cooldown last when any living entity is damaged (default 10)", ty: RuleType::Int, min: Some(1.0), max: None },
    Rule { name: "explosionNoBlockDamage", desc: "Explosions won't destroy blocks", ty: RuleType::Enum, min: None, max: None },
    Rule { name: "explosionNoFire", desc: "Allows intentional game design explosions (from beds and respawn anchors) to not explode with fire", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "windChargeNoTrigger", desc: "Wind Charges won't activate redstone blocks", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "editablePlayerNbt", desc: "Enables editing player nbt, so you can directly edit values within a player's data", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "clientsIgnoreSlowTickRate", desc: "If true, makes client players ignore slower tick rates", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "xpNoCooldown", desc: "Players absorb XP instantly, without delay", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "noProjectileRandom", desc: "Removes randomness from projectiles while true", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "deleteChunkEntities", desc: "Chunk Resetting deletes entities within that chunk", ty: RuleType::Bool, min: None, max: None },
    Rule { name: "disableExperimentalScreen", desc: "Remove the experimental world setting (if on client disables all, if on world, only disables it on this world)", ty: RuleType::Bool, min: None, max: None },
];
