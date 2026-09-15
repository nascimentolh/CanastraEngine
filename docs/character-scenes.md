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
   *2026-09-15:* static meshes in world zones now use Fermata's hemisphere ambient and wrapped sun
   diffuse, fed by `StaticMeshAmbient` and `HSVStaticMeshLight` at the client's starting hour, with the sun
   along `NMovableSunLight0`. The HSV ramps follow Unreal 2's `FGetHSV`, whose saturation runs backwards:
   read as ordinary HSV, the daytime ramp turns everything red. At 22:00, the starting hour, the village
   houses take the warm light of the H5 screenshot, and the login is unchanged.
   *2026-09-15:* terrain in world zones. Each Lobby02 terrain sector stores eight intensity maps, like the
   eight visibility entries of mesh instances, one per three hours of the day. Three options were captured
   against the screenshot: Fermata's model on heightmap normals alone (a bright, warm brown ground), the
   stored map of the hour alone (the grass goes dark green), and both, with the stored map scaling the sun.
   Both comes closest to the screenshot's dark olive ground, with `TerrainAmbient` and `HSVTerrainLight` and
   the map of the hour's state. The order of the states is assumed; more screenshots at other
   hours would confirm it.
   *2026-09-15:* the sky, drawn from the client's own data. `Env.int` names the sky's materials in
   `L2_Skies.utx` (`SkybackgroundColor`, `HazeRing_Final`, cloud and star layers), which `TimeEnv` colors by
   the hour; the engine supplies their geometry. A dome now shades from `HazeringColor` at the horizon to
   `SkyBoxColor` above, under the wispy `Cloud_Final01_sh` layer in `CloudColor4`. Fermata's HDR panoramas
   were captured too, and its day panorama looked closest, but it is Fermata's image, not the client's, so
   the client's sky was kept.
   The hour: H5's lobby clock starts at 22:00 and runs at six times real time, so a screenshot can show any
   hour. At 22:00 the sky turns pink; at 18:00 to 20:00 the houses lose their warm light. 21:00 gives both
   the blue sky and the warm light of the screenshot, and world zones are now shown at that hour.
   *2026-09-15:* the hall's BSP. The level's `Model` (lobby01's `Model5`) is read up to its vertex pool,
   following Lineage2JS's description of the format, and its visible polygons draw textured along their
   surface's axes. From `Char_Select_Warp` the hall now shows its carpet, the rune circle, the stairs and
   the throne as in the H5 screenshot. All 208 maps of the client still read; one had nodes pointing
   nowhere, which are left out. Still to do: BSP light maps (the floor and walls draw at full
   brightness), the torch flames, and the wider framing of H5's select camera, which the scene may move
   after its warp.
3. **Skinned rendering in the client.** Draw one part in its bind pose, then a whole body playing its
   lobby idle sequence, with GPU skinning.
   *2026-09-15:* done, on the CPU. A Human Fighter's default body (`Chargrp` record 0: face, both hair
   meshes, gloves, upper, lower and boots, each with its texture) stands in the first `Logongrp` slot and
   loops `Wait_Hand_MFighter`. Findings:
   - Parts are placed without `MeshOrigin`: scaled, turned by `RotOrigin`, then by the pawn's yaw at the
     slot. Subtracting or adding the origin sinks or lifts the body by its pelvis height.
   - Unreal Engine 2 stores every bone rotation conjugated except the root's. Of the four conventions
     captured, only that one gives the idle pose with the arms at the sides, as H5's characters stand.
   - Hair meshes name no animation, yet share the body's skeleton; every part plays the body's sequence,
     or the hair stays in the bind pose over a moving head.
   - The lobby now loads the hall from `Char_Select_Warp` when the character list shows.
4. **Bodies as game data.** Migrate `Chargrp`, `Logongrp` and `Charcreategrp` into the game data: the
   meshes a body uses by race, sex and archetype, and the scene's pawn slots. Equipment models come from
   items, which already carry them.
   *2026-09-15:* done, game data format 3. `GameData.bodies` gives each of the 16 bodies its faces (one mesh
   with three textures), hair styles and bare gloves, upper, lower and boots; `GameData.lobby` gives the
   eight select slots and the 20 characters on display at creation with their gear. Findings:
   - `Chargrp` orders bodies Human fighter, Dark Elf, Dwarf, Elf, Human mystic, Orc fighter, Orc mystic,
     Kamael, male then female; a seventeenth record is empty. Elves and Dark Elves share one body per sex
     across fighter and mystic classes.
   - Its hair table holds five blocks of fifteen styles: the first is hair without headgear (front and
     back meshes), the second the back hair alone under helmets, the rest hair under other headgear.
     Only the first is kept. The client has no hair color textures; how H5 colors hair is still open.
   - `Charcreategrp` holds four characters per race for Human, Elf, Dark Elf and Orc, then two for Dwarf and
     Kamael. The last select slot, in front of the camera, is the selected character's.
