# H5 character select and creation scenes

The target is the High Five lobby as the client shows it: the characters' 3D models standing in lobby01,
and a creation scene that flies to each race. Findings come from the test client's files (2026-09-15).

## What the client shows

H5 screenshots (2026-09-15) set the target:

- **Select:** a stone hall with a long carpet, stairs on both sides and a throne with torches. The
  selected character stands in the middle of the carpet on a rune circle, and the others stand to the
  left. Each shows its name above the head. The panel at the top left shows name, level, class, HP, MP,
  SP, karma and experience. Create, Delete and Re-Login are at the right, and Start at the bottom.
- **Creation:** for Human, a village of Talking Island with a windmill, and four models in display gear:
  a male and a female fighter, then a male and a female mystic. The panel at the left picks name, race,
  class, gender, hair style, hair color and face above a description, with Create and Previous at the right.

Our screens keep the project's own UI look; the 3D scenes follow H5.

## What the client holds

- **Maps:** select uses `MAPS/lobby01.unr`, the login map, and creation uses `MAPS/Lobby02.unr`. Lobby02
  holds a small world for creation: 1208 static meshes and a scene per race, Kamael included.
- **Cameras** (SceneManagers):
  - lobby01 `Char_Select_Warp` at (150890, -246952, -8053), fog #51777b from 600 to 10000: the hall.
  - Lobby02 `Char_Create_Warp` and `Human` at (181810, -247791, -6370), fog #aeaa95 from 1500 to 10000:
    the village.
  - Lobby02 has one scene per race (`Human`, `Elf`, `DarkElf`, `orc`, `Dwarf`, `Kamael`), plus transitions
    between fighter and mystic (`K`/`W`) and between male and female: `<Race>_Kman_Kwoman`,
    `<Race>_Wman_Wwoman` and their reverses. lobby01 holds an older copy of the same scenes without the
    village around them.
- **Select pawns:** `system/Logongrp.dat` has eight slots, each with x, y, z and yaw, around
  (150900, -246650, -8117).
- **Creation pawns:** `system/Charcreategrp.dat` has 20 records, each with a position, yaw and the item
  ids worn for display (chest, legs, gloves, feet, right and left hand).
- **Bodies:** `system/Chargrp.dat` has 17 records, one per race, sex and archetype. Each lists hair meshes
  and textures, face meshes and textures, and the default gloves, upper, lower and boots worn when
  nothing is equipped.
- **Models:** `Animations/<Line>.ukx` packages (`Fighter`, `Elf`, `DarkElf`, `Orc`, `Dwarf`, `Kamael`,
  and `Magic` or `Shaman` for mystics) hold:
  - one `SkeletalMesh` per body part, named `<M|F><Line>_m<set>_<part>`, where the part is `u` upper,
    `l` lower, `g` gloves, `b` boots or `h` head, with hair variants;
  - one `MeshAnimation` per body, `MFighter_anim` or `FFighter_anim`.

## Plan

Each step is checked against the real client: a dump of the files, or a screenshot of the H5 lobby.

1. **Skeletal meshes and animations in `ue2-assets`.** Read `SkeletalMesh` (vertices, wedges, faces,
   materials, reference skeleton, weights) and `MeshAnimation` (bones, sequences, tracks), with a
   `canastra` command that prints a mesh's counts and bone names.
   *2026-09-15:* done. `canastra mesh` reads every skeletal mesh and animation in the client's own packages
   to their last byte: 7318 meshes and 1129 animations. The 3887 objects in custom packages saved with
   other package versions are skipped. Parts store their first level of detail either as Lineage soft
   and rigid streams or in the older wedge layout, and both are read. Animations without bone indices
   give one track per bone, in order. The lobby idle is `Wait_Hand_<Body>`.
2. **Scene fidelity for both maps.** Our renderer already frames both scenes from their cameras, but a
   capture shows what it still lacks. The hall shows its walls, stairs and torches almost black, with no
   floor: its floor and walls are BSP, which is not drawn yet, and it needs the level's lighting. In the
   village, trees, grass, paths and the windmill draw, but the houses are black and there is no sky.
   *2026-09-15:* why the village differs from the login. lobby01's zones set `bUseZoneState`, and their
   meshes carry precomputed colors for the zone's state. Lobby02's zones are ordinary world zones: the
   houses' stored colors are black, and H5 lights them by the hour instead.
   - **Lighting:** `system/TimeEnv0.int` has hourly ramps for ambient light (`StaticMeshAmbient`,
     `BSPAmbient`, `TerrainAmbient`, `ActorAmbient`) and the sun's light in HSV (`HSVStaticMeshLight` and
     the rest). Adding the static mesh ambient alone brings the houses back with their textures.
   - **Sun:** after its colors, each `StaticMeshInstance` stores a revision and eight entries of one
     visibility bit per vertex. These are likely the sun's shadowing, which is not decoded yet.
     `NMovableSunLight0` gives the sun's rotation.
   - **Sky:** Lobby02's `SkyZoneInfo` holds only the sun and moon sprites and moon planes; no sky mesh
     exists. The sky is drawn from the `TimeEnv` sky, haze and cloud color ramps.
   - **Hour:** `system/Env.int` starts the clock at 22, but the H5 creation screenshot shows late
     daylight, so the lobby's hour is still to be found.
   - **Fermata:** it lights these zones with its own improved model, driven by the same time of day:
     hemisphere ambient, sun with sky exposure, and local lights. Try it against the screenshot too.
3. **Skinned rendering in the client.** Draw one part in its bind pose, then a whole body playing its
   lobby idle sequence, with GPU skinning.
4. **Bodies as game data.** Migrate `Chargrp`, `Logongrp` and `Charcreategrp` into the game data: the
   meshes a body uses by race, sex and archetype, and the scene's pawn slots. Equipment models come from
   items, which already carry them.
5. **Select scene.** Move the camera to `Char_Select_Warp` and stand the account's characters on the
   eight slots, wearing what they wear. Clicking a model selects it, and the UI shows name, class and level.
6. **Creation scene.** Fly to the race's camera, show the class's model in its display gear, and switch
   the model when the sex, hair style, hair color or face changes, as H5 does.
