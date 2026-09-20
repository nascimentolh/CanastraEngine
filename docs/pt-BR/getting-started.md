[English](../getting-started.md) | Português (Brasil) | [Español](../es/getting-started.md)

# Primeiros passos

Este guia leva uma máquina nova até um CanastraEngine rodando: os servidores de login e de jogo, o cliente e
o Studio. Depois ele cobre o desenvolvimento do dia a dia.

Windows 10 ou 11 é a plataforma testada, e o CI roda no Windows. Linux e macOS também devem compilar; relatos
sobre eles são bem-vindos.

Docker é opcional. A única coisa que o Docker roda é o PostgreSQL. Os servidores, o cliente e o Studio sempre
rodam direto na sua máquina com `cargo`. Se você não quiser usar Docker, instale o PostgreSQL de forma nativa
(seção 4, "Sem Docker") e siga todas as outras seções como estão.

## Conteúdo

1. [Pré-requisitos](#1-pré-requisitos)
2. [Instalar as ferramentas](#2-instalar-as-ferramentas)
3. [Baixar o código e compilar](#3-baixar-o-código-e-compilar)
4. [Configurar o PostgreSQL](#4-configurar-o-postgresql): [com Docker](#com-docker) ou [sem Docker](#sem-docker)
5. [Criar os dados do jogo](#5-criar-os-dados-do-jogo)
6. [Configurar o servidor de login](#6-configurar-o-servidor-de-login)
7. [Configurar o servidor de jogo](#7-configurar-o-servidor-de-jogo)
8. [Rodar os servidores e o cliente](#8-rodar-os-servidores-e-o-cliente)
9. [Editar os dados do jogo com o Studio](#9-editar-os-dados-do-jogo-com-o-studio)
10. [Desenvolvimento](#10-desenvolvimento)
11. [Solução de problemas](#11-solução-de-problemas)

## 1. Pré-requisitos

- **Rust 1.98 ou mais recente.** É o `rust-version` do `Cargo.toml` do workspace.
- **Um cliente de Lineage II High Five** ("Freya - High Five") instalado. O CanastraEngine lê os arquivos
  dele no lugar e nunca os altera. O repositório não inclui nenhum arquivo do cliente.
- **Um datapack de servidor High Five no estilo L2J**, por causa da pasta `data/stats`. Ele só é necessário
  para criar os dados do jogo (seção 5).
- **PostgreSQL.** Os servidores guardam contas e personagens nele. Use Docker ou uma instalação nativa
  (seção 4).
- **Git.**
- Uma GPU com suporte a **DirectX 12 ou Vulkan** (Metal no macOS).
- Cerca de 5 GB de espaço livre em disco para a compilação.

## 2. Instalar as ferramentas

### Rust

**Windows**

1. Baixe e execute o `rustup-init.exe` em [rustup.rs](https://rustup.rs).
2. Quando ele pedir as ferramentas de build C++ do Visual Studio, deixe que ele as instale. Você também pode
   instalar o [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   por conta própria, com a carga de trabalho **Desktop development with C++**.
3. Aceite a instalação padrão (toolchain stable, `x86_64-pc-windows-msvc`).
4. Abra um novo terminal e confira:

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

A lista de pacotes é para Debian e Ubuntu. Ela cobre as bibliotecas de janela que o cliente e o Studio usam;
outras distribuições têm pacotes com nomes parecidos.

**macOS**

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Se o Rust já estiver instalado, atualize com `rustup update stable`.

## 3. Baixar o código e compilar

```bash
git clone https://github.com/nascimentolh/CanastraEngine.git
cd CanastraEngine
cargo build --release
```

A primeira compilação baixa as dependências e leva vários minutos. As seguintes são bem mais rápidas.

A compilação não precisa de banco de dados. As migrações SQL ficam embutidas nos binários e são aplicadas em
tempo de execução.

Rode todos os comandos do resto deste guia a partir da pasta do repositório. Os servidores, o cliente e a
ferramenta de migração procuram seus arquivos (`canastra-login.toml`, `canastra-game.toml`, `gamedata.cana`,
`assets/ui`) relativos à pasta atual.

## 4. Configurar o PostgreSQL

Os dois servidores se conectam usando o `database_url` da sua configuração. As configurações de exemplo usam:

```
postgres://canastra:canastra@localhost:5433/canastra
```

Ou seja: usuário `canastra`, senha `canastra`, banco `canastra`, porta **5433**. Escolha um dos dois caminhos
abaixo. Os dois terminam com um banco que os servidores conseguem usar.

Nenhum dos caminhos precisa de uma etapa de migração. Sempre que um servidor se conecta ao banco, ele primeiro
aplica as migrações pendentes de `crates/canastra-db/migrations`. O `create-account` se conecta do mesmo
jeito, então as tabelas existem depois do primeiro comando que acessa o banco.

### Com Docker

O `compose.yaml` define um único serviço, `postgres`: a imagem `postgres:17-alpine` com usuário, senha e banco
todos iguais a `canastra`, publicada na porta 5433 do host, com os dados guardados no volume nomeado
`postgres`. Ele não inicia o servidor de login nem o de jogo.

1. Instale o Docker.
   - Windows e macOS: instale o [Docker Desktop](https://www.docker.com/products/docker-desktop/) e abra-o.
   - Linux: instale o Docker Engine e o plugin do Compose pela sua distribuição ou por docs.docker.com.
2. Inicie o banco:

   ```bash
   docker compose up -d postgres
   ```

3. Confira se ele está rodando:

   ```bash
   docker compose ps
   ```

As configurações de exemplo já apontam para este banco, então não há nada para mudar.

Para parar, rode `docker compose stop postgres`. Para apagá-lo junto com todas as contas e personagens, rode
`docker compose down -v`.

Continue na [seção 5](#5-criar-os-dados-do-jogo).

### Sem Docker

Instale o PostgreSQL na sua máquina, crie o usuário e o banco `canastra` e aponte os servidores para ele.
Qualquer versão suportada do PostgreSQL deve funcionar; o caminho com Docker usa a versão 17.

#### Instalar o PostgreSQL

**Windows**

1. Baixe o instalador em [postgresql.org/download/windows](https://www.postgresql.org/download/windows/)
   e execute.
2. Mantenha a porta padrão, 5432. Escolha uma senha para o superusuário `postgres` e guarde.
3. O instalador registra o PostgreSQL como um serviço do Windows que inicia junto com a máquina.
4. Abra um novo PowerShell e conecte como superusuário. Troque `17` pela versão que você instalou:

   ```powershell
   & "C:\Program Files\PostgreSQL\17\bin\psql.exe" -U postgres
   ```

**Linux (Debian e Ubuntu)**

```bash
sudo apt install postgresql
sudo systemctl enable --now postgresql
sudo -u postgres psql
```

Em outras distribuições, instale o pacote do servidor PostgreSQL, inicialize e inicie o serviço como a sua
distribuição documenta, e depois rode `sudo -u postgres psql`.

**macOS (Homebrew)**

```bash
brew install postgresql@17
brew services start postgresql@17
psql postgres
```

O Homebrew cria um superusuário com o nome do seu usuário do macOS, então `psql postgres` funciona sem `-U`.

#### Criar o usuário e o banco

No prompt do `psql` do passo anterior, rode:

```sql
CREATE ROLE canastra LOGIN PASSWORD 'canastra';
CREATE DATABASE canastra OWNER canastra;
```

Depois saia com `\q`.

O usuário precisa ser o dono do banco. Desde o PostgreSQL 15, por padrão só o dono do banco pode criar tabelas
no schema `public`, e as migrações criam tabelas nele. Se o banco puder ser acessado de outras máquinas, use
outra senha e coloque-a no `database_url`.

#### Apontar os servidores para a porta 5432

Uma instalação nativa escuta na porta 5432, enquanto as configurações de exemplo dizem 5433. Depois de criar
`canastra-login.toml` e `canastra-game.toml` (seções 6 e 7), mude o `database_url` nos dois arquivos para:

```toml
database_url = "postgres://canastra:canastra@localhost:5432/canastra"
```

Outra opção é deixar os arquivos como estão e definir `CANASTRA_DATABASE_URL` em cada terminal que roda um
servidor ou o `create-account`. Ela sobrescreve o `database_url` do arquivo.

PowerShell:

```powershell
$env:CANASTRA_DATABASE_URL = "postgres://canastra:canastra@localhost:5432/canastra"
```

bash:

```bash
export CANASTRA_DATABASE_URL=postgres://canastra:canastra@localhost:5432/canastra
```

#### Conferir a conexão

```bash
psql "postgres://canastra:canastra@localhost:5432/canastra" -c "select 1"
```

No Windows, chame o `psql.exe` pelo caminho completo, como mostrado acima. Se isso falhar, os servidores vão
falhar do mesmo jeito. Veja a [solução de problemas](#11-solução-de-problemas).

## 5. Criar os dados do jogo

O cliente, o servidor de jogo e o Studio leem itens, skills, NPCs e classes de um único arquivo `.cana`. Crie
esse arquivo a partir do cliente e do datapack:

```
cargo run --release -p canastra-cli -- migrate "<H5 client folder>" "<datapack>/data/stats" gamedata.cana
```

O segundo argumento é a pasta `stats` do datapack. O comando lê as pastas `items`, `skills` e `npcs`,
`chars/classList.xml`, `chars/baseStats` e `initialEquipment.xml`. Do cliente, ele lê as tabelas em `system`.

Ele imprime contagens, um relatório e uma linha `validation: N issues`. Busque `0 issues`: o Studio se recusa
a salvar enquanto houver algum problema. Se a migração reportar erros, ela não grava nada. O terceiro
argumento é opcional, mas sem ele o comando só imprime o relatório e não grava arquivo nenhum.

Mantenha o `gamedata.cana` na pasta do repositório. O servidor de jogo (`game_data` na configuração) e o
cliente (`CANASTRA_GAME_DATA`) procuram o arquivo ali por padrão.

Rode a migração de novo depois de puxar mudanças no formato dos dados do jogo. Um arquivo antigo falha ao
carregar com `game data format N is not supported`.

## 6. Configurar o servidor de login

1. Copie a configuração de exemplo.

   PowerShell:

   ```powershell
   Copy-Item apps\canastra-login\canastra-login.example.toml canastra-login.toml
   ```

   bash:

   ```bash
   cp apps/canastra-login/canastra-login.example.toml canastra-login.toml
   ```

2. Gere as chaves:

   ```
   cargo run --release -p canastra-login -- keygen
   ```

   O comando imprime uma seção `[keys]`. Cole-a por cima da seção `[keys]` vazia em `canastra-login.toml`.
   Guarde a saída: o cliente e o servidor de jogo precisam do `noise_public`, e o servidor de jogo precisa do
   valor `ticket_public` mostrado na última linha de comentário.

3. Sem Docker, ajuste o `database_url` como descrito na [seção 4](#apontar-os-servidores-para-a-porta-5432).

4. Crie uma conta. Digite a senha na linha seguinte e pressione Enter:

   ```
   cargo run --release -p canastra-login -- create-account myaccount
   ```

   Você também pode definir a senha antes em `CANASTRA_PASSWORD`, que o `create-account` passa a ler no
   lugar da entrada padrão.

Os outros ajustes estão documentados nos comentários do arquivo: `players` (onde os jogadores se conectam,
porta padrão 2106), `game_servers` (onde os servidores de jogo se registram, padrão `127.0.0.1:2107`),
`ticket_seconds` e os `[limits]` de tentativas de login.

`canastra-login.toml` e `canastra-game.toml` são ignorados pelo git. Nunca faça commit deles.

## 7. Configurar o servidor de jogo

1. Copie a configuração de exemplo.

   PowerShell:

   ```powershell
   Copy-Item apps\canastra-game\canastra-game.example.toml canastra-game.toml
   ```

   bash:

   ```bash
   cp apps/canastra-game/canastra-game.example.toml canastra-game.toml
   ```

2. Gere as chaves e cole a seção `[keys]` impressa por cima da seção vazia em `canastra-game.toml`:

   ```
   cargo run --release -p canastra-game -- keygen
   ```

3. Em `canastra-game.toml`, preencha a seção `[login]` com as chaves do servidor de login da seção 6:

   ```toml
   [login]
   address = "127.0.0.1:2107"
   public_key = "<the login server's noise_public>"
   ticket_public = "<the login server's ticket_public>"
   ```

4. Em `canastra-login.toml`, autorize o servidor de jogo. Use o `noise_public` do próprio servidor de jogo e o
   mesmo `id` de `canastra-game.toml` (1 no exemplo):

   ```toml
   [[authorized]]
   id = 1
   public_key = "<the game server's noise_public>"
   ```

5. Sem Docker, ajuste o `database_url` como descrito na [seção 4](#apontar-os-servidores-para-a-porta-5432).

O arquivo também define o `name` do servidor, `players` (endereço de escuta, porta padrão 7777),
`public_address` (onde os jogadores se conectam), `capacity`, `game_data` e as regras de `[characters]`:
slots por conta, o padrão de nome e as palavras proibidas.

### Geodata (opcional)

`geodata` aponta para uma pasta de tiles de geodata do High Five, arquivos chamados `17_25.l2j` e assim por
diante, um por tile de mapa. Com ela o servidor decide onde fica o chão: os personagens entram em pé sobre o
piso embaixo deles e param nas paredes em vez de atravessá-las. Sem ela, andam para onde pedirem. O
repositório não traz geodata; use a que o seu servidor já tiver.

## 8. Rodar os servidores e o cliente

Use um terminal por programa, todos na pasta do repositório. Inicie primeiro o servidor de login:

```
cargo run --release -p canastra-login -- serve
```

Ele registra `login server listening` no log. Depois inicie o servidor de jogo:

```
cargo run --release -p canastra-game -- serve
```

Ele registra `game data loaded`, `game server listening` e, quando o servidor de login o aceita,
`registered with the login server`. Se o servidor de login não estiver acessível, o servidor de jogo continua
tentando.

Os dois comandos aceitam um caminho de configuração opcional, como em `serve other.toml`. Sem ele, leem
`canastra-login.toml` e `canastra-game.toml` da pasta atual. Pare um servidor com Ctrl+C.

Depois inicie o cliente. Ele precisa do `noise_public` do servidor de login em `CANASTRA_LOGIN_KEY`, para
conferir que está falando com o servidor certo.

**Windows (PowerShell)**

```powershell
$env:CANASTRA_LOGIN_KEY = "<the login server's noise_public>"
.\run-client.bat "C:\path\to\Lineage II High Five"
```

O `run-client.bat` muda para a pasta do repositório e roda
`cargo run --release -p canastra-client -- <client folder>`. Sem argumento, ele usa
`%USERPROFILE%\Documents\Lineage II - The Chaotic Throne - Freya - High Five`.

**Linux e macOS (bash)**

```bash
CANASTRA_LOGIN_KEY=<the login server's noise_public> \
    cargo run --release -p canastra-client -- "/path/to/Lineage II High Five"
```

O cliente recebe a pasta do cliente e uma pasta de UI opcional (padrão `assets/ui`). F5 recarrega o markup e
o CSS da UI enquanto ele roda.

Entre com a conta que você criou, escolha o servidor e crie um personagem.

## 9. Editar os dados do jogo com o Studio

```
cargo run --release -p canastra-studio -- gamedata.cana "<H5 client folder>"
```

A pasta do cliente é opcional; o Studio a usa para mostrar os ícones do cliente. O Studio mantém um histórico
de desfazer e se recusa a salvar enquanto houver algum problema de validação. Reinicie o servidor de jogo
depois de salvar para que ele carregue os dados novos.

## 10. Desenvolvimento

Leia o [architecture-rules.md](architecture-rules.md) antes de escrever código, e o
[CONTRIBUTING.pt-BR.md](../../CONTRIBUTING.pt-BR.md) para saber como as mudanças são aceitas.

### Compilar e rodar

```
cargo build                        # debug build of the whole workspace
cargo build --release              # the build the sections above use
cargo run -p canastra-cli -- scan "<H5 client folder>"
```

O binário da CLI se chama `canastra` e imprime seus subcomandos quando roda sem argumentos. Os servidores, o
cliente e o Studio também imprimem o modo de uso quando os argumentos estão errados.

### Verificações

O hook de pre-commit e o CI rodam as mesmas três verificações. Ative o hook uma vez por clone:

```
git config core.hooksPath .githooks
```

Rode as verificações à mão com:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked --quiet
```

- Formate com `cargo fmt --all`. O `rustfmt.toml` define uma largura de linha de 120.
- O `Cargo.toml` do workspace nega o `all` do clippy e avisa em `pedantic`; `-D warnings` transforma esses
  avisos em erros. Código `unsafe` é proibido. `unwrap`, `expect`, `panic!` e indexação são negados fora dos
  testes (o `clippy.toml` permite nos testes).

### Testes de banco de dados

Os testes que precisam do PostgreSQL são marcados com `#[ignore]`, então um `cargo test` simples os pula. Para
rodá-los, aponte `CANASTRA_TEST_DATABASE_URL` para um banco. Eles criam contas com nomes de teste únicos e as
deixam lá, então use um banco de desenvolvimento.

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

Com um PostgreSQL nativo, use a porta 5432.

### Formatos versionados

O formato de dados do jogo `.cana` e o protocolo de rede têm cada um uma versão
(`canastra_data::format::VERSION` e `canastra_protocol::VERSION`), e os testes congelam os bytes serializados.
Quando você muda um tipo serializado, incremente a versão. Para o formato de dados do jogo, regrave a fixture
congelada com `CANASTRA_BLESS` definido:

```bash
CANASTRA_BLESS=1 cargo test -p canastra-data
```

No PowerShell, rode `$env:CANASTRA_BLESS = "1"` primeiro, rode o teste e depois `Remove-Item Env:CANASTRA_BLESS`.

### Variáveis de ambiente

Estas são todas as variáveis `CANASTRA_*` que o código lê.

| Variável | Lida por | O que faz |
| --- | --- | --- |
| `CANASTRA_LOGIN_KEY` | cliente | O `noise_public` do servidor de login. Obrigatória para entrar. |
| `CANASTRA_LOGIN` | cliente | Endereço do servidor de login no formato `host:port`. Padrão `127.0.0.1:2106`. |
| `CANASTRA_GAME_DATA` | cliente | Caminho do arquivo de dados do jogo. Padrão `gamedata.cana`. |
| `CANASTRA_BACKEND` | cliente | Força um backend gráfico: `dx12`, `vulkan` ou `metal`. Por padrão o Windows tenta DX12 e depois Vulkan, o macOS usa Metal e os outros sistemas usam Vulkan. |
| `CANASTRA_AUTOLOGIN` | cliente | `account:password`. Entra ao iniciar sem digitar. Um atalho de desenvolvimento para capturas automatizadas. |
| `CANASTRA_DATABASE_URL` | login, jogo | Sobrescreve o `database_url` da configuração. |
| `CANASTRA_NOISE_PRIVATE`, `CANASTRA_NOISE_PUBLIC` | login, jogo | Sobrescrevem o par de chaves Noise em `[keys]`. |
| `CANASTRA_TICKET_SECRET` | login | Sobrescreve `keys.ticket_secret`. |
| `CANASTRA_PASSWORD` | login `create-account` | A senha da nova conta, em vez de lê-la da entrada padrão. |
| `CANASTRA_TEST_DATABASE_URL` | testes | Banco para os testes ignorados de PostgreSQL. |
| `CANASTRA_BLESS` | testes de `canastra-data` | Regrava a fixture congelada dos dados do jogo. |

O servidor de jogo também lê `RUST_LOG` para definir os níveis de log, por exemplo
`RUST_LOG=canastra_game=debug`. O servidor de login não lê.

## 11. Solução de problemas

### Banco de dados

| Sintoma | O que conferir |
| --- | --- |
| Um servidor ou o `create-account` espera e depois falha com `database: pool timed out ...` | Nada responde no host e na porta do `database_url`. Com Docker, abra o Docker e rode `docker compose up -d postgres`. Sem Docker, inicie o serviço do PostgreSQL e confira a porta: instalações nativas usam 5432, as configurações de exemplo dizem 5433. |
| `database: ... password authentication failed for user "canastra"` | O usuário ou a senha não conferem. Recrie o role como na seção 4, ou corrija o `database_url`. |
| `database: ... database "canastra" does not exist` | Rode `CREATE DATABASE canastra OWNER canastra;` como superusuário. |
| `database migration: ...` mencionando `permission denied for schema public` | O usuário `canastra` não é dono do banco. Rode `ALTER DATABASE canastra OWNER TO canastra;` como superusuário. |
| `database migration: migration N was previously applied but is missing in the resolved migrations` | O banco é mais novo que o binário do servidor: outra compilação já aplicou uma migração que esta não traz. Recompile os servidores a partir do commit atual (`cargo build --release`) e inicie-os de novo. |
| `database_url is not set` | A configuração não tem `database_url` e `CANASTRA_DATABASE_URL` não está definida. |
| `CANASTRA_DATABASE_URL` parece ser ignorada | Variáveis de ambiente valem por terminal. Defina-a no mesmo terminal que roda o servidor. |

### Servidores e cliente

| Sintoma | O que conferir |
| --- | --- |
| `canastra-login.toml: ...` ou `canastra-game.toml: ...` logo ao iniciar | O arquivo não está na pasta atual, ou o TOML dele é inválido. Rode a partir da pasta do repositório, ou passe o caminho da configuração para o `serve`. |
| `keys.noise_private / keys.noise_public: ...`, `keys.ticket_secret: ...`, `login.public_key: ...` ou `login.ticket_public: ...` | Uma chave está vazia ou foi colada errada. Cole de novo a saída do `keygen`. |
| `gamedata.cana: ...` quando o servidor de jogo inicia | O arquivo de dados do jogo não existe, ou foi gerado por uma compilação mais antiga. Rode de novo a migração da seção 5. |
| O servidor de login registra `a game server with an unauthorized key connected`, e o servidor de jogo fica reconectando | O `noise_public` do servidor de jogo não está na lista `[[authorized]]` do servidor de login. Reinicie o servidor de login depois de editá-la. |
| `the login server rejected this server: WrongId` | O `id` em `canastra-game.toml` é diferente do `id` autorizado para a chave dele. |
| `the login server rejected this server: AlreadyRegistered` | Outro servidor de jogo com o mesmo id ainda está conectado. Pare-o. |
| O cliente diz `CANASTRA_LOGIN_KEY is not set` | Defina-a com o `noise_public` do servidor de login no mesmo terminal, antes de iniciar o cliente. |
| A lista de servidores está vazia | O servidor de jogo não está registrado. Procure `registered with the login server` no log dele. |
| O cliente diz "This client is out of date. Please update.", ou o servidor de jogo é rejeitado com `UpdateRequired` | O cliente e os servidores foram compilados a partir de commits com versões de protocolo diferentes. Recompile todos a partir do mesmo commit e reinicie os servidores. |
| `game data: gamedata.cana: ...` na saída do cliente | Rode o cliente a partir da pasta do repositório, ou defina `CANASTRA_GAME_DATA`. |
| `no supported graphics adapter`, ou a janela do cliente fica preta | Confira o caminho da pasta do cliente. Tente forçar um backend com `CANASTRA_BACKEND=vulkan` ou `dx12`. |
| `unknown CANASTRA_BACKEND` | O valor precisa ser `dx12`, `vulkan` ou `metal`. |

### Compilação

| Sintoma | O que conferir |
| --- | --- |
| No Windows, uma recompilação falha porque `canastra-client.exe` (ou outro binário) não pode ser substituído | O Windows trava um executável em execução. Feche o cliente, o Studio ou o servidor e compile de novo. Para achar um processo que ficou para trás, rode `Get-Process canastra*` e depois `Stop-Process -Name canastra-client`. |
| `cargo` não é reconhecido | Abra um novo terminal depois de instalar o Rust, ou adicione `%USERPROFILE%\.cargo\bin` ao `PATH`. |
| Erros de link no Windows | Faltam as ferramentas de build C++ do Visual Studio. Instale a carga de trabalho **Desktop development with C++**. |
| O Cargo diz que um pacote exige um `rustc` mais novo | Rode `rustup update stable`. |
| O clippy passa localmente mas falha no CI | Rode exatamente como o CI: `cargo clippy --workspace --all-targets --locked -- -D warnings`. |

Ainda travado? Abra uma issue com o comando que você rodou e a saída completa.
