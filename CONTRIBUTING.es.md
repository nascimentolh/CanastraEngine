[English](CONTRIBUTING.md) | [Português (Brasil)](CONTRIBUTING.pt-BR.md) | Español

# Cómo contribuir a CanastraEngine

Gracias por ayudar a construir CanastraEngine. Esta guía explica cómo funciona el proyecto, para que tu tiempo
se convierta en cambios que se aceptan. Las preguntas son bienvenidas en los issues, en inglés o portugués.

## Antes de empezar

1. Prepara tu equipo con [docs/es/getting-started.md](docs/es/getting-started.md).
2. Lee las [reglas de arquitectura](docs/es/architecture-rules.md). Todo cambio se revisa contra ellas.
3. Revisa [docs/decisions.md](docs/decisions.md) para ver lo que ya se acordó y el orden de construcción.
4. Busca un issue abierto, o abre uno que describa lo que quieres hacer antes de empezar un trabajo grande,
   para no chocar con algo que ya está en curso.

Lee también el [código de conducta](CODE_OF_CONDUCT.es.md); se aplica en todos los lugares donde está el
proyecto.

## Reglas obligatorias

- **Nunca hagas commit de archivos del cliente de Lineage II**, assets extraídos ni nada tomado de fuentes
  filtradas o propietarias. El motor lee un cliente instalado en su lugar; el repositorio contiene solo
  nuestro propio trabajo.
- **Respeta el límite de licencias.** Solo las herramientas de migración pueden depender de
  `crates/l2-dat-h5`. El cliente, los servidores y Studio no.
- **Nada de secretos.** Los archivos `canastra-login.toml` y `canastra-game.toml` reales, las claves y las
  contraseñas quedan fuera del repositorio; solo se hace commit de los archivos `*.example.toml`.
- **Los clientes no son confiables.** El código del servidor valida todo lo que envía un jugador.

## Cómo fluye el trabajo

1. Haz un fork del repositorio y crea una rama desde `master`, con el nombre del cambio:
   `feat/character-select-camera`, `fix/bsp-zero-length-surfaces`.
2. Haz un cambio enfocado por pull request. Varios pull requests pequeños se revisan más rápido que uno
   grande.
3. Activa las protecciones una vez por clon, para que se revise cada commit:

   ```
   git config core.hooksPath .githooks
   ```

   El hook ejecuta `cargo fmt --check`, `cargo clippy -D warnings` y `cargo test`. El CI ejecuta las mismas
   verificaciones y un pull request no se puede fusionar mientras fallen.
4. Abre un pull request contra `master` y completa la plantilla. Se necesita al menos una revisión de un
   mantenedor, y las conversaciones deben estar resueltas antes de fusionar.

## Escribir código

- **Lo más simple que funcione.** Ninguna abstracción con una sola implementación, ninguna configuración que
  nadie ajusta, nada de código "para después". Las [reglas de arquitectura](docs/es/architecture-rules.md)
  lo detallan.
- **Organiza por contexto.** El `src/` de un crate se divide en carpetas según el tema del código, no se deja
  todo en un mismo nivel. Los archivos tienen como máximo unas 200 a 250 líneas.
- **Crates de formato puros.** `ue2-*`, `l2-dat`, `canastra-data`, `canastra-protocol` y `canastra-ui`
  reciben bytes o texto y devuelven tipos, sin sistema de archivos, red ni reloj.
- **Nada de panics con entrada no confiable.** `unwrap`, `expect`, la indexación y `panic!` se deniegan fuera
  de las pruebas, porque los archivos del cliente y los mensajes de red pueden contener cualquier cosa.
- **Deja una prueba** para la lógica no trivial: un parser, una regla con ramas, cualquier cosa que toque la
  seguridad.
- **Inglés** para el código, los comentarios y la documentación.

## Demostrar que funciona

Las pruebas unitarias por sí solas no demuestran que una funcionalidad sirve. Compruébalo con el sistema real:

- Para formatos de archivo, ejecuta las herramientas `canastra` (`scan`, `level`, `mesh`, `migrate`) en un
  cliente High Five real e incluye los números en el pull request.
- Para todo lo visible, adjunta capturas de pantalla de antes y después, y una captura del H5 cuando el
  objetivo sea igualar el cliente original.
- Para los servidores, describe la ejecución de principio a fin: qué iniciaste, qué hiciste en el cliente y
  qué mostraron los logs.

## Mensajes de commit

Usa [Conventional Commits](https://www.conventionalcommits.org): `type(scope): summary`, donde el scope es el
crate o el área, como en `git log`. Después del resumen, escribe un cuerpo en prosa que explique qué cambió,
por qué y cómo se verificó.

```
fix(ue2-level): skip BSP nodes whose references lead nowhere

One map of the client stores nodes whose surface index is -1, which failed the
whole level. Such nodes are now left out. All 208 maps read.
```

Tipos comunes: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`.

## Reportar errores e ideas

Abre un issue con la plantilla correspondiente. Para errores, incluye los pasos, lo que esperabas, lo que
pasó, tu sistema operativo y GPU, y logs o capturas de pantalla. Los problemas de seguridad no van en issues;
consulta [SECURITY.es.md](SECURITY.es.md).

## Licencia de las contribuciones

Al contribuir, aceptas que tu trabajo queda bajo la [Canastra Source License](LICENSE).
En resumen: CanastraEngine se puede usar, modificar y compartir de forma gratuita, y los servidores hechos con
él pueden generar dinero, pero el motor en sí no se puede vender. Las extensiones que escribas son tuyas y se
pueden vender.
