# H5 login background scene

What the High Five client shows behind the login window, found by reading the test client's files
(2026-09-14). This is the target for the client's first 3D rendering.

## Where it lives

- **Map:** `MAPS/lobby01.unr`. It holds the login scene and character creation/selection.
  `MAPS/Lobby02.unr` is an older lobby that this client does not use: `system/logongrp.dat` places the
  eight character-select pawns around (150900, -246650, -8117), inside lobby01's terrain
  (TerrainInfo at 147456, -245760) and outside Lobby02's (180224, -245760).
- **Which map the client loads is not in plain text** (not in `L2.ini`, the `.u` scripts,
  `interface.xdat` or the DLLs); `l2.exe` decides natively. `L2.ini` has `Map=Index.unr`,
  `LocalMap=Entry.unr` (a 7 KB empty level) and `SharedSky=True`.
- Fermata's Interlude client uses a single `Lobby.unr` plus `skylevel.unr`; H5 differs.

## Camera

- The SceneManager tagged `Logon_Warp` (group `7thLobby`) runs one `ActionWarp` to
  `InterpolationPoint6`: location (148959.11, -252862.58, -5817.08), rotation pitch 2200, yaw -42971.
- The camera does not move during the login screen. The other SceneManagers are character select
  (`Char_Select_Warp` → `InterpolationPoint60`) and creation transitions (`<Race>_<Class>_<Class>`).

## What is in view

In front of the camera (under 30 000 units, within 60°): 130 StaticMeshActors, 46 Emitters,
181 Lights, 52 Brushes, 11 L2MovableStaticMeshActors, 23 AmbientSoundObjects and the terrain.

- **Sky and moon:** `L2_Lobby.BloodLobbySky_S` (DrawScale3D 5) and `L2_Lobby.BloodLobbyMoon_S`, the only
  meshes in group `7thLobby`. The level's `NSun`/`NMoon` (`L2_Skies`) belong to the shared sky zone.
- **Tree:** 13 pieces `L2_Lobby.BTree_*` at (147000, -247340, -5684), about 5 900 units ahead.
- **Foreground particles:** Emitters 3, 2 and 14 (up to 7 SpriteEmitters) within 1 000 units of the camera.
- **Hills:** lobby01's terrain. Other meshes nearby come from `Freya_S`, `FX_E_S`,
  `Schtgart_Fielddeco1_S` and `Castle_Kent_s`.

## Rendering plan, each step checked against a screenshot of the H5 login

1. Public tagged-property reading in `ue2-assets` and level actor extraction in `ue2-level` (class,
   mesh, location, rotation, scale, groups, skins, scene warps). **Done:** `canastra level` reads 616
   actors and 62 distinct meshes from lobby01 and finds `Logon_Warp` at the camera above.
2. Static meshes (`.usx`) with their textures, drawn from the fixed camera behind the login UI.
   **Done:** the client draws lobby01's 62 meshes (138 264 vertices, 99 445 triangles, 132 textures,
   about 0.4 s to load) with each material's base texture. The H5 screenshot puts the moon and the
   tree where a 50° horizontal field of view does. Still missing, for step 3: blending, so the
   opaque star layers of `BloodLobbySky_S` hide its purple sky and the moon shows only its outline.
3. Materials the scene uses (shaders, panners, blending) so the sky and clouds animate.
   **Done:** `l2-catalog` reduces Texture, Shader, FinalBlend, ColorModifier, Combiner (select,
   multiply, add) and TexPanner/TexScaler/TexRotator chains to one or two texture stages with a tint,
   a blend mode (opaque, masked, alpha, additive, modulate, brighten) and time-based UV transforms.
   The sky's two panned layers multiply ×4 in gamma space, the stars add over it and the moon blends,
   and the sky moves. Still different from H5: colors are more saturated and orange low in the sky
   (zone fog is not applied), the hills are missing (terrain), and Shader self-illumination, masks,
   oscillators and distance sorting of blended batches are not modeled.
4. Terrain. **Done:** `ue2-level` reads each TerrainInfo's heightmap, scale, painted layers and quad
   visibility bitmap; the client builds one vertex per G16 sample (world = location + scale × (sample
   offset from the center, height − 32768 over 256)), leaves holes out and draws the bottom layer
   opaque with the others blended by their alpha maps' red channel. The tree's base lands within 50
   units of the ground this computes. Textures now upload with box-filtered mips, which removed the
   ground's shimmer. The hills and the tree on its hill match the H5 screenshot; zone fog is still
   missing, and edge turns and terrain lighting are not modeled. **Decorations:** the terrain's two
   DecoLayers scatter `L2_Lobby.L2lobb1grass` and `L2lobb2grass` over the quads their density maps
   paint. Each quad gets `MaxPerQuad` chances, each taken with the map's weight times
   `DensityMultiplier` read as a percentage, which gives the sparse tufts along the H5 horizon (the
   plain factor saturates into a wall of bushes). Placements take a random point in the quad, the
   interpolated height, a random yaw and scale, and the terrain's intensity; they draw as static
   meshes whose opacity falls from 1 to 0 between the two `FadeoutRadius` distances, as Fermata fades
   them; masked grass thins out as its alpha drops below the cutoff. The grass shaders cut out by `AlphaTest` and `AlphaRef`,
   which materials now honor.
