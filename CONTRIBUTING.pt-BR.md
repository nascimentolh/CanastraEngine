[English](CONTRIBUTING.md) | Português (Brasil) | [Español](CONTRIBUTING.es.md)

# Como contribuir com o CanastraEngine

Obrigado por ajudar a construir o CanastraEngine. Este guia explica como o projeto funciona, para que o seu
tempo vire mudanças que são aceitas. Perguntas são bem-vindas nas issues, em inglês ou português.

## Antes de começar

1. Prepare sua máquina com o [docs/pt-BR/getting-started.md](docs/pt-BR/getting-started.md).
2. Leia as [regras de arquitetura](docs/pt-BR/architecture-rules.md). Toda mudança é conferida com elas.
3. Dê uma olhada no [docs/decisions.md](docs/decisions.md) para ver o que já foi combinado e a ordem de
   construção.
4. Procure uma issue aberta, ou abra uma descrevendo o que você quer fazer antes de começar um trabalho
   maior, para não entrar em conflito com algo em andamento.

Leia também o [código de conduta](CODE_OF_CONDUCT.pt-BR.md); ele vale em todo lugar onde o projeto está.

## Regras obrigatórias

- **Nunca faça commit de arquivos do cliente de Lineage II**, assets extraídos ou qualquer coisa tirada de
  fontes vazadas ou proprietárias. A engine lê um cliente instalado no lugar; o repositório guarda só o nosso
  próprio trabalho.
- **Respeite a fronteira de licença.** Só as ferramentas de migração podem depender de `crates/l2-dat-h5`.
  O cliente, os servidores e o Studio não podem.
- **Nada de segredos.** Os arquivos `canastra-login.toml` e `canastra-game.toml` reais, chaves e senhas ficam
  fora do repositório; só os arquivos `*.example.toml` entram no commit.
- **Clientes não são confiáveis.** O código do servidor valida tudo o que um jogador envia.

## Como o trabalho flui

1. Faça um fork do repositório e crie um branch a partir de `master`, com o nome da mudança:
   `feat/character-select-camera`, `fix/bsp-zero-length-surfaces`.
2. Faça uma mudança focada por pull request. Vários pull requests pequenos são revisados mais rápido que um
   grande.
3. Ative as proteções uma vez por clone, para que todo commit seja conferido:

   ```
   git config core.hooksPath .githooks
   ```

   O hook roda `cargo fmt --check`, `cargo clippy -D warnings` e `cargo test`. O CI roda as mesmas
   verificações e um pull request não pode ser mesclado enquanto elas falham.
4. Abra um pull request contra `master` e preencha o template. É preciso pelo menos uma revisão de um
   mantenedor, e as conversas precisam estar resolvidas antes do merge.

## Escrevendo código

- **A coisa mais simples que funciona.** Nenhuma abstração com uma única implementação, nenhuma configuração
  que ninguém ajusta, nenhum código "para depois". As [regras de arquitetura](docs/pt-BR/architecture-rules.md)
  detalham isso.
- **Organize por contexto.** O `src/` de um crate é dividido em pastas pelo assunto do código, não jogado
  todo no mesmo nível. Os arquivos ficam com no máximo umas 200 a 250 linhas.
- **Crates de formato puros.** `ue2-*`, `l2-dat`, `canastra-data`, `canastra-protocol` e `canastra-ui`
  recebem bytes ou texto e devolvem tipos, sem sistema de arquivos, rede ou relógio.
- **Nada de panic com entrada não confiável.** `unwrap`, `expect`, indexação e `panic!` são negados fora dos
  testes, porque arquivos do cliente e mensagens de rede podem ter qualquer conteúdo.
- **Deixe um teste** para lógica não trivial: um parser, uma regra com ramificações, qualquer coisa que
  envolva segurança.
- **Inglês** no código, nos comentários e na documentação.

## Provando que funciona

Testes unitários sozinhos não provam que uma funcionalidade funciona. Confira com o sistema real:

- Para formatos de arquivo, rode as ferramentas `canastra` (`scan`, `level`, `mesh`, `migrate`) em um cliente
  High Five real e inclua os números no pull request.
- Para qualquer coisa visível, anexe capturas de tela de antes e depois, e uma captura do H5 quando o
  objetivo for ficar igual ao cliente original.
- Para servidores, descreva a execução de ponta a ponta: o que você iniciou, o que fez no cliente e o que os
  logs mostraram.

## Mensagens de commit

Use [Conventional Commits](https://www.conventionalcommits.org): `type(scope): summary`, onde o scope é o
crate ou a área, como no `git log`. Depois do resumo, escreva um corpo em prosa que explique o que mudou, por
que e como foi verificado.

```
fix(ue2-level): skip BSP nodes whose references lead nowhere

One map of the client stores nodes whose surface index is -1, which failed the
whole level. Such nodes are now left out. All 208 maps read.
```

Tipos comuns: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`.

## Relatando bugs e ideias

Abra uma issue com o template correspondente. Para bugs, inclua os passos, o que você esperava, o que
aconteceu, seu sistema operacional e GPU, e logs ou capturas de tela. Problemas de segurança não vão em
issues; veja o [SECURITY.pt-BR.md](SECURITY.pt-BR.md).

## Licença das contribuições

Ao contribuir, você concorda que o seu trabalho fica licenciado sob a [Canastra Source License](LICENSE).
Em resumo: o CanastraEngine pode ser usado, modificado e compartilhado de graça, e servidores feitos com ele
podem gerar dinheiro, mas a engine em si não pode ser vendida. As extensões que você escrever são suas e
podem ser vendidas.
