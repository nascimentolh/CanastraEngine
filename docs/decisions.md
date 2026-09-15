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
- **Binary on disk** as `.cana`: `CANASTRA` magic, a `u32` format version, then postcard (serde).
  - *2026-09-14:* the schema hash is a frozen golden file instead: a sample with every nested type is
    compared byte for byte, so a layout change fails the tests until the version is bumped.
  - Readers reject other versions; migrations between versions are written when version 2 exists.
  - Asset paths are validated again on load, since content packs come from servers.
  - Uncompressed (31 MB for the H5 data; zlib gets 2.4 MB). Compression belongs to content-pack
    transport, added with it.
- **Typed IDs and references** (`ItemId`, `SkillId`, `TextureRef`, `MeshRef`) with referential
  integrity checked on save; a broken reference blocks the save.
- **Edited only through Canastra Studio**, never by hand.

### Migration policy (2026-09-14)

- **The server decides what exists and how it plays; the client supplies names and presentation.**
  Client-only entries are not playable and are left out, with a warning.
- **No silent loss:** every legacy client field is mapped or ignored under a stated reason (gameplay
  copy, unknown meaning, not modeled yet). An unaccounted field fails the migration. Server data
  without a place in the model is counted per item type.
- **Repairs are reported:** broken client data is fixed only where the intent is clear, and each fix
  is listed.
- **Presentation is independent of gameplay kind:** shields are armor held like a weapon, and some etc
  items are worn meshes.
- Deferred until their systems are designed: item conditions, extractable contents, on-crit and
  on-magic skills, item sets, quests.

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
- **Graphics backends** (wgpu, chosen automatically, overridable in settings): DX12 then Vulkan on
  Windows, Vulkan on Linux, Metal on macOS, Vulkan on Android.
  - *2026-09-14:* old Android devices are not supported: no OpenGL/GLES fallback, so the renderer is
    not limited to GLES 3 features.
- **Scale:** automatic by DPI plus a player slider.
- **Engine:** our own renderer on wgpu. egui is for tools only, never game UI.
- **Styling:** our own CSS subset: flex/grid layout (taffy), gradients, borders, radius, shadows,
  9-slice textures, light transitions (hover, open/close, cooldowns), which players can disable.
- **Art:** undecided. L2UI textures are read in place from the client packages; styled (CSS) chrome can
  replace them.
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
2. Texture reading (`.utx`), in place.
   - *2026-09-14:* textures are not converted or re-encoded. Parsed textures borrow the package bytes
     untouched, and pixels are decoded to RGBA in memory only when needed (as Fermata does).
3. Canastra Studio: game data editor.
4. UI engine: markup + CSS subset, taffy layout, wgpu renderer, text shaping (Cyrillic/CJK), bindings,
   compiler to binary.
5. **Login background scene (priority):** the animated 3D scene H5 shows behind the login screen,
   rendered by our client behind our own login UI. Only the rendering that scene needs comes first.
   - *2026-09-14:* decided before the UI binary compiler and before general world rendering. Fermata
     (Interlude) is a reference for how it loads the scene, but H5's login scene differs.
   - Findings and step plan: [`login-scene.md`](login-scene.md).
   - *2026-09-14:* accepted as it stands after a measured comparison with the H5 login screenshot
     (summed error 521 over twelve points); remaining differences are listed in `login-scene.md`.
6. World rendering basics (terrain, BSP, static meshes).
7. Protocol crate + server skeleton (login, character select, enter world, movement).
8. MVP windows and NPC dialog templates.
