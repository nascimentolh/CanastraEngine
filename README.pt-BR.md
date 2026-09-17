[English](README.md) | Português (Brasil) | [Español](README.es.md)

<p align="center"><img src="logo.png" alt="Canastra Engine" width="560"></p>

# CanastraEngine

Cliente e servidores de Lineage 2 High Five em Rust. Ele lê os assets direto de uma instalação existente do
cliente H5; as tabelas de `system` (`.dat`) estão sendo migradas para o formato de dados próprio do Canastra.
Veja [`docs/decisions.md`](docs/decisions.md) para a arquitetura combinada e a ordem de construção.

<p align="center"><img src="docs/screenshots/01-login.png" alt="Tela de login" width="720"></p>

- **Experimente:** [docs/pt-BR/getting-started.md](docs/pt-BR/getting-started.md) instala o Rust e roda os
  servidores, o cliente e o Studio.
- **Veja:** [docs/screenshots](docs/screenshots) mostra as telas que já funcionam.
- **Ajude a construir:** leia o [CONTRIBUTING.pt-BR.md](CONTRIBUTING.pt-BR.md) e o
  [código de conduta](CODE_OF_CONDUCT.pt-BR.md).

## Arquitetura

As dependências só apontam para baixo. Os crates de formato são puros (`&[u8]` entra, tipos saem, sem IO),
então podem ser testados com bytes sintéticos e reaproveitados por ferramentas.

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

A configuração do PostgreSQL (com ou sem Docker), dos servidores de login e de jogo e do cliente está descrita
passo a passo em [docs/pt-BR/getting-started.md](docs/pt-BR/getting-started.md).

`scan` em um cliente H5 real: 3020 de 3021 arquivos são descriptografados, 1244 pacotes são lidos e todas as
54 tabelas de `system` são decodificadas com todos os bytes contabilizados. A única falha
(`Animations/RTA63900`) é um arquivo corrompido que sobrou.

## Proteções

Ative o hook de pre-commit uma vez por clone:

```
git config core.hooksPath .githooks
```

Ele roda `cargo fmt --check`, `cargo clippy -D warnings` e `cargo test`. Os lints ficam no `Cargo.toml` do
workspace: `unsafe` é proibido, o `all` do clippy é negado, `pedantic` gera aviso, e
`unwrap`/`panic`/`todo` são negados fora dos testes, porque os arquivos do cliente são entrada não confiável.

## Notas sobre formatos (verificadas no cliente)

- Cabeçalho: `Lineage2VerXXX` em UTF-16LE (28 bytes). Trailer opcional de 20 bytes `[0, ?, ?, crc32, 0]`.
- Ver111: XOR `0xAC`. Ver121: XOR com o byte menos significativo da soma do nome do arquivo em minúsculas.
  Em arquivos XOR, o trailer só é removido quando o CRC confere (encoders customizados o omitem).
- Ver413: blocos RSA de 128 bytes (chaves públicas da NCsoft e do l2encdec), cada um `[0,0,0,size,...data]`,
  que concatenados formam `u32 size` + zlib. O alinhamento dos blocos indica se o trailer existe.
- Pacotes: tag `0x9E2A83C1`, versões 117 a 128. O offset de um export está presente quando `SerialSize != 0`.
- Tabelas: um `u32` com a contagem de registros, os registros e depois a `FString` `SafePackage`. Os layouts
  H5 ficam em `crates/l2-dat-h5`; `RideData` não tinha layout público e foi deduzido do arquivo.

## Licença

O CanastraEngine está sob a [Canastra Source License](LICENSE): pode ser usado, modificado e compartilhado de
graça, e servidores de jogo feitos com ele podem gerar dinheiro, mas a engine em si não pode ser vendida.
Extensões como plugins, scripts e pacotes de conteúdo pertencem aos seus autores e podem ser vendidas.

53 dos layouts H5 derivam dos descritores do L2ClientDat, licenciados sob GPL. Eles ficam isolados em
`l2-dat-h5`, sob GPL-3.0, do qual só as ferramentas de migração podem depender; o cliente, o servidor e o
Studio não podem. Lineage II é uma marca registrada da NCSOFT; este projeto não tem vínculo com a NCSOFT e
não distribui arquivos do cliente.
