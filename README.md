# HeroBot (Horizon port)

Server-side framework for player bots that can fight, plus a block-based
AI editor. Upstream: HerobaneNair/HeRoBot (Modrinth herobot, 1.78M
downloads), license CC-BY-NC-4.0 (LICENSE.txt copied verbatim).

## Upstream layout

- herobot-common, 52 files: the AI script engine, a block-based script
  language: BlockDef registry, expressions, runtime variables, branches.
- herobot-mod, 181 files: the server-side bot framework - BotPlayer,
  pathing with placement and stepping, remote control, networking
  payloads - plus the client half: AI editor screen, movement recorder,
  skins.
- herobot-plugin, 89 files: a Paper/Velocity plugin backend for the
  same framework.

## Port status

- settings.rs: HeroBotSettings + Rule registry ported 1:1, same names,
  defaults, bounds and descriptions, ordered as upstream. The /herobot
  command wiring lands with the server command module.
- ai.rs: the AI script model ported and rustc-clean: VarType,
  VarDecl, FuncDecl, Comment with per-char style flags, Wire,
  BlockInstance, AiScript graph with the nextId allocator. AiHexCodec
  (deflate+hex framing) is next; the horizon workspace already depends
  on flate2 so the codec ports to it. AiScriptCodec JSON follows.
- Bot framework, BotPlayer and pathing: BLOCKED on Horizon features,
  see below. Porting resumes behind them.

## Horizon feature gaps, to file on horizon-rust/horizon

1. Server-side fake players. HeroBot's core is spawning and driving
   player entities with near-client parity: auto-jump, spear, knockback
   scaled by per-bot ping. Horizon's server mod API, hooks 8-15, has
   commands, menus, block entities, particles and movement guards but
   no way to spawn, tick or animate a fake player entity. Needed: a
   native hook to create and despawn a player entity owned by a mod,
   tick callbacks for it, and a way to apply movement and input to it.
2. Client-parity physics hooks for the bot: hurt and knockback
   overrides, shield stun windows, damage-tick overrides, so the
   settings above actually bind.

Issue text drafted. Filing is blocked until the GitHub token is
renewed: push and the gh API both fail on the expired one.

## License

CC-BY-NC-4.0: this port is a derivative for non-commercial use with
attribution. NOT listable in the horizon-mods registry without Danil's
grant decision, since registry policy only auto-lists MIT, BSD, Apache,
ISC, zlib, Unlicense and CC0. Listing is Danil's call.
