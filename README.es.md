[English](README.md) | [Português (Brasil)](README.pt-BR.md) | Español

<p align="center"><img src="logo.png" alt="Canastra Engine" width="560"></p>

# CanastraEngine

Cliente y servidores de Lineage 2 High Five en Rust. Lee los assets directamente de una instalación existente
del cliente H5; las tablas de `system` (`.dat`) se están migrando al formato de datos propio de Canastra.
Consulta [`docs/decisions.md`](docs/decisions.md) para ver la arquitectura acordada y el orden de construcción.

<p align="center"><img src="docs/screenshots/01-login.png" alt="Pantalla de login" width="720"></p>

- **Pruébalo:** [docs/es/getting-started.md](docs/es/getting-started.md) instala Rust y ejecuta los
  servidores, el cliente y Studio.
- **Míralo:** [docs/screenshots](docs/screenshots) muestra las pantallas que ya funcionan.
- **Ayuda a construirlo:** lee [CONTRIBUTING.es.md](CONTRIBUTING.es.md) y el
  [código de conducta](CODE_OF_CONDUCT.es.md).

## Arquitectura

Las dependencias solo apuntan hacia abajo. Los crates de formato son puros (entra `&[u8]`, salen tipos, sin
IO), así que se pueden probar con bytes sintéticos y reutilizar en herramientas.

```
apps/        canastra-client     game client: winit + wgpu window drawing the UI (login screen for now)
             canastra-studio     game data editor (egui): validation blocks saving, undo, client icons, class kits
             canastra-cli        IO, filesystem, user-facing commands
             canastra-login      login server: accounts, rate-limited auth, game server registry, tickets
             canastra-game       game server: admits players by ticket, lists, creates and deletes their characters
crates/      canastra-migrate    legacy client tables + server XML -> game data, with a report
             ue2-assets          tagged properties, textures, palettes and static meshes read in place
             ue2-level           placed actors and scene cameras of a map
             canastra-data       game data domain model shared by client, server and Studio
             canastra-protocol   network messages per phase and direction (postcard, no IO)
             canastra-net        Noise-encrypted framed connections (tokio) and signed admission tickets
             canastra-db         PostgreSQL (sqlx) migrations and accounts with Argon2id passwords
             canastra-ui         UI markup + CSS subset laid out into draw commands (no GPU, no fonts)
             l2-catalog          finds client textures, static meshes and material textures by path
             l2-dat-h5           H5 .dat layouts (GPL-derived, migration tooling only)
             l2-dat              schema-driven decoder for the legacy system/*.dat tables
             ue2-package         UE2 package tables: names, imports, exports, object paths
             ue2-core            bounds-checked binary reader (compact index, FString, UTF-16)
             l2-crypto           Lineage2Ver111 / 121 (XOR) and 413 (RSA + zlib)
             l2-env              Env.int and TimeEnv time-of-day color ramps
```

## Comandos

```
cargo run --release -p canastra-cli -- scan    "<H5 client root>"
cargo run --release -p canastra-cli -- package "<file>.utx"
cargo run --release -p canastra-cli -- dat     "<file>.dat"
cargo run --release -p canastra-cli -- texture "<file>.utx" Group.Name out.png
cargo run --release -p canastra-cli -- decrypt "<file>.dat" out.bin
cargo run --release -p canastra-cli -- level   "<client>/MAPS/lobby01.unr"
cargo run --release -p canastra-cli -- mesh    "<client>/Animations/Fighter.ukx" MFighter_anim
cargo run --release -p canastra-studio -- gamedata.cana "<H5 client root>"
cargo run --release -p canastra-cli -- migrate "<H5 client root>" "<server>/data/stats" gamedata.cana
```

La configuración de PostgreSQL (con o sin Docker), de los servidores de login y de juego y del cliente se
explica paso a paso en [docs/es/getting-started.md](docs/es/getting-started.md).

`scan` en un cliente H5 real: se descifran 3020 de 3021 archivos, se leen 1244 paquetes y se decodifican las
54 tablas de `system` con todos los bytes contabilizados. El único fallo (`Animations/RTA63900`) es un archivo
corrupto que quedó de sobra.

## Protecciones

Activa el hook de pre-commit una vez por clon:

```
git config core.hooksPath .githooks
```

Ejecuta `cargo fmt --check`, `cargo clippy -D warnings` y `cargo test`. Los lints están en el `Cargo.toml` del
workspace: `unsafe` está prohibido, el grupo `all` de clippy se deniega, `pedantic` genera advertencias, y
`unwrap`/`panic`/`todo` se deniegan fuera de las pruebas, porque los archivos del cliente son entrada no
confiable.

## Notas de formato (verificadas contra el cliente)

- Encabezado: `Lineage2VerXXX` en UTF-16LE (28 bytes). Trailer opcional de 20 bytes `[0, ?, ?, crc32, 0]`.
- Ver111: XOR `0xAC`. Ver121: XOR con el byte bajo de la suma del nombre del archivo en minúsculas.
  En los archivos XOR, el trailer solo se quita cuando su CRC coincide (los encoders personalizados lo omiten).
- Ver413: bloques RSA de 128 bytes (claves públicas de NCsoft y de l2encdec), cada uno `[0,0,0,size,...data]`,
  que concatenados forman `u32 size` + zlib. La alineación de los bloques indica si existe el trailer.
- Paquetes: etiqueta `0x9E2A83C1`, versiones 117 a 128. El offset de un export está presente cuando
  `SerialSize != 0`.
- Tablas: un `u32` con la cantidad de registros, los registros y luego la `FString` `SafePackage`. Los layouts
  H5 están en `crates/l2-dat-h5`; `RideData` no tenía layout público y se dedujo del archivo.

## Licencia

CanastraEngine está bajo la [Canastra Source License](LICENSE): se puede usar, modificar y compartir de forma
gratuita, y los servidores de juego hechos con él pueden generar dinero, pero el motor en sí no se puede
vender. Las extensiones como plugins, scripts y paquetes de contenido pertenecen a sus autores y se pueden
vender.

53 de los layouts H5 derivan de los descriptores de L2ClientDat, con licencia GPL. Están aislados en
`l2-dat-h5`, bajo GPL-3.0, del que solo pueden depender las herramientas de migración; el cliente, el servidor
y Studio no. Lineage II es una marca registrada de NCSOFT; este proyecto no está afiliado a NCSOFT y no
distribuye archivos del cliente.
