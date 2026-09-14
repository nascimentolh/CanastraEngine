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

1. Public tagged-property reading in `ue2-assets` and level actor extraction (class, mesh, location,
   rotation, DrawScale/DrawScale3D, group, emitter lists).
2. Static meshes (`.usx`) with their textures, drawn from the fixed camera behind the login UI.
3. Materials the scene uses (shaders, panners, blending) so the sky and clouds animate.
4. Terrain.
5. Emitters: sprite particles.
6. Movers, sway and ambient sound as the comparison shows they matter.
