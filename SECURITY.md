English | [Português (Brasil)](SECURITY.pt-BR.md) | [Español](SECURITY.es.md)

# Security

CanastraEngine includes login and game servers that face the internet, so security problems matter.

## Reporting a vulnerability

Do not open a public issue. Report it privately through GitHub's
[private vulnerability reporting](../../security/advisories/new) for this repository, with:

- what is affected (client, login server, game server, a crate);
- how to reproduce it;
- what an attacker could do with it.

You will get an answer as soon as a maintainer can look at it. Please give us a chance to fix the problem
before sharing it publicly.

## Scope

Examples of what we want to hear about: authentication or ticket bypasses, ways to crash or take over a
server with crafted messages, reading other players' data, and weaknesses in the encryption of
connections. Problems in the original Lineage II client are out of scope.
