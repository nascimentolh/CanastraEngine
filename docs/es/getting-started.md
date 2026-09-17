[English](../getting-started.md) | [Português (Brasil)](../pt-BR/getting-started.md) | Español

# Primeros pasos

Esta guía lleva un equipo nuevo hasta un CanastraEngine en funcionamiento: los servidores de login y de juego,
el cliente y Studio. Después cubre el desarrollo del día a día.

Windows 10 u 11 es la plataforma probada, y el CI se ejecuta en Windows. Linux y macOS también deberían
compilar; los reportes sobre ellos son bienvenidos.

Docker es opcional. Lo único que ejecuta Docker es PostgreSQL. Los servidores, el cliente y Studio siempre se
ejecutan directamente en tu equipo con `cargo`. Si no quieres usar Docker, instala PostgreSQL de forma nativa
(sección 4, "Sin Docker") y sigue todas las demás secciones tal como están.

## Contenido

1. [Requisitos previos](#1-requisitos-previos)
2. [Instalar las herramientas](#2-instalar-las-herramientas)
3. [Obtener el código y compilarlo](#3-obtener-el-código-y-compilarlo)
4. [Configurar PostgreSQL](#4-configurar-postgresql): [con Docker](#con-docker) o [sin Docker](#sin-docker)
5. [Crear los datos del juego](#5-crear-los-datos-del-juego)
6. [Configurar el servidor de login](#6-configurar-el-servidor-de-login)
7. [Configurar el servidor de juego](#7-configurar-el-servidor-de-juego)
8. [Ejecutar los servidores y el cliente](#8-ejecutar-los-servidores-y-el-cliente)
9. [Editar los datos del juego con Studio](#9-editar-los-datos-del-juego-con-studio)
10. [Desarrollo](#10-desarrollo)
11. [Solución de problemas](#11-solución-de-problemas)

## 1. Requisitos previos

- **Rust 1.98 o más reciente.** Es el `rust-version` del `Cargo.toml` del workspace.
- **Un cliente de Lineage II High Five** ("Freya - High Five") instalado. CanastraEngine lee sus archivos en su
  lugar y nunca los modifica. El repositorio no incluye ningún archivo del cliente.
- **Un datapack de servidor High Five estilo L2J**, por su carpeta `data/stats`. Solo se necesita para crear
  los datos del juego (sección 5).
- **PostgreSQL.** Los servidores guardan ahí las cuentas y los personajes. Usa Docker o una instalación nativa
  (sección 4).
- **Git.**
- Una GPU compatible con **DirectX 12 o Vulkan** (Metal en macOS).
- Unos 5 GB de espacio libre en disco para la compilación.

## 2. Instalar las herramientas

### Rust

**Windows**

1. Descarga y ejecuta `rustup-init.exe` desde [rustup.rs](https://rustup.rs).
2. Cuando pida las herramientas de compilación de C++ de Visual Studio, deja que las instale. También puedes
   instalar [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/) por tu
   cuenta, con la carga de trabajo **Desktop development with C++**.
3. Acepta la instalación predeterminada (toolchain stable, `x86_64-pc-windows-msvc`).
4. Abre una terminal nueva y compruébalo:

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

La lista de paquetes es para Debian y Ubuntu. Cubre las bibliotecas de ventanas que usan el cliente y Studio;
otras distribuciones tienen paquetes con nombres parecidos.

**macOS**

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Si Rust ya está instalado, actualízalo con `rustup update stable`.

## 3. Obtener el código y compilarlo

```bash
git clone https://github.com/nascimentolh/CanastraEngine.git
cd CanastraEngine
cargo build --release
```

La primera compilación descarga las dependencias y tarda varios minutos. Las siguientes son mucho más rápidas.

La compilación no necesita base de datos. Las migraciones SQL van incrustadas en los binarios y se aplican en
tiempo de ejecución.

Ejecuta todos los comandos del resto de esta guía desde la carpeta del repositorio. Los servidores, el cliente
y la herramienta de migración buscan sus archivos (`canastra-login.toml`, `canastra-game.toml`,
`gamedata.cana`, `assets/ui`) en relación con la carpeta actual.

## 4. Configurar PostgreSQL

Los dos servidores se conectan con el `database_url` de su configuración. Las configuraciones de ejemplo usan:

```
postgres://canastra:canastra@localhost:5433/canastra
```

Es decir: usuario `canastra`, contraseña `canastra`, base de datos `canastra`, puerto **5433**. Elige uno de
los dos caminos de abajo. Ambos terminan con una base de datos que los servidores pueden usar.

Ninguno de los dos necesita un paso de migración. Cada vez que un servidor se conecta a la base de datos,
primero aplica las migraciones pendientes de `crates/canastra-db/migrations`. `create-account` se conecta de la
misma forma, así que las tablas existen después del primer comando que usa la base de datos.

### Con Docker

`compose.yaml` define un solo servicio, `postgres`: la imagen `postgres:17-alpine` con usuario, contraseña y
base de datos iguales a `canastra`, publicada en el puerto 5433 del host, con sus datos en el volumen con
nombre `postgres`. No inicia el servidor de login ni el de juego.

1. Instala Docker.
   - Windows y macOS: instala [Docker Desktop](https://www.docker.com/products/docker-desktop/) y ábrelo.
   - Linux: instala Docker Engine y el plugin de Compose desde tu distribución o desde docs.docker.com.
2. Inicia la base de datos:

   ```bash
   docker compose up -d postgres
   ```

3. Comprueba que está en ejecución:

   ```bash
   docker compose ps
   ```

Las configuraciones de ejemplo ya apuntan a esta base de datos, así que no hay nada que cambiar.

Para detenerla, ejecuta `docker compose stop postgres`. Para borrarla junto con todas las cuentas y
personajes, ejecuta `docker compose down -v`.

Continúa con la [sección 5](#5-crear-los-datos-del-juego).

### Sin Docker

Instala PostgreSQL en tu equipo, crea el usuario y la base de datos `canastra` y luego apunta los servidores a
ella. Cualquier versión de PostgreSQL con soporte debería funcionar; el camino con Docker usa la versión 17.

#### Instalar PostgreSQL

**Windows**

1. Descarga el instalador desde [postgresql.org/download/windows](https://www.postgresql.org/download/windows/)
   y ejecútalo.
2. Deja el puerto predeterminado, 5432. Elige una contraseña para el superusuario `postgres` y guárdala.
3. El instalador registra PostgreSQL como un servicio de Windows que arranca con el equipo.
4. Abre un PowerShell nuevo y conéctate como superusuario. Cambia `17` por la versión que instalaste:

   ```powershell
   & "C:\Program Files\PostgreSQL\17\bin\psql.exe" -U postgres
   ```

**Linux (Debian y Ubuntu)**

```bash
sudo apt install postgresql
sudo systemctl enable --now postgresql
sudo -u postgres psql
```

En otras distribuciones, instala el paquete del servidor PostgreSQL, inicialízalo y arráncalo como indique tu
distribución, y luego ejecuta `sudo -u postgres psql`.

**macOS (Homebrew)**

```bash
brew install postgresql@17
brew services start postgresql@17
psql postgres
```

Homebrew crea un superusuario con el nombre de tu usuario de macOS, así que `psql postgres` funciona sin `-U`.

#### Crear el usuario y la base de datos

En el prompt de `psql` del paso anterior, ejecuta:

```sql
CREATE ROLE canastra LOGIN PASSWORD 'canastra';
CREATE DATABASE canastra OWNER canastra;
```

Luego sal con `\q`.

El usuario debe ser dueño de la base de datos. Desde PostgreSQL 15, por defecto solo el dueño de la base de
datos puede crear tablas en el esquema `public`, y las migraciones crean tablas ahí. Si la base de datos es
accesible desde otros equipos, usa otra contraseña y ponla en `database_url`.

#### Apuntar los servidores al puerto 5432

Una instalación nativa escucha en el puerto 5432, mientras que las configuraciones de ejemplo dicen 5433.
Después de crear `canastra-login.toml` y `canastra-game.toml` (secciones 6 y 7), cambia `database_url` en los
dos archivos a:

```toml
database_url = "postgres://canastra:canastra@localhost:5432/canastra"
```

Otra opción es dejar los archivos como están y definir `CANASTRA_DATABASE_URL` en cada terminal que ejecute un
servidor o `create-account`. Esta variable sobrescribe el `database_url` del archivo.

PowerShell:

```powershell
$env:CANASTRA_DATABASE_URL = "postgres://canastra:canastra@localhost:5432/canastra"
```

bash:

```bash
export CANASTRA_DATABASE_URL=postgres://canastra:canastra@localhost:5432/canastra
```

#### Comprobar la conexión

```bash
psql "postgres://canastra:canastra@localhost:5432/canastra" -c "select 1"
```

En Windows, llama a `psql.exe` por su ruta completa, como se muestra arriba. Si esto falla, los servidores
fallarán de la misma forma. Consulta la [solución de problemas](#11-solución-de-problemas).

## 5. Crear los datos del juego

El cliente, el servidor de juego y Studio leen ítems, habilidades, NPCs y clases de un solo archivo `.cana`.
Créalo a partir del cliente y del datapack:

```
cargo run --release -p canastra-cli -- migrate "<H5 client folder>" "<datapack>/data/stats" gamedata.cana
```

El segundo argumento es la carpeta `stats` del datapack. El comando lee sus carpetas `items`, `skills` y
`npcs`, `chars/classList.xml`, `chars/baseStats` e `initialEquipment.xml`. Del cliente lee las tablas de
`system`.

Imprime conteos, un reporte y una línea `validation: N issues`. Apunta a `0 issues`: Studio se niega a guardar
mientras quede algún problema. Si la migración reporta errores, no escribe nada. El tercer argumento es
opcional, pero sin él el comando solo imprime el reporte y no escribe ningún archivo.

Deja `gamedata.cana` en la carpeta del repositorio. El servidor de juego (`game_data` en su configuración) y el
cliente (`CANASTRA_GAME_DATA`) lo buscan ahí por defecto.

Vuelve a ejecutar la migración después de traer cambios en el formato de los datos del juego. Un archivo viejo
falla al cargar con `game data format N is not supported`.

## 6. Configurar el servidor de login

1. Copia la configuración de ejemplo.

   PowerShell:

   ```powershell
   Copy-Item apps\canastra-login\canastra-login.example.toml canastra-login.toml
   ```

   bash:

   ```bash
   cp apps/canastra-login/canastra-login.example.toml canastra-login.toml
   ```

2. Genera sus claves:

   ```
   cargo run --release -p canastra-login -- keygen
   ```

   Imprime una sección `[keys]`. Pégala encima de la sección `[keys]` vacía en `canastra-login.toml`. Guarda la
   salida: el cliente y el servidor de juego necesitan `noise_public`, y el servidor de juego necesita el valor
   `ticket_public` que aparece en la última línea de comentario.

3. Sin Docker, corrige `database_url` como se describe en la [sección 4](#apuntar-los-servidores-al-puerto-5432).

4. Crea una cuenta. Escribe su contraseña en la línea siguiente y presiona Enter:

   ```
   cargo run --release -p canastra-login -- create-account myaccount
   ```

   También puedes definir la contraseña de antemano en `CANASTRA_PASSWORD`, que `create-account` lee entonces
   en lugar de la entrada estándar.

Los demás ajustes están documentados en los comentarios del archivo: `players` (donde se conectan los
jugadores, puerto predeterminado 2106), `game_servers` (donde se registran los servidores de juego,
predeterminado `127.0.0.1:2107`), `ticket_seconds` y los `[limits]` de intentos de login.

`canastra-login.toml` y `canastra-game.toml` están ignorados por git. Nunca hagas commit de ellos.

## 7. Configurar el servidor de juego

1. Copia la configuración de ejemplo.

   PowerShell:

   ```powershell
   Copy-Item apps\canastra-game\canastra-game.example.toml canastra-game.toml
   ```

   bash:

   ```bash
   cp apps/canastra-game/canastra-game.example.toml canastra-game.toml
   ```

2. Genera sus claves y pega la sección `[keys]` impresa encima de la vacía en `canastra-game.toml`:

   ```
   cargo run --release -p canastra-game -- keygen
   ```

3. En `canastra-game.toml`, completa la sección `[login]` con las claves del servidor de login de la sección 6:

   ```toml
   [login]
   address = "127.0.0.1:2107"
   public_key = "<the login server's noise_public>"
   ticket_public = "<the login server's ticket_public>"
   ```

4. En `canastra-login.toml`, autoriza el servidor de juego. Usa el `noise_public` propio del servidor de juego
   y el mismo `id` que en `canastra-game.toml` (1 en el ejemplo):

   ```toml
   [[authorized]]
   id = 1
   public_key = "<the game server's noise_public>"
   ```

5. Sin Docker, corrige `database_url` como se describe en la [sección 4](#apuntar-los-servidores-al-puerto-5432).

El archivo también define el `name` del servidor, `players` (dirección de escucha, puerto predeterminado
7777), `public_address` (a dónde se conectan los jugadores), `capacity`, `game_data` y las reglas de
`[characters]`: espacios por cuenta, el patrón de nombre y las palabras prohibidas.

## 8. Ejecutar los servidores y el cliente

Usa una terminal por programa, todas en la carpeta del repositorio. Inicia primero el servidor de login:

```
cargo run --release -p canastra-login -- serve
```

Registra `login server listening` en el log. Luego inicia el servidor de juego:

```
cargo run --release -p canastra-game -- serve
```

Registra `game data loaded`, `game server listening` y, cuando el servidor de login lo acepta,
`registered with the login server`. Si no se puede llegar al servidor de login, el servidor de juego sigue
reintentando.

Los dos comandos aceptan una ruta de configuración opcional, como en `serve other.toml`. Sin ella, leen
`canastra-login.toml` y `canastra-game.toml` de la carpeta actual. Detén un servidor con Ctrl+C.

Luego inicia el cliente. Necesita el `noise_public` del servidor de login en `CANASTRA_LOGIN_KEY`, para poder
comprobar que habla con el servidor correcto.

**Windows (PowerShell)**

```powershell
$env:CANASTRA_LOGIN_KEY = "<the login server's noise_public>"
.\run-client.bat "C:\path\to\Lineage II High Five"
```

`run-client.bat` cambia a la carpeta del repositorio y ejecuta
`cargo run --release -p canastra-client -- <client folder>`. Sin argumento usa
`%USERPROFILE%\Documents\Lineage II - The Chaotic Throne - Freya - High Five`.

**Linux y macOS (bash)**

```bash
CANASTRA_LOGIN_KEY=<the login server's noise_public> \
    cargo run --release -p canastra-client -- "/path/to/Lineage II High Five"
```

El cliente recibe la carpeta del cliente y una carpeta de UI opcional (predeterminada `assets/ui`). F5 recarga
el markup y el CSS de la UI mientras se ejecuta.

Inicia sesión con la cuenta que creaste, elige el servidor y crea un personaje.

## 9. Editar los datos del juego con Studio

```
cargo run --release -p canastra-studio -- gamedata.cana "<H5 client folder>"
```

La carpeta del cliente es opcional; Studio la usa para mostrar los íconos del cliente. Studio mantiene un
historial para deshacer y se niega a guardar mientras quede algún problema de validación. Reinicia el servidor
de juego después de guardar para que cargue los datos nuevos.

## 10. Desarrollo

Lee [architecture-rules.md](architecture-rules.md) antes de escribir código, y
[CONTRIBUTING.es.md](../../CONTRIBUTING.es.md) para saber cómo se aceptan los cambios.

### Compilar y ejecutar

```
cargo build                        # debug build of the whole workspace
cargo build --release              # the build the sections above use
cargo run -p canastra-cli -- scan "<H5 client folder>"
```

El binario de la CLI se llama `canastra` e imprime sus subcomandos cuando se ejecuta sin argumentos. Los
servidores, el cliente y Studio también imprimen su modo de uso cuando los argumentos son incorrectos.

### Verificaciones

El hook de pre-commit y el CI ejecutan las mismas tres verificaciones. Activa el hook una vez por clon:

```
git config core.hooksPath .githooks
```

Ejecuta las verificaciones a mano con:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked --quiet
```

- Formatea con `cargo fmt --all`. `rustfmt.toml` fija un ancho de línea de 120.
- El `Cargo.toml` del workspace deniega el grupo `all` de clippy y advierte con `pedantic`; `-D warnings`
  convierte esas advertencias en errores. El código `unsafe` está prohibido. `unwrap`, `expect`, `panic!` y la
  indexación se deniegan fuera de las pruebas (`clippy.toml` los permite en las pruebas).

### Pruebas de base de datos

Las pruebas que necesitan PostgreSQL están marcadas con `#[ignore]`, así que un `cargo test` normal las salta.
Para ejecutarlas, apunta `CANASTRA_TEST_DATABASE_URL` a una base de datos. Crean cuentas con nombres de prueba
únicos y las dejan ahí, así que usa una base de datos de desarrollo.

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

Con un PostgreSQL nativo, usa el puerto 5432.

### Formatos versionados

El formato de datos del juego `.cana` y el protocolo de red tienen cada uno una versión
(`canastra_data::format::VERSION` y `canastra_protocol::VERSION`), y las pruebas congelan sus bytes
serializados. Cuando cambies un tipo serializado, incrementa la versión. Para el formato de datos del juego,
regenera la fixture congelada con `CANASTRA_BLESS` definida:

```bash
CANASTRA_BLESS=1 cargo test -p canastra-data
```

En PowerShell, ejecuta primero `$env:CANASTRA_BLESS = "1"`, luego la prueba y después
`Remove-Item Env:CANASTRA_BLESS`.

### Variables de entorno

Estas son todas las variables `CANASTRA_*` que lee el código.

| Variable | Leída por | Qué hace |
| --- | --- | --- |
| `CANASTRA_LOGIN_KEY` | cliente | El `noise_public` del servidor de login. Obligatoria para iniciar sesión. |
| `CANASTRA_LOGIN` | cliente | Dirección del servidor de login como `host:port`. Predeterminada `127.0.0.1:2106`. |
| `CANASTRA_GAME_DATA` | cliente | Ruta del archivo de datos del juego. Predeterminada `gamedata.cana`. |
| `CANASTRA_BACKEND` | cliente | Fuerza un backend gráfico: `dx12`, `vulkan` o `metal`. Por defecto Windows prueba DX12 y luego Vulkan, macOS usa Metal y los demás sistemas usan Vulkan. |
| `CANASTRA_AUTOLOGIN` | cliente | `account:password`. Inicia sesión al arrancar sin escribir. Un atajo de desarrollo para capturas automatizadas. |
| `CANASTRA_DATABASE_URL` | login, juego | Sobrescribe `database_url` de la configuración. |
| `CANASTRA_NOISE_PRIVATE`, `CANASTRA_NOISE_PUBLIC` | login, juego | Sobrescriben el par de claves Noise de `[keys]`. |
| `CANASTRA_TICKET_SECRET` | login | Sobrescribe `keys.ticket_secret`. |
| `CANASTRA_PASSWORD` | login `create-account` | La contraseña de la cuenta nueva, en lugar de leerla de la entrada estándar. |
| `CANASTRA_TEST_DATABASE_URL` | pruebas | Base de datos para las pruebas ignoradas de PostgreSQL. |
| `CANASTRA_BLESS` | pruebas de `canastra-data` | Regenera la fixture congelada de los datos del juego. |

El servidor de juego también lee `RUST_LOG` para fijar los niveles de log, por ejemplo
`RUST_LOG=canastra_game=debug`. El servidor de login no.

## 11. Solución de problemas

### Base de datos

| Síntoma | Qué revisar |
| --- | --- |
| Un servidor o `create-account` espera y luego falla con `database: pool timed out ...` | Nada responde en el host y el puerto de `database_url`. Con Docker, abre Docker y ejecuta `docker compose up -d postgres`. Sin Docker, inicia el servicio de PostgreSQL y revisa el puerto: las instalaciones nativas usan 5432, las configuraciones de ejemplo dicen 5433. |
| `database: ... password authentication failed for user "canastra"` | El usuario o la contraseña no coinciden. Vuelve a crear el rol como en la sección 4, o corrige `database_url`. |
| `database: ... database "canastra" does not exist` | Ejecuta `CREATE DATABASE canastra OWNER canastra;` como superusuario. |
| `database migration: ...` que menciona `permission denied for schema public` | El usuario `canastra` no es dueño de la base de datos. Ejecuta `ALTER DATABASE canastra OWNER TO canastra;` como superusuario. |
| `database migration: migration N was previously applied but is missing in the resolved migrations` | La base de datos es más nueva que el binario del servidor: otra compilación ya aplicó una migración que esta no trae. Vuelve a compilar los servidores desde el commit actual (`cargo build --release`) e inícialos de nuevo. |
| `database_url is not set` | La configuración no tiene `database_url` y `CANASTRA_DATABASE_URL` no está definida. |
| `CANASTRA_DATABASE_URL` parece ignorada | Las variables de entorno aplican por terminal. Defínela en la misma terminal que ejecuta el servidor. |

### Servidores y cliente

| Síntoma | Qué revisar |
| --- | --- |
| `canastra-login.toml: ...` o `canastra-game.toml: ...` justo al iniciar | El archivo no está en la carpeta actual, o su TOML no es válido. Ejecuta desde la carpeta del repositorio, o pasa la ruta de la configuración a `serve`. |
| `keys.noise_private / keys.noise_public: ...`, `keys.ticket_secret: ...`, `login.public_key: ...` o `login.ticket_public: ...` | Una clave está vacía o se pegó mal. Vuelve a pegar la salida de `keygen`. |
| `gamedata.cana: ...` cuando arranca el servidor de juego | Falta el archivo de datos del juego, o lo generó una compilación anterior. Vuelve a ejecutar la migración de la sección 5. |
| El servidor de login registra `a game server with an unauthorized key connected`, y el servidor de juego se sigue reconectando | El `noise_public` del servidor de juego no está en la lista `[[authorized]]` del servidor de login. Reinicia el servidor de login después de editarla. |
| `the login server rejected this server: WrongId` | El `id` en `canastra-game.toml` es distinto del `id` autorizado para su clave. |
| `the login server rejected this server: AlreadyRegistered` | Otro servidor de juego con el mismo id sigue conectado. Detenlo. |
| El cliente dice `CANASTRA_LOGIN_KEY is not set` | Defínela con el `noise_public` del servidor de login en la misma terminal antes de iniciar el cliente. |
| La lista de servidores está vacía | El servidor de juego no está registrado. Busca `registered with the login server` en su log. |
| El cliente dice "This client is out of date. Please update.", o el servidor de juego es rechazado con `UpdateRequired` | El cliente y los servidores se compilaron desde commits con versiones de protocolo distintas. Vuelve a compilarlos todos desde el mismo commit y reinicia los servidores. |
| `game data: gamedata.cana: ...` en la salida del cliente | Ejecuta el cliente desde la carpeta del repositorio, o define `CANASTRA_GAME_DATA`. |
| `no supported graphics adapter`, o la ventana del cliente se queda en negro | Revisa la ruta de la carpeta del cliente. Prueba a forzar un backend con `CANASTRA_BACKEND=vulkan` o `dx12`. |
| `unknown CANASTRA_BACKEND` | El valor debe ser `dx12`, `vulkan` o `metal`. |

### Compilación

| Síntoma | Qué revisar |
| --- | --- |
| En Windows, una recompilación falla porque no se puede reemplazar `canastra-client.exe` (u otro binario) | Windows bloquea un ejecutable en uso. Cierra el cliente, Studio o el servidor y vuelve a compilar. Para encontrar un proceso que quedó abierto, ejecuta `Get-Process canastra*` y luego `Stop-Process -Name canastra-client`. |
| `cargo` no se reconoce | Abre una terminal nueva después de instalar Rust, o agrega `%USERPROFILE%\.cargo\bin` a `PATH`. |
| Errores de enlazado en Windows | Faltan las herramientas de compilación de C++ de Visual Studio. Instala la carga de trabajo **Desktop development with C++**. |
| Cargo dice que un paquete requiere un `rustc` más nuevo | Ejecuta `rustup update stable`. |
| Clippy pasa en local pero falla en el CI | Ejecútalo exactamente como el CI: `cargo clippy --workspace --all-targets --locked -- -D warnings`. |

¿Sigues atascado? Abre un issue con el comando que ejecutaste y su salida completa.
