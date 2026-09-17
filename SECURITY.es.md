[English](SECURITY.md) | [Português (Brasil)](SECURITY.pt-BR.md) | Español

# Seguridad

CanastraEngine incluye servidores de login y de juego expuestos a internet, así que los problemas de seguridad
importan.

## Reportar una vulnerabilidad

No abras un issue público. Repórtala en privado mediante el
[reporte privado de vulnerabilidades](../../security/advisories/new) de GitHub para este repositorio, con:

- qué se ve afectado (cliente, servidor de login, servidor de juego, un crate);
- cómo reproducirla;
- qué podría hacer un atacante con ella.

Recibirás una respuesta en cuanto un mantenedor pueda revisarla. Danos la oportunidad de corregir el problema
antes de hacerlo público.

## Alcance

Ejemplos de lo que queremos saber: formas de saltarse la autenticación o los tickets, formas de tumbar o tomar
el control de un servidor con mensajes manipulados, lectura de datos de otros jugadores y debilidades en el
cifrado de las conexiones. Los problemas del cliente original de Lineage II quedan fuera del alcance.
