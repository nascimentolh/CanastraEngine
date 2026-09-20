English | [Português (Brasil)](pt-BR/getting-started.md) | [Español](es/getting-started.md)

# Getting started

This guide takes a fresh machine to a running CanastraEngine: the login and game servers, the client, and
Studio. It then covers day-to-day development.

Windows 10 or 11 is the tested platform, and CI runs on Windows. Linux and macOS should build too; reports
about them are welcome.

Docker is optional. The only thing Docker runs is PostgreSQL. The servers, the client and Studio always run
directly on your machine with `cargo`. If you do not want Docker, install PostgreSQL natively (section 4,
"Without Docker") and follow every other section as written.

## Contents

1. [Prerequisites](#1-prerequisites)
2. [Install the tools](#2-install-the-tools)
3. [Get the code and build it](#3-get-the-code-and-build-it)
4. [Set up PostgreSQL](#4-set-up-postgresql): [with Docker](#with-docker) or [without Docker](#without-docker)
5. [Create the game data](#5-create-the-game-data)
6. [Configure the login server](#6-configure-the-login-server)
7. [Configure the game server](#7-configure-the-game-server)
8. [Run the servers and the client](#8-run-the-servers-and-the-client)
9. [Edit the game data with Studio](#9-edit-the-game-data-with-studio)
10. [Developing](#10-developing)
11. [Troubleshooting](#11-troubleshooting)

## 1. Prerequisites

- **Rust 1.98 or newer.** This is the `rust-version` in the workspace `Cargo.toml`.
- **A Lineage II High Five client** ("Freya - High Five") installed. CanastraEngine reads its files in place
  and never changes them. The repository does not include any client files.
- **An L2J-style High Five server datapack**, for its `data/stats` folder. It is only needed to create the
  game data (section 5).
- **PostgreSQL.** The servers keep accounts and characters there. Use Docker or a native install
  (section 4).
- **Git.**
- A GPU with **DirectX 12 or Vulkan** support (Metal on macOS).
- About 5 GB of free disk space for the build.

## 2. Install the tools

### Rust

**Windows**

1. Download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs).
2. When it asks for the Visual Studio C++ build tools, let it install them. You can also install
   [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/) yourself
   with the **Desktop development with C++** workload.
3. Accept the default installation (stable toolchain, `x86_64-pc-windows-msvc`).
4. Open a new terminal and check it:

   ```powershell
   rustc --version
   cargo --version
   ```

**Linux**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo apt install build-essential pkg-config libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
    libxkbcommon-dev libwayland-dev
```

The package list is for Debian and Ubuntu. It covers the windowing libraries the client and Studio use;
other distributions have packages with similar names.

**macOS**

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

If Rust is already installed, update it with `rustup update stable`.

## 3. Get the code and build it

```bash
git clone https://github.com/nascimentolh/CanastraEngine.git
cd CanastraEngine
cargo build --release
```

The first build downloads dependencies and takes several minutes. Later builds are much faster.

The build does not need a database. The SQL migrations are embedded into the binaries and applied at run
time.

Run every command in the rest of this guide from the repository folder. The servers, the client and the
migration tool look for their files (`canastra-login.toml`, `canastra-game.toml`, `gamedata.cana`,
`assets/ui`) relative to the current folder.

## 4. Set up PostgreSQL

Both servers connect with the `database_url` in their configuration. The example configurations use:

```
postgres://canastra:canastra@localhost:5433/canastra
```

That is user `canastra`, password `canastra`, database `canastra`, port **5433**. Pick one of the two paths
below. Both end with a database the servers can use.

Neither path needs a migration step. Whenever a server connects to the database, it first applies any
pending migrations from `crates/canastra-db/migrations`. `create-account` connects the same way, so the
tables exist after the first command that touches the database.

### With Docker

`compose.yaml` defines a single service, `postgres`: the `postgres:17-alpine` image with user, password and
database all set to `canastra`, published on host port 5433, with its data kept in the named volume
`postgres`. It does not start the login or game server.

1. Install Docker.
   - Windows and macOS: install [Docker Desktop](https://www.docker.com/products/docker-desktop/) and start
     it.
   - Linux: install Docker Engine and the Compose plugin from your distribution or docs.docker.com.
2. Start the database:

   ```bash
   docker compose up -d postgres
   ```

3. Check that it is running:

   ```bash
   docker compose ps
   ```

The example configurations already point at this database, so there is nothing to change.

To stop it, run `docker compose stop postgres`. To delete it along with all accounts and characters, run
`docker compose down -v`.

Continue with [section 5](#5-create-the-game-data).

### Without Docker

Install PostgreSQL on your machine, create the `canastra` user and database, then point the servers at it.
Any supported PostgreSQL release should work; the Docker path uses version 17.

#### Install PostgreSQL

**Windows**

1. Download the installer from [postgresql.org/download/windows](https://www.postgresql.org/download/windows/)
   and run it.
2. Keep the default port, 5432. Choose a password for the `postgres` superuser and keep it.
3. The installer registers PostgreSQL as a Windows service that starts with the machine.
4. Open a new PowerShell and connect as the superuser. Adjust `17` to the version you installed:

   ```powershell
   & "C:\Program Files\PostgreSQL\17\bin\psql.exe" -U postgres
   ```

**Linux (Debian and Ubuntu)**

```bash
sudo apt install postgresql
sudo systemctl enable --now postgresql
sudo -u postgres psql
```

On other distributions, install the PostgreSQL server package, initialize and start it as your distribution
documents, then run `sudo -u postgres psql`.

**macOS (Homebrew)**

```bash
brew install postgresql@17
brew services start postgresql@17
psql postgres
```

Homebrew creates a superuser named after your macOS user, so `psql postgres` works without `-U`.

#### Create the user and database

In the `psql` prompt from the previous step, run:

```sql
CREATE ROLE canastra LOGIN PASSWORD 'canastra';
CREATE DATABASE canastra OWNER canastra;
```

Then leave with `\q`.

The user must own the database. Since PostgreSQL 15, only the database owner may create tables in the
`public` schema by default, and the migrations create tables there. If the database is reachable from other
machines, use a different password and put it in `database_url`.

#### Point the servers at port 5432

A native install listens on port 5432, while the example configurations say 5433. After you create
`canastra-login.toml` and `canastra-game.toml` (sections 6 and 7), change `database_url` in both files to:

```toml
database_url = "postgres://canastra:canastra@localhost:5432/canastra"
```

Alternatively, leave the files alone and set `CANASTRA_DATABASE_URL` in each terminal that runs a server or
`create-account`. It overrides `database_url` from the file.

PowerShell:

```powershell
$env:CANASTRA_DATABASE_URL = "postgres://canastra:canastra@localhost:5432/canastra"
```

bash:

```bash
export CANASTRA_DATABASE_URL=postgres://canastra:canastra@localhost:5432/canastra
```

#### Check the connection

```bash
psql "postgres://canastra:canastra@localhost:5432/canastra" -c "select 1"
```

On Windows, call `psql.exe` by its full path as shown above. If this fails, the servers will fail the same
way. See [troubleshooting](#11-troubleshooting).

## 5. Create the game data

The client, the game server and Studio read items, skills, NPCs and classes from one `.cana` file. Create it
from the client and the datapack:

```
cargo run --release -p canastra-cli -- migrate "<H5 client folder>" "<datapack>/data/stats" gamedata.cana
```

The second argument is the datapack's `stats` folder. The command reads its `items`, `skills` and `npcs`
folders, `chars/classList.xml`, `chars/baseStats` and `initialEquipment.xml`. From the client it reads the
tables in `system`.

It prints counts, a report, and a `validation: N issues` line. Aim for `0 issues`: Studio refuses to save
while any issue remains. If the migration reports errors, it writes nothing. The third argument is optional,
but without it the command only prints the report and writes no file.

Keep `gamedata.cana` in the repository folder. The game server (`game_data` in its configuration) and the
client (`CANASTRA_GAME_DATA`) look for it there by default.

Run the migration again after pulling changes to the game data format. An old file fails to load with
`game data format N is not supported`.

## 6. Configure the login server

1. Copy the example configuration.

   PowerShell:

   ```powershell
   Copy-Item apps\canastra-login\canastra-login.example.toml canastra-login.toml
   ```

   bash:

   ```bash
   cp apps/canastra-login/canastra-login.example.toml canastra-login.toml
   ```

2. Generate its keys:

   ```
   cargo run --release -p canastra-login -- keygen
   ```

   It prints a `[keys]` section. Paste it over the empty `[keys]` section in `canastra-login.toml`. Keep the
   output: the client and the game server need `noise_public`, and the game server needs the
   `ticket_public` value shown in the last comment line.

3. Without Docker, fix `database_url` as described in [section 4](#point-the-servers-at-port-5432).

4. Create an account. Type its password on the next line and press Enter:

   ```
   cargo run --release -p canastra-login -- create-account myaccount
   ```

   You can also set the password beforehand in `CANASTRA_PASSWORD`, which `create-account` then reads
   instead of standard input.

The other settings are documented by the comments in the file: `players` (where players connect, default
port 2106), `game_servers` (where game servers register, default `127.0.0.1:2107`), `ticket_seconds`, and
the `[limits]` on login attempts.

`canastra-login.toml` and `canastra-game.toml` are ignored by git. Never commit them.

## 7. Configure the game server

1. Copy the example configuration.

   PowerShell:

   ```powershell
   Copy-Item apps\canastra-game\canastra-game.example.toml canastra-game.toml
   ```

   bash:

   ```bash
   cp apps/canastra-game/canastra-game.example.toml canastra-game.toml
   ```

2. Generate its keys and paste the printed `[keys]` section over the empty one in `canastra-game.toml`:

   ```
   cargo run --release -p canastra-game -- keygen
   ```

3. In `canastra-game.toml`, fill the `[login]` section with the login server's keys from section 6:

   ```toml
   [login]
   address = "127.0.0.1:2107"
   public_key = "<the login server's noise_public>"
   ticket_public = "<the login server's ticket_public>"
   ```

4. In `canastra-login.toml`, authorize the game server. Use the game server's own `noise_public` and the
   same `id` as in `canastra-game.toml` (1 in the example):

   ```toml
   [[authorized]]
   id = 1
   public_key = "<the game server's noise_public>"
   ```

5. Without Docker, fix `database_url` as described in [section 4](#point-the-servers-at-port-5432).

The file also sets the server's `name`, `players` (listen address, default port 7777), `public_address`
(what players connect to), `capacity`, `game_data`, and the `[characters]` rules: slots per account, the
name pattern and forbidden words.

### Geodata (optional)

`geodata` names a folder of High Five geodata tiles, files called `17_25.l2j` and so on, one per map tile.
With it the server decides where the ground is: characters enter standing on the floor under them and stop at
walls instead of walking through them. Without it they walk wherever they ask to. The repository ships no
geodata; use the one your server already has.

## 8. Run the servers and the client

Use one terminal per program, all in the repository folder. Start the login server first:

```
cargo run --release -p canastra-login -- serve
```

It logs `login server listening`. Then start the game server:

```
cargo run --release -p canastra-game -- serve
```

It logs `game data loaded`, `game server listening`, and `registered with the login server` once the login
server accepts it. If the login server cannot be reached, the game server keeps retrying.

Both commands take an optional configuration path, as in `serve other.toml`. Without it they read
`canastra-login.toml` and `canastra-game.toml` from the current folder. Stop a server with Ctrl+C.

Then start the client. It needs the login server's `noise_public` in `CANASTRA_LOGIN_KEY`, so it can check
that it is talking to the right server.

**Windows (PowerShell)**

```powershell
$env:CANASTRA_LOGIN_KEY = "<the login server's noise_public>"
.\run-client.bat "C:\path\to\Lineage II High Five"
```

`run-client.bat` changes to the repository folder and runs
`cargo run --release -p canastra-client -- <client folder>`. Without an argument it uses
`%USERPROFILE%\Documents\Lineage II - The Chaotic Throne - Freya - High Five`.

**Linux and macOS (bash)**

```bash
CANASTRA_LOGIN_KEY=<the login server's noise_public> \
    cargo run --release -p canastra-client -- "/path/to/Lineage II High Five"
```

The client takes the client folder and an optional UI folder (default `assets/ui`). F5 reloads the UI
markup and CSS while it runs.

Log in with the account you created, pick the server, and create a character.

## 9. Edit the game data with Studio

```
cargo run --release -p canastra-studio -- gamedata.cana "<H5 client folder>"
```

The client folder is optional; Studio uses it to show client icons. Studio keeps an undo history and refuses
to save while any validation issue remains. Restart the game server after saving so it loads the new data.

## 10. Developing

Read [architecture-rules.md](architecture-rules.md) before writing code, and
[CONTRIBUTING.md](../CONTRIBUTING.md) for how changes get merged.

### Build and run

```
cargo build                        # debug build of the whole workspace
cargo build --release              # the build the sections above use
cargo run -p canastra-cli -- scan "<H5 client folder>"
```

The CLI binary is named `canastra` and prints its subcommands when run without arguments. The servers, the client and Studio also
print their usage when the arguments are wrong.

### Checks

The pre-commit hook and CI run the same three checks. Enable the hook once per clone:

```
git config core.hooksPath .githooks
```

Run the checks by hand with:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked --quiet
```

- Format with `cargo fmt --all`. `rustfmt.toml` sets a line width of 120.
- The workspace `Cargo.toml` denies clippy `all` and warns on `pedantic`; `-D warnings` turns those warnings
  into errors. `unsafe` code is forbidden. `unwrap`, `expect`, `panic!` and indexing are denied outside
  tests (`clippy.toml` allows them in tests).

### Database tests

Tests that need PostgreSQL are marked `#[ignore]`, so a plain `cargo test` skips them. To run them, point
`CANASTRA_TEST_DATABASE_URL` at a database. They create accounts with unique test names and leave them
there, so use a development database.

PowerShell:

```powershell
$env:CANASTRA_TEST_DATABASE_URL = "postgres://canastra:canastra@localhost:5433/canastra"
cargo test -p canastra-db -- --include-ignored
cargo test -p canastra-login -- --include-ignored
```

bash:

```bash
export CANASTRA_TEST_DATABASE_URL=postgres://canastra:canastra@localhost:5433/canastra
cargo test -p canastra-db -- --include-ignored
cargo test -p canastra-login -- --include-ignored
```

With a native PostgreSQL, use port 5432.

### Versioned formats

The `.cana` game data format and the network protocol each have a version
(`canastra_data::format::VERSION` and `canastra_protocol::VERSION`), and tests freeze their serialized
bytes. When you change a serialized type, bump the version. For the game data format, rewrite the frozen
fixture with `CANASTRA_BLESS` set:

```bash
CANASTRA_BLESS=1 cargo test -p canastra-data
```

In PowerShell, run `$env:CANASTRA_BLESS = "1"` first, run the test, then `Remove-Item Env:CANASTRA_BLESS`.

### Environment variables

These are all the `CANASTRA_*` variables the code reads.

| Variable | Read by | What it does |
| --- | --- | --- |
| `CANASTRA_LOGIN_KEY` | client | The login server's `noise_public`. Required to log in. |
| `CANASTRA_LOGIN` | client | Login server address as `host:port`. Default `127.0.0.1:2106`. |
| `CANASTRA_GAME_DATA` | client | Path of the game data file. Default `gamedata.cana`. |
| `CANASTRA_BACKEND` | client | Forces a graphics backend: `dx12`, `vulkan` or `metal`. By default Windows tries DX12 then Vulkan, macOS uses Metal, and other systems use Vulkan. |
| `CANASTRA_AUTOLOGIN` | client | `account:password`. Logs in at start without typing. A development shortcut for automated captures. |
| `CANASTRA_DATABASE_URL` | login, game | Overrides `database_url` from the configuration. |
| `CANASTRA_NOISE_PRIVATE`, `CANASTRA_NOISE_PUBLIC` | login, game | Override the Noise key pair in `[keys]`. |
| `CANASTRA_TICKET_SECRET` | login | Overrides `keys.ticket_secret`. |
| `CANASTRA_PASSWORD` | login `create-account` | The new account's password, instead of reading it from standard input. |
| `CANASTRA_TEST_DATABASE_URL` | tests | Database for the ignored PostgreSQL tests. |
| `CANASTRA_BLESS` | `canastra-data` tests | Rewrites the frozen game data fixture. |

The game server also reads `RUST_LOG` to set log levels, for example `RUST_LOG=canastra_game=debug`. The
login server does not.

## 11. Troubleshooting

### Database

| Symptom | What to check |
| --- | --- |
| A server or `create-account` waits, then fails with `database: pool timed out ...` | Nothing answers at the host and port in `database_url`. With Docker, start Docker and run `docker compose up -d postgres`. Without Docker, start the PostgreSQL service and check the port: native installs use 5432, the example configurations say 5433. |
| `database: ... password authentication failed for user "canastra"` | The user or password does not match. Recreate the role as in section 4, or fix `database_url`. |
| `database: ... database "canastra" does not exist` | Run `CREATE DATABASE canastra OWNER canastra;` as the superuser. |
| `database migration: ...` mentioning `permission denied for schema public` | The `canastra` user does not own the database. Run `ALTER DATABASE canastra OWNER TO canastra;` as the superuser. |
| `database migration: migration N was previously applied but is missing in the resolved migrations` | The database is newer than the server binary: another build already applied a migration this one does not carry. Rebuild the servers from the current commit (`cargo build --release`) and start them again. |
| `database_url is not set` | The configuration has no `database_url` and `CANASTRA_DATABASE_URL` is not set. |
| `CANASTRA_DATABASE_URL` seems ignored | Environment variables apply per terminal. Set it in the same terminal that runs the server. |

### Servers and client

| Symptom | What to check |
| --- | --- |
| `canastra-login.toml: ...` or `canastra-game.toml: ...` right at start | The file is missing from the current folder, or its TOML is invalid. Run from the repository folder, or pass the configuration path to `serve`. |
| `keys.noise_private / keys.noise_public: ...`, `keys.ticket_secret: ...`, `login.public_key: ...` or `login.ticket_public: ...` | A key is empty or was pasted wrongly. Paste the `keygen` output again. |
| `gamedata.cana: ...` when the game server starts | The game data file is missing, or an older build made it. Run the migration from section 5 again. |
| The login server logs `a game server with an unauthorized key connected`, and the game server keeps reconnecting | The game server's `noise_public` is not in the login server's `[[authorized]]` list. Restart the login server after editing it. |
| `the login server rejected this server: WrongId` | The `id` in `canastra-game.toml` differs from the `id` authorized for its key. |
| `the login server rejected this server: AlreadyRegistered` | Another game server with the same id is still connected. Stop it. |
| The client says `CANASTRA_LOGIN_KEY is not set` | Set it to the login server's `noise_public` in the same terminal before starting the client. |
| The server list is empty | The game server is not registered. Look for `registered with the login server` in its log. |
| The client says "This client is out of date. Please update.", or the game server is rejected with `UpdateRequired` | The client and servers were built from commits with different protocol versions. Rebuild all of them from the same commit and restart the servers. |
| `game data: gamedata.cana: ...` in the client output | Run the client from the repository folder, or set `CANASTRA_GAME_DATA`. |
| `no supported graphics adapter`, or the client window stays black | Check the client folder path. Try forcing a backend with `CANASTRA_BACKEND=vulkan` or `dx12`. |
| `unknown CANASTRA_BACKEND` | The value must be `dx12`, `vulkan` or `metal`. |

### Building

| Symptom | What to check |
| --- | --- |
| On Windows, a rebuild fails because `canastra-client.exe` (or another binary) cannot be replaced | Windows locks a running executable. Close the client, Studio or server, then build again. To find a leftover process, run `Get-Process canastra*`, then `Stop-Process -Name canastra-client`. |
| `cargo` is not recognized | Open a new terminal after installing Rust, or add `%USERPROFILE%\.cargo\bin` to `PATH`. |
| Link errors on Windows | The Visual Studio C++ build tools are missing. Install the **Desktop development with C++** workload. |
| Cargo says a package requires a newer `rustc` | Run `rustup update stable`. |
| Clippy passes locally but fails in CI | Run it exactly as CI does: `cargo clippy --workspace --all-targets --locked -- -D warnings`. |

Still stuck? Open an issue with the command you ran and its full output.
