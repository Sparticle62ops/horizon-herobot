//! HeroBot for Horizon: the mod API wiring of the 1:1 port.
//!
//! The 1:1 logic lives in the sibling modules, untouched: the AI script
//! model (`ai`), the block and block-def registries, the bool, string,
//! vector and number expression evaluators, the sensor table, the hex
//! (deflate+hex framing) and JSON script codecs, and the runtime that
//! evaluates scripts over bots. The bot framework itself (BotPlayer,
//! pathing, the `/herobot` command) waits on the server-side fake-player
//! API Horizon does not have yet and lands behind it.
//!
//! The mod API wires the original's @Rule table (`settings::RULES`, 21
//! rules, same names, defaults, bounds and order as upstream) as
//! settings, so Horizon's Options > Mods page holds them with the same
//! defaults the game's `/herobot` command would read once the command
//! module lands.
use mod_api::{RegistrationContext, Registrar, SettingSpec};

pub mod ai;
pub mod block_def_registry;
pub mod block_registry;
pub mod bool_eval;
pub mod expr;
pub mod hex_codec;
pub mod runtime;
pub mod script_codec;
pub mod sensor;
pub mod settings;
pub mod str_eval;
pub mod vec_eval;

#[derive(Default)]
struct Mod;

/// The mod's id: its settings' namespace.
const MOD_ID: &str = "herobot";

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    /// The original's @Rule table registered as settings, same defaults
    /// and upstream order. Upstream bounds several rules only from below
    /// (the @Rule has a min but no max); those sliders take a display cap
    /// to stay drawable, noted per rule, while the rule's own bound is the
    /// min and the port's validation keeps upstream's semantics.
    fn register(&self, registrar: &mut dyn Registrar, _context: &RegistrationContext) {
        let cfg = settings::Settings::default();
        let label = |key: &str| format!("setting.herobot.{key}");
        let tip = |key: &str| format!("setting.herobot.{key}.tooltip");
        let reg = |r: &mut dyn Registrar, s: SettingSpec| {
            let _ = r.register_setting(s);
        };

        // Toggles, upstream order.
        for (key, value) in [
            ("creative_no_clip", cfg.creative_no_clip),
            ("allow_spawning_offline_players", cfg.allow_spawning_offline_players),
            ("allow_listing_bot_players", cfg.allow_listing_bot_players),
            ("bot_leave_on_death", cfg.bot_leave_on_death),
            ("shield_stunning", cfg.shield_stunning),
            ("kb_scaling", cfg.kb_scaling),
            ("explosion_no_fire", cfg.explosion_no_fire),
            ("wind_charge_no_trigger", cfg.wind_charge_no_trigger),
            ("editable_player_nbt", cfg.editable_player_nbt),
            ("clients_ignore_slow_tick_rate", cfg.clients_ignore_slow_tick_rate),
            ("xp_no_cooldown", cfg.xp_no_cooldown),
            ("no_projectile_random", cfg.no_projectile_random),
            ("delete_chunk_entities", cfg.delete_chunk_entities),
            ("disable_experimental_screen", cfg.disable_experimental_screen),
        ] {
            reg(
                registrar,
                SettingSpec::toggle(key, label(key), value).tooltip(tip(key)),
            );
        }

        // Numbers with an upstream min only: the cap is a display cap.
        reg(
            registrar,
            SettingSpec::range("creative_fly_speed", label("creative_fly_speed"), 0.0, 10.0, 0.1, cfg.creative_fly_speed)
                .tooltip(tip("creative_fly_speed")),
        );
        reg(
            registrar,
            SettingSpec::range("creative_fly_drag", label("creative_fly_drag"), 0.0, 1.0, 0.01, cfg.creative_fly_drag)
                .tooltip(tip("creative_fly_drag")),
        );
        reg(
            registrar,
            SettingSpec::range("bot_ping_to_ticks", label("bot_ping_to_ticks"), 1.0, 1000.0, 1.0, cfg.bot_ping_to_ticks as f64)
                .tooltip(tip("bot_ping_to_ticks")),
        );
        reg(
            registrar,
            SettingSpec::range("shield_stunning_window", label("shield_stunning_window"), 0.0, 100.0, 1.0, cfg.shield_stunning_window as f64)
                .tooltip(tip("shield_stunning_window")),
        );
        reg(
            registrar,
            SettingSpec::range("shield_delay_ticks", label("shield_delay_ticks"), 0.0, 100.0, 1.0, cfg.shield_delay_ticks as f64)
                .tooltip(tip("shield_delay_ticks")),
        );
        reg(
            registrar,
            SettingSpec::range("damage_ticks", label("damage_ticks"), 1.0, 100.0, 1.0, cfg.damage_ticks as f64)
                .tooltip(tip("damage_ticks")),
        );

        // The one upstream enum: True / False / Most, in declaration order.
        const EXPLOSION_MODES: [&str; 3] = [
            "setting.herobot.explosion_no_block_damage.true",
            "setting.herobot.explosion_no_block_damage.false",
            "setting.herobot.explosion_no_block_damage.most",
        ];
        let default_index = match cfg.explosion_no_block_damage {
            settings::ExplosionNoDmgMode::True => 0,
            settings::ExplosionNoDmgMode::False => 1,
            settings::ExplosionNoDmgMode::Most => 2,
        };
        reg(
            registrar,
            SettingSpec::choice("explosion_no_block_damage", label("explosion_no_block_damage"), EXPLOSION_MODES, default_index)
                .tooltip(tip("explosion_no_block_damage")),
        );
    }
}

// MOD_ID documents the settings' namespace; the registrar derives it from
// the package id, and the const keeps the two in one place.
const _: () = {
    let _ = MOD_ID;
};
