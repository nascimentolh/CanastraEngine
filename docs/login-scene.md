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
   missing, and edge turns and terrain lighting are not modeled.
5. Emitters: sprite particles.
6. Movers, sway and ambient sound as the comparison shows they matter.
