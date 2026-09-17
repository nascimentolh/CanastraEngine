[English](SECURITY.md) | Português (Brasil) | [Español](SECURITY.es.md)

# Segurança

O CanastraEngine inclui servidores de login e de jogo expostos à internet, então problemas de segurança
importam.

## Como relatar uma vulnerabilidade

Não abra uma issue pública. Relate em particular pelo
[relato privado de vulnerabilidades](../../security/advisories/new) do GitHub para este repositório, com:

- o que é afetado (cliente, servidor de login, servidor de jogo, um crate);
- como reproduzir;
- o que um atacante poderia fazer com isso.

Você vai receber uma resposta assim que um mantenedor puder analisar. Dê a nós a chance de corrigir o problema
antes de divulgá-lo publicamente.

## Escopo

Exemplos do que queremos saber: formas de burlar a autenticação ou os tickets, formas de derrubar ou tomar
controle de um servidor com mensagens forjadas, leitura de dados de outros jogadores e fraquezas na
criptografia das conexões. Problemas no cliente original de Lineage II estão fora do escopo.
