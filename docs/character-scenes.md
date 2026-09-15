# H5 character select and creation scenes

The target is the High Five lobby as the client shows it: the characters' 3D models standing in lobby01,
and a creation scene that flies to each race. Findings come from the test client's files (2026-09-15).

## What the client holds

- **Cameras** (SceneManagers in `MAPS/lobby01.unr`):
  - `Char_Select_Warp` at (150890, -246952, -8053), fog #51777b from 600 to 10000.
  - `Char_Create_Warp`, the same point as `Logon_Warp`.
  - One per race (`human`, `Elf`, `Darkelf`, `orc`, `Dwarf`), plus transitions between fighter and
    mystic (`K`/`W`) and between male and female: `<Race>_Kman_Kwoman`, `<Race>_Wman_Wwoman` and their
    reverses.
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
2. **Skinned rendering in the client.** Draw one part in its bind pose, then a whole body playing its
   lobby idle sequence, with GPU skinning.
3. **Bodies as game data.** Migrate `Chargrp`, `Logongrp` and `Charcreategrp` into the game data: the
   meshes a body uses by race, sex and archetype, and the scene's pawn slots. Equipment models come from
   items, which already carry them.
4. **Select scene.** Move the camera to `Char_Select_Warp` and stand the account's characters on the
   eight slots, wearing what they wear. Clicking a model selects it, and the UI shows name, class and level.
5. **Creation scene.** Fly to the race's camera, show the class's model in its display gear, and switch
   the model when the sex, hair style, hair color or face changes, as H5 does.