5. Emitters: sprite particles. **Done, after zone fog:** the camera's zone fog (#a8afbf from 200 to
   50000 units) fades surfaces by view distance, towards its color for opaque and alpha surfaces and
   towards the neutral value for the others. `ue2-level` reads each Emitter's SpriteEmitters (counts,
   lifetimes, start boxes, velocities, acceleration, size and color curves, fades, spin, texture cells,
   draw style, depth test, fogging, projection plane) with Unreal's defaults; the client simulates them
   on the CPU starting warmed up, respawns them as they die, and draws them over the level. Only the
   camera zone's emitters draw, since other zones are closed off. Styles that ignore alpha fade through
   the vertex color, and modulate and darken output gamma-space factors so mid gray stays neutral.
   The purple haze over the hills and around the tree now matches the H5 screenshot. Particles now
   revolve (`UseRevolution`), and sprites laid in a plane take Unreal's axes as Fermata recovered
   them. Stacked on one spot, SpriteEmitter18's twelve `eva_effect_map15` sprites had drawn an
   edge-on vertical beam that H5 does not show. Not modeled: sphere and polar start shapes, the emitter's DrawScale (scaling sizes by it blew the
   clouds up to screen size), BSP portal visibility and particle depth sorting.
   **Blending, checked against Fermata:** the scene now writes and blends gamma-space colors through a
   non-sRGB view of the window, as the original client's framebuffer did; linear blending had crushed
   the dim tails of mist, stars and the moon glow. Translucent is Unreal's screen blend (One,
   OneMinusSrcColor), modulate fades and fogs towards mid gray (white doubled what lies behind), and
   translucent and brighten surfaces are tinted by fog instead of fading to black. A Shader's
   self-illumination now adds over its diffuse where the diffuse is opaque, so `Lobby_Moon_s` shows
   its panning `OpenEyeMoon_myst_bitmap2` glow through `Bloodmoon_deco`'s crater mask as in H5. The
   H5 moon is still pinker than ours, with a dimmer symbol.
   **Soft particles:** H5 sprites with `UseSoftParticle` (the large lobby clouds, 450–1500 units) fade
   out over half their mean size in front of the geometry behind them. Level geometry draws first and
   stores its depth; particles then draw in a pass that only reads it. This removed the hard straight
   edges those clouds cut into the hills. Fermata ignores this flag. The flat ground mist
   (`ZTest=false`, not soft) still leaves a faint band along the ground.
   **Measured blending of alpha sprites:** H5's haze lifts the dark hills yet leaves the bright horizon
   and moon at full brightness (254 red), which plain alpha blending cannot do. Drawn as
   `(SrcAlpha, One)`, alpha-blended sprites bring the horizon to (253, 104, 117) against H5's
   (254, 109, 119) and the summed error over twelve sampled points from 811 to 555. This is a
   measured choice, not the client's recovered blend state. Particle systems now also draw back to
   front from the camera.
6. Movers, sway and ambient sound as the comparison shows they matter.

## Lighting

- **Precomputed lighting, drawn now:** each StaticMeshActor's `StaticMeshInstance` holds one RGBA color
  per mesh vertex (serialized red first) after its empty property list, and each TerrainSector holds
  its quads, offset and a count of intensity maps, one byte per sector vertex each (8 in lobby01,
  found by shape after variable L2 data). The login camera's zone is in state `CurZoneState = 2`,
  whose maps are the darkest, matching the dark hills of the H5 login. `bUnlit` actors (sky, moon)
  stay at full brightness; movers have no stored lighting and draw unlit for now.
- **Cloud color, drawn now:** `l2-env` reads `system/Env.int` (the clock starts at 22h) and the
  hourly ramps of `TimeEnv0.int`. Sprites with `UseCloudColor` (the haze, clouds and screen wash of
  lobby01) are tinted by `SkyBoxColor` at the start hour, (98, 107, 159). Measured against the H5
  screenshot, this turned the brown haze over the tree and hills into its blue-gray: hills
  (31, 29, 32) against H5's (32, 30, 36), trunk (30, 29, 36) against (43, 47, 54). `CloudColor1` and
  `HazeringColor` both left it brown. The moon and the bright horizon stay darker than H5, which the
  moon shader's missing self-illumination explains, not the tint.
- **Time of day, next:** `system/Env.int` starts the clock at 22h with 8 terrain shadow maps and 8
  actor light sets per day, and `TimeEnv0..3.int` give hourly ambient colors and HSV lights for
  terrain, static meshes, actors and BSP, plus sky, cloud and haze colors. Fermata reads the same
  palette as an atmospheric light probe and adds hemisphere ambient, a directional sun, rim light,
  specular and local lights on top; Canastra follows that path after the faithful baseline.