5. **Select scene.** Move the camera to `Char_Select_Warp` and stand the account's characters on the
   eight slots, wearing what they wear. Clicking a model selects it, and the UI shows name, class and level.
   *2026-09-15:* the account's characters stand in the hall in their bare bodies, dressed from
   `GameData.bodies` by their class line's race and archetype, their sex, face and hair style, looping
   `Wait_Hand`. The selected character takes the last slot and the others the first seven in list order, as
   in H5: the two side characters of the H5 screenshot fall where the camera puts slots 0 and 1, to the
   pixel. The selected one does not: H5 stands it about 65 units behind the last slot, on the rune circle's
   far edge, which both its size and its feet agree on. The list moved to the right so the middle stays
   clear. Still to do: worn gear, names above heads, and picking by clicking the model.
6. **Creation scene.** Fly to the race's camera, show the class's model in its display gear, and switch
   the model when the sex, hair style, hair color or face changes, as H5 does.
   *2026-09-15:* the creation screen flies to Lobby02's scene for the chosen class's race (Human until one is
   picked) and stands that race's display characters from `Charcreategrp` in their display armor. Findings:
   - An armor's worn model gives each mesh the texture at its index; full armor brings upper and lower
     meshes, which replace the bare chest and legs. Held weapons still wait for bone attachments.
   - Characters in world zones take the `ActorAmbient` and `HSVActorLight` ramps on their skinned normals;
     unlit, the armor looked silver where H5's is warm gold.
   - The root bone keeps its bind rotation. Its animation keys turn female idles away: with them, the female
     elves showed their backs; conjugated, every Human did. Without them all 20 face the camera and the Human
     poses match the H5 screenshot, three-quarter turns included.
   *2026-09-15:* vegetation, which looked neon and jagged next to H5's dark olive. Three causes:
   - Trees store no vertex colors, and meshes without them drew at full brightness without the hour's light.
     In world zones every mesh now takes the hour's light over whatever it stores.
   - Terrain decorations added the static mesh daylight over the terrain intensity they already carried.
     They now take the terrain's own light where they stand, as the grass in the screenshot does.
   - Leaf materials with `AlphaRef` 0 kept their fully clear texels, which showed as grey cards. Unreal's
     alpha test passes only texels above the reference. Masked edges now follow Fermata: 4× MSAA with the
     alpha turned into sample coverage, sharpened to about a pixel, so leaves and grass come out smooth.
   *2026-09-15:* the trees still looked like solid green sheets. Their leaf materials are Shaders with an Opacity
   map, `AlphaTest` and `AlphaRef` 1, which were read as cut-outs. Cut at 1, box-filtered mips fill whole leaf
   cards in (the client's stored mips are filtered the same way). Cut at half, the leaves turn lacy but the
   login's bushes, the same kind of shader, vanish. A Shader with an Opacity map blends, and its alpha test
   only keeps clear texels out of the depth buffer: read that way, a capture at the screenshot's resolution
   shows the Talking Island trees with H5's shapes, and the login keeps its bushes. What still differs is the
   whole scene's warmer grade in H5, not the trees.
   *2026-09-15:* the creation panel, redesigned: Race, Class and Gender are combos, and Hairstyle, Hair Color
   and Face are steppers, over a box with the class's attributes. The race moves the camera, the class lists
   only that race's, and the display character of the chosen archetype and sex wears the face and hair
   picked. `canastra-ui` gained `<select bind options action>`: a row with the value that opens its options
   as an overlay, drawn after everything and hit first. The limits of five male and seven female hair styles,
   four colors and three faces now live in the protocol, shared by client and server. Still to do: hair color
   on the model, held weapons, and the camera closing in on the chosen model as H5 does.
   *2026-09-16:* held weapons and shields. What the client and Fermata show, not guessed:
   - The client's `Engine.u` gives `Pawn` its bones by default properties: `RightHandBone=Weapon_R_Bone`,
     `LeftHandBone=Weapon_L_Bone`, `LeftArmBone=Shield_L_Bone`, `RootBone=bip01`. The attaching itself is native
     code in `Engine.dll`, whose code section is packed and cannot be read statically.
   - Fermata's assembly (`char_pawn`) picks the bone from candidate lists by slot (`Weapon_R_Bone`, `Sword Bone`,
     `Weapon Bone`, `Bow Bone`, `Bip01 R Hand`; shields `Shield_L_Bone` first; bows in the left hand), attaches
     with an identity offset, and builds its bind skeleton conjugating only the root, with its matrices the
     transpose of ours: the same skeleton.
   - Weapons are one-bone `SkeletalMesh`es in `LineageWeapons.ukx`, drawn in the bone's frame. A character holding
     one plays the idle of its grip (`Wait_1HS`, `Wait_2HS`, `Wait_Pole`, `Wait_Bow`, `Wait_Dual`).
   - `Weapongrp`'s scales and offsets belong to the enchant effect and the rough range mesh, not to the weapon.
   - Weapon materials are a `FinalBlend` alpha blend with `AlphaTest` at 120 to 160 over a `Shader`: the alpha
     test now applies to alpha blends, and a `Shader` without an Opacity map no longer blends by its diffuse alpha,
     which made them see-through.
   - Still to do: some weapons point differently from H5.
   *2026-09-16:* frame time. Rewriting every material's uniform each frame took 17 ms in lobby01 and 55 ms in
   Lobby02; only panning and rotating materials are rewritten now. Creation went from 8 to about 40 fps, the
   select hall to 60.
   *2026-09-16:* the other races, checked against H5 screenshots of each. Their cameras and display stands were
   right: projecting the stands through each race's warp at the same field of view puts heads and feet where the
   screenshots have them. What differed was the Kamael, and the fix comes from the data:
   - Armor rows list extra meshes (`m_Kamael_add`): the wings and a skirt layer, each with the texture at its
     index. They are skinned meshes with skeletons and animations of their own (`Wing_Mkamael`,
     `SkirtA_Mkamael`) whose sequences share the body's names, so each part now plays its own animation when it
     names one, and the body's `<Body>_anim` otherwise.
   - A Kamael armor lists more textures than meshes (`_t84_u`, `_t84_l`, `_t84_ut`); textures past the meshes
     dress the further sections of the mesh whose texture their name extends.
   - `CANASTRA_AUTOLOGIN=account:password` logs in at start, so captures no longer type into the window.
   *2026-09-16:* the elves' bodies, checked against the H5 screenshot:
   - Every part is drawn with the scale and rotation of the body, the part naming `<Body>_anim` (the face):
     parts share one instance, and some armor meshes store no scale or rotation, which tilted mystic robes.
   - Bones a part shares with the body take the body's pose, and back hair that is a chain of its own
     (`FElf_m000_m00_bh`, Hair01 to Hair13) hangs from the body's `Bip01_Head`.
   - The root bone plays its keys. Idle roots differ by up to 100 degrees of yaw (`Wait_Hand_FElf` 57 to 68,
     `Wait_1HS_MElf` 154), and the pelvis keys make up for it: with the root's keys the head faces the same way
     in every idle, and with its bind rotation it did not. Stances and weapons now match the screenshot.
   - Still to do: H5 turns each head toward the camera (`Pawn.bFaceRotation`, `LastNeckRot` in `Engine.u`).
   *2026-09-16:* heads and hair, all races. Faces and hair (`_f`, `_ah`, most `_bh`) are rigid meshes bound
   whole to the root bone, with no influences, yet modeled around the head of the bind pose: the client carries
   them with the head bone (`Pawn.HeadBone`), so they now hang from `Bip01_Head`. Before, they stood with the
   root while the head moved with the idle, which bent necks and left hair behind. Bone names match whether
   they separate words with spaces or underscores (`Bip01 Head`), as Orc and Dwarf skeletons do.
   - Not done: turning heads toward the camera. H5 does not; its dark elf mystic keeps his back to the camera.
   - Lobby02 holds a scene per chosen class and gender (`Elf_Knight_Kman`, `Elf_Kman_Kwoman`) and a chest
     close-up (`Elf_Kman_Chest`), each ActionMoveCamera moves along interpolation points; Fermata offers the
     same with a rotate and zoom strip after gender selection. Still to build.
   *2026-09-16:* the creation camera. Lobby02's scenes hold camera moves as well as warps: each ActionMoveCamera
   names an interpolation point, a duration and, with `PathStyle` 1, a Bezier path through the handles around
   its ends (`StartControlPoint` leaving a point, `EndControlPoint` arriving at it). The client reads every
   scene's shots and flies between the views H5 shows: the race (`Elf`), then a class's pair once the class is
   picked (`Elf_Knight`, `Elf_Wizard`), then one character once the gender is (`Elf_Knight_Kman`), a cut when
   the gender changes (`Elf_Kman_Kwoman`) and a close-up on the chest behind a + button (`Elf_Kman_Chest`,
   played back to zoom out). Views with no scene between them cut to where the view's own scene ends. Race,
   class and gender now start unchosen, as in H5.
   - Level, particle and pawn vertices stay relative to where the map was loaded; a moving camera only shifts
     the view matrix.
   - Still to do: the camera's pace along each path (even in time here) and rotating the character.
   *2026-09-16:* the select screen, redesigned as a carousel: a card per character along the bottom with a
   Create card last, Enter World above them, and the selected character's name, class and level in a panel
   beside them. Clicking a card or the character in the hall selects it and stands it in the middle. Delete
   asks first, on a screen of its own over the same hall. Names over heads are left out where a panel covers
   them, since text draws over every shape. The UI gained `:checked` (an element whose `checked` binding holds
   `true`, as the selected card) and percent offsets for absolute elements.
   - Enter World only says it is not ready yet: the game server has no world to enter.
   *2026-09-16:* the chosen character's controls. Once a class and gender are picked, a strip under the character
   turns them left, closes in or out, and turns them right, with Lucide icons (a 2 KB subset of the ISC-licensed
   font in `assets/ui/fonts`). Holding a turn button turns the character a quarter turn a second while it walks in
   place with the client's own `Walk_*` sequence, whose root stays put; the turn eases in and out and the walk
   blends with the idle over a quarter second, so quick clicks do not snap the pose. The UI gained `show`, which
   leaves an element out unless its binding holds `true`.
   - Still to measure: H5's turning pace.
