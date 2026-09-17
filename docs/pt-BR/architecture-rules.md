[English](../architecture-rules.md) | Português (Brasil) | [Español](../es/architecture-rules.md)

# Regras de arquitetura

Leia isto antes de escrever ou mover código, e confira uma mudança com estas regras antes do commit. Quando
uma regra e o código discordam, a regra vale para o código novo. O código existente é reorganizado na próxima
vez que for mexido, não em uma grande reescrita. Só mude uma regra com uma nota datada, como faz o
`decisions.md`.

## 1. A coisa mais simples que funciona (ponytail)

Pare no primeiro degrau que resolver:

1. Isso precisa existir? Descarte necessidades especulativas.
2. Já existe nesta base de código? Reaproveite.
3. A biblioteca padrão faz isso?
4. Uma dependência já instalada faz isso? Só adicione uma nova quando poucas linhas não bastarem.
5. Só então escreva o mínimo de código que funciona.

- Nenhuma abstração com uma única implementação, nenhuma factory para um único produto, nenhuma configuração
  que ninguém ajusta e nenhuma estrutura "para depois".
- Um atalho deliberado com um limite conhecido recebe um comentário `// ponytail:` que diz qual é o limite e
  o caminho para evoluir.
- Entenda o problema primeiro: leia o código que a mudança toca e siga o fluxo. A correção de um bug vai no
  ponto por onde todos os chamadores passam, não só onde o sintoma apareceu.
- Lógica não trivial (um parser, uma regra cheia de ramificações, segurança ou dinheiro) deixa um teste
  pequeno executável.
- Antes de todo commit, revise o diff em busca de excesso de engenharia (ponytail review) e corte o que
  aparecer.

## 2. Camadas do workspace

- `crates/` guarda bibliotecas e `apps/` guarda binários. As dependências só apontam para baixo.
- **Crates de formato são puros** (`ue2-*`, `l2-dat`, `canastra-data`, `canastra-protocol`, `canastra-ui`):
  entram bytes ou texto, saem tipos. Sem sistema de arquivos, rede, relógio ou threads, para que sejam
  testados com entrada sintética.
- **O IO fica nas bordas**: `canastra-net` (conexões), `canastra-db` (PostgreSQL) e os apps.
- **Os apps são finos**: o `main.rs` lê os argumentos e inicia as coisas. Cada responsabilidade do app fica no
  seu próprio módulo.
- Cliente, servidor e Studio compartilham os mesmos crates de domínio. Um tipo é definido uma vez; nada é
  copiado entre os lados.

## 3. Pastas por contexto dentro de um crate

- O `src/` de um crate é organizado por **contexto** (o assunto do código), não jogado todo no mesmo nível:
  - `src/scene/` para a cena 3D, `src/network/` para conexões, `src/screens/` para as telas de UI;
  - em um servidor, `src/players/`, `src/registry/`, `src/world/`, `src/characters/`.
- Uma pasta de contexto tem um `mod.rs` que diz no seu doc comment o que o contexto possui e expõe uma API
  pequena. Cada arquivo dela tem uma única responsabilidade focada.
- Um contexto que cabe em um arquivo é esse arquivo (`class.rs`). Ele vira pasta assim que precisa de um
  **segundo arquivo**; nunca espalhe um contexto em vários arquivos soltos em `src/`.
- Helpers compartilhados ficam no contexto dono deles. Um saco de sobras `utils` ou `common` não é um
  contexto.
- Os testes ficam junto do código: `#[cfg(test)] mod tests` no arquivo, ou `tests.rs` dentro da pasta de
  contexto para testes de ponta a ponta daquele contexto.

## 4. Arquivos e funções

- Um arquivo tem uma responsabilidade. Divida quando ele passar de umas 200 a 250 linhas ou começar a fazer
  duas coisas.
- O clippy roda com pedantic e `-D warnings`. Entrada não confiável nunca causa panic: nada de `unwrap`,
  `expect`, indexação ou `panic` fora dos testes.
- Doc comments dizem para que algo serve e qualquer regra não óbvia; o código diz como. Siga a densidade de
  comentários do código ao redor.
- Nomes são palavras inteiras em inglês. Código, comentários e documentação são em inglês; a conversa com o
  usuário é em português.

## 5. Dados e formatos

- **Modelo de domínio, não espelho do legado.** Junte e reestruture os dados legados no que eles significam
  para o jogo. Guarde um valor uma vez: quando os arquivos legados o repetem (uma classe filha copiando a
  classe inicial), o modelo o mantém em um só lugar.
- As migrações dão conta de todo campo legado: mapeado, ou ignorado com um motivo declarado. Dados de servidor
  não modelados são contados no relatório.
- Layouts serializados são congelados por testes. Mudar um tipo significa incrementar o seu `VERSION`
  (`canastra_data::format`, `canastra_protocol`) e regravar a fixture.
- A configuração é TOML por binário, com segredos que podem ser sobrescritos por variáveis de ambiente. Todo
  app traz um `*.example.toml` que documenta cada ajuste; as configurações reais são ignoradas pelo git.

## 6. Segurança e rede

- Jogadores só chegam aos servidores pelo `canastra-net` (Noise, chave do servidor fixada). Servidores pares
  usam chaves autorizadas.
- O servidor valida toda ação. Clientes são entrada não confiável.
- Senhas usam Argon2id. A existência de uma conta nunca vaza por mensagens ou por tempo de resposta.
- Há limites em todas as bordas: tamanho de frame, timeout de handshake, limites de taxa de tentativas,
  capacidade.

## 7. Verificação e commits

- Confira com o sistema real: os arquivos do cliente H5, um servidor rodando, uma captura de tela ou uma
  medição. Testes unitários sozinhos não provam que uma funcionalidade funciona.
- Os testes de banco de dados são `#[ignore]` e rodam com `CANASTRA_TEST_DATABASE_URL`.
- Faça commit de cada passo concluído com Conventional Commits (`type(scope): summary`, como no `git log`), um
  corpo em prosa dizendo o que mudou e por que, e sem trailers.
- Faça push depois de cada commit.
