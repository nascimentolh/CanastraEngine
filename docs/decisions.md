# Architecture decisions

Decided on 2026-09-14 through a design interview. Change an entry only with a new dated note.

## Scope

- CanastraEngine is a High Five based client **and** a server rewritten from scratch, in one Rust
  workspace sharing domain, data and protocol crates.
- Existing server projects (`l2journey`, `private_server`) are reference material only: code may be
  copied from them, they are never modified.
- The legacy `system` folder goes away. Assets (maps, meshes, textures, sounds) are read from
  packages; `system/*.dat` tables migrate to Canastra's own format.

## Game data

- **Domain model, not a mirror of `.dat` files.** `Item` merges itemname + weapongrp/armorgrp/etcitemgrp,
  `Skill` merges skillname + skillgrp + skillsoundgrp, `Npc` merges npcname + npcgrp. Migration does
  the merge once.
- **Rust structs are the single source of truth.** Client, server and Studio use the same crate.
- **Binary on disk**, with magic, format version, schema hash and versioned migrations.
- **Typed IDs and references** (`ItemId`, `SkillId`, `TextureRef`, `MeshRef`) with referential
  integrity checked on save; a broken reference blocks the save.
- **Edited only through Canastra Studio**, never by hand.

## Server and extensibility

- **Rust, same workspace.**
- **Own protocol from day one:** typed, versioned messages in a shared crate. No H5 wire compatibility.
- **Extension without forks:** core in crates; heavy mechanics through Rust plugins (stable traits and
  events); content (quests, events, simple AI) through sandboxed scripts with hot reload; data
  through Studio.
- **Custom content reaches players as a signed content pack** (data + new assets) that the
  launcher/client downloads and caches per version. One client serves any server.
- **Custom mechanics get UI without client forks:** servers declare custom message schemas and
  windows in their skin pack; the client validates them and binds fields to widgets. No arbitrary
  code runs on the client.

## UI

- **Look:** H5 remaster. Layout and visual language kept, modernized (sharp at high resolution, better
  typography).
- **Platforms:** Windows, Linux, macOS, Android. No gamepad support planned.
- **Scale:** automatic by DPI plus a player slider.
- **Engine:** our own renderer on wgpu. egui is for tools only, never game UI.
- **Styling:** our own CSS subset: flex/grid layout (taffy), gradients, borders, radius, shadows,
  9-slice textures, light transitions (hover, open/close, cooldowns), which players can disable.
- **Art:** undecided. L2UI textures are available through migration into our own atlases; styled
  (CSS) chrome can replace them.
- **Source:** markup + CSS as text, editable by hand or in Studio (which reads and writes the same
  text). The build validates it and compiles it to binary for the client.
- **Widgets are addressed by name**, never by index. Bindings are declared (`bind="player.adena"`),
  and a missing binding is a build error.
- **Logic:** declarative UI with behavior in Rust. Scripting may come later without a rewrite.
- **Who customizes:** the project team and server owners.
- **Delivery:** the default UI ships in the client build; servers may send a signed skin pack
  (theme tokens and whole-window replacements, validated against available bindings and actions).
- **Windows:** move, lock and resize; layouts saved per character on the server; switchable HUD presets.
- **Chat:** channel tabs the player configures, clickable item/skill links, history and search,
  inline icons/emojis (rich text layout).
- **Anti-automation:** bindings only read state; actions require real player input; the server
  validates everything. Official in-game macros stay, server-controlled.
- **Languages at launch:** Portuguese (BR), English, Spanish, Russian, CJK. Text shaping, line
  breaking and glyph atlases must handle Cyrillic and CJK from the start.

## NPC dialogs

- **New format, no legacy HTML.** Dialog templates live in the UI pack; the server sends a dialog id,
  variables and the allowed actions. No markup travels over the wire, and translation happens in the
  client.
- **No fallback** for unconverted HTML; community board, rankings and similar screens become native
  windows.

## Tooling

- **Canastra Studio:** a separate egui app for game data and UI, with live hot-reload preview into a
  running client, and fixtures (mock states) for previewing windows.

## Licensing

- Project license: undecided.
- Layouts derived from GPL L2ClientDat descriptors are isolated in `l2-dat-h5`, used only by migration
  tooling, so every license option stays open.

## MVP

Login, then server and character selection, then walking in the world, with a basic HUD
(HP/MP/CP, target, chat, shortcuts) and NPC dialogs.

## Build order

1. Domain model crate + binary format + `canastra migrate` (dat to binary), verified against the client.
2. Texture decoding (`.utx`) and conversion into our own atlases.
3. Canastra Studio: game data editor.
4. UI engine: markup + CSS subset, taffy layout, wgpu renderer, text shaping (Cyrillic/CJK), bindings,
   compiler to binary.
5. World rendering basics (terrain, BSP, static meshes).
6. Protocol crate + server skeleton (login, character select, enter world, movement).
7. MVP windows and NPC dialog templates.
