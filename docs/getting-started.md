# Getting started

This guide takes a fresh machine to a running CanastraEngine: the login and game servers, the client
showing the login screen, and Studio editing the game data. Windows 10 or 11 is the tested platform;
Linux and macOS should build too, and reports about them are welcome.

## What you need

- A **Lineage II High Five client** ("Freya - High Five") installed. CanastraEngine reads its files in
  place and never changes them. The repository does not include any client files.
- An **L2J-based High Five server datapack**, for its `data/stats` folder (items, skills, NPCs, classes and
  `initialEquipment.xml`). It is only needed once, to create the game data.
- A GPU with **DirectX 12 or Vulkan** support.
- About 5 GB of free disk space for the build.

## 1. Install the tools

### Git

- Windows: install [Git for Windows](https://git-scm.com/download/win) with the default options.
- Linux: `sudo apt install git` (or your distribution's package).
- macOS: `xcode-select --install`.

### Rust

CanastraEngine needs Rust **1.98 or newer**.

**Windows**

1. Download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs).
2. When it asks for the Visual Studio C++ build tools, let it install them, or install
   [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/) yourself
   with the **Desktop development with C++** workload.
3. Accept the default installation (stable toolchain, `x86_64-pc-windows-msvc`).
4. Open a new terminal and check it:

   ```
   rustc --version
   cargo --version
   ```

**Linux**

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo apt install build-essential pkg-config libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
    libxkbcommon-dev libwayland-dev
```

**macOS**

```
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

If Rust was already installed, update it with `rustup update stable`.

### PostgreSQL through Docker

The servers keep accounts and characters in PostgreSQL. The simplest way to run it is Docker:

- Windows and macOS: install [Docker Desktop](https://www.docker.com/products/docker-desktop/) and start it.
- Linux: install Docker Engine and the Compose plugin from your distribution or docs.docker.com.

You can use your own PostgreSQL 17 instead: create a database `canastra` with user and password
`canastra`, reachable on port 5433, or change `database_url` in the server configurations.

## 2. Get the code and build it

```
git clone https://github.com/nascimentolh/CanastraEngine.git
cd CanastraEngine
git config core.hooksPath .githooks
cargo build --release
```

The first build downloads dependencies and takes several minutes; later builds are much faster.

Start the database:

```
docker compose up -d postgres
```

## 3. Create the game data

The client and servers read items, skills, NPCs and classes from one `gamedata.cana` file, migrated from
the client and a datapack:

```
cargo run --release -p canastra-cli -- migrate "<H5 client folder>" "<datapack>/data/stats" gamedata.cana
```

It prints a report and must end with `0 issues`. Keep `gamedata.cana` in the repository folder, where the
programs look for it by default.

## 4. Set up the login server

1. Copy `apps/canastra-login/canastra-login.example.toml` to `canastra-login.toml` in the repository folder.
2. Generate its keys and paste the printed `[keys]` section over the empty one:

   ```
   cargo run --release -p canastra-login -- keygen
   ```

   Keep the output: the game server and the client need `noise_public`, and the game server needs the
   `ticket_public` shown in the last comment line.
3. Create an account. Type its password on the next line and press Enter, or set it beforehand in the
   `CANASTRA_PASSWORD` environment variable:

   ```
   cargo run --release -p canastra-login -- create-account myaccount
   ```

## 5. Set up the game server

1. Copy `apps/canastra-game/canastra-game.example.toml` to `canastra-game.toml`.
2. Generate its keys and paste the printed `[keys]` section over the empty one:

   ```
   cargo run --release -p canastra-game -- keygen
   ```

3. In `canastra-game.toml`, set `[login] public_key` to the login server's `noise_public` and
   `ticket_public` to its `ticket_public`.
4. In `canastra-login.toml`, authorize the game server with its own `noise_public`:

   ```
   [[authorized]]
   id = 1
   public_key = "<the game server's noise_public>"
   ```

## 6. Run everything

Use one terminal per program, from the repository folder:

```
cargo run --release -p canastra-login -- serve
cargo run --release -p canastra-game -- serve
```

The game server logs `registered with the login server` once both are up.

Then start the client, telling it which key the login server must prove it holds:

**Windows (PowerShell)**

```
$env:CANASTRA_LOGIN_KEY = "<the login server's noise_public>"
.\run-client.bat "C:\path\to\Lineage II High Five"
```

**Linux and macOS**

```
CANASTRA_LOGIN_KEY=<the login server's noise_public> \
    cargo run --release -p canastra-client -- "/path/to/Lineage II High Five"
```

Log in with the account you created, pick the server, and create a character.

## 7. Edit the game data with Studio

```
cargo run --release -p canastra-studio -- gamedata.cana "<H5 client folder>"
```

Studio shows client icons, keeps an undo history, and refuses to save while any validation issue remains.
Restart the game server after saving so it loads the new data.

## Troubleshooting

| Problem | What to check |
| --- | --- |
| `pool timed out` when a server starts | PostgreSQL is not running: start Docker and run `docker compose up -d postgres`. |
| The client says `CANASTRA_LOGIN_KEY is not set` | Set it to the login server's `noise_public` before starting the client. |
| The server list is empty | The game server is not registered: check its `[login]` keys and the `[[authorized]]` entry and id on the login server. |
| The client window stays black | The client folder path is wrong, or the GPU backend failed; try `CANASTRA_BACKEND=vulkan` or `dx12`. |
| `cargo` is not recognized | Open a new terminal after installing Rust, or add `%USERPROFILE%\.cargo\bin` to `PATH`. |
| Link errors on Windows | The Visual Studio C++ build tools are missing; install the **Desktop development with C++** workload. |

Still stuck? Open an issue with the command you ran and its full output.
