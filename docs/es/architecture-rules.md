[English](../architecture-rules.md) | [Português (Brasil)](../pt-BR/architecture-rules.md) | Español

# Reglas de arquitectura

Lee esto antes de escribir o mover código, y revisa un cambio contra estas reglas antes de hacer commit.
Cuando una regla y el código no coinciden, la regla manda para el código nuevo. El código existente se
reorganiza la próxima vez que se toca, no en una gran reescritura. Una regla solo se cambia con una nota
fechada, como hace `decisions.md`.

## 1. Lo más simple que funcione (ponytail)

Detente en el primer escalón que resuelva:

1. ¿Necesita existir? Descarta las necesidades especulativas.
2. ¿Ya está en este código? Reutilízalo.
3. ¿Lo hace la biblioteca estándar?
4. ¿Lo hace una dependencia ya instalada? Agrega una nueva solo cuando unas pocas líneas no alcancen.
5. Solo entonces escribe el mínimo código que funcione.

- Ninguna abstracción con una sola implementación, ninguna factory para un solo producto, ninguna
  configuración que nadie ajusta y ninguna estructura "para después".
- Un atajo deliberado con un límite conocido lleva un comentario `// ponytail:` que indica el límite y el
  camino para mejorarlo.
- Primero entiende el problema: lee el código que toca el cambio y sigue el flujo. La corrección de un bug va
  donde pasan todos los llamadores, no solo donde apareció el síntoma.
- La lógica no trivial (un parser, una regla con muchas ramas, seguridad o dinero) deja una prueba pequeña
  ejecutable.
- Antes de cada commit, revisa el diff en busca de sobreingeniería (ponytail review) y recorta lo que
  encuentre.

## 2. Capas del workspace

- `crates/` contiene bibliotecas y `apps/` contiene binarios. Las dependencias solo apuntan hacia abajo.
- **Los crates de formato son puros** (`ue2-*`, `l2-dat`, `canastra-data`, `canastra-protocol`,
  `canastra-ui`): entran bytes o texto, salen tipos. Sin sistema de archivos, red, reloj ni hilos, para que se
  prueben con entrada sintética.
- **El IO vive en los bordes**: `canastra-net` (conexiones), `canastra-db` (PostgreSQL) y las apps.
- **Las apps son delgadas**: `main.rs` lee los argumentos e inicia las cosas. Cada responsabilidad de la app
  vive en su propio módulo.
- Cliente, servidor y Studio comparten los mismos crates de dominio. Un tipo se define una sola vez; nada se
  copia entre los lados.

## 3. Carpetas por contexto dentro de un crate

- El `src/` de un crate se organiza por **contexto** (el tema del código), no se deja todo en un mismo nivel:
  - `src/scene/` para la escena 3D, `src/network/` para las conexiones, `src/screens/` para las pantallas de
    UI;
  - en un servidor, `src/players/`, `src/registry/`, `src/world/`, `src/characters/`.
- Una carpeta de contexto tiene un `mod.rs` que indica en su doc comment de qué se encarga el contexto y
  expone una API pequeña. Cada uno de sus archivos tiene una sola responsabilidad enfocada.
- Un contexto que cabe en un archivo es ese archivo (`class.rs`). Se convierte en carpeta en cuanto necesita
  un **segundo archivo**; nunca repartas un contexto en varios archivos sueltos en `src/`.
- Los helpers compartidos van en el contexto al que pertenecen. Un cajón de sastre `utils` o `common` no es un
  contexto.
- Las pruebas van junto al código: `#[cfg(test)] mod tests` en el archivo, o `tests.rs` dentro de la carpeta
  de contexto para las pruebas de principio a fin de ese contexto.

## 4. Archivos y funciones

- Un archivo tiene una responsabilidad. Divídelo cuando pase de unas 200 a 250 líneas o empiece a hacer dos
  cosas.
- Clippy se ejecuta con pedantic y `-D warnings`. La entrada no confiable nunca provoca panic: nada de
  `unwrap`, `expect`, indexación ni `panic` fuera de las pruebas.
- Los doc comments dicen para qué sirve algo y cualquier regla no evidente; el código dice cómo. Sigue la
  densidad de comentarios del código que lo rodea.
- Los nombres son palabras completas en inglés. El código, los comentarios y la documentación están en
  inglés; la conversación con el usuario es en portugués.

## 5. Datos y formatos

- **Modelo de dominio, no espejo del legado.** Une y reestructura los datos heredados según lo que significan
  para el juego. Guarda un valor una sola vez: cuando los archivos heredados lo repiten (una clase hija que
  copia su clase inicial), el modelo lo mantiene en un solo lugar.
- Las migraciones dan cuenta de cada campo heredado: mapeado, o ignorado con un motivo declarado. Los datos
  del servidor que no se modelan se cuentan en el reporte.
- Los layouts serializados quedan congelados por pruebas. Cambiar un tipo implica incrementar su `VERSION`
  (`canastra_data::format`, `canastra_protocol`) y regenerar la fixture.
- La configuración es TOML por binario, con secretos que se pueden sobrescribir con variables de entorno.
  Cada app incluye un `*.example.toml` que documenta cada ajuste; las configuraciones reales las ignora git.

## 6. Seguridad y red

- Los jugadores llegan a los servidores solo a través de `canastra-net` (Noise, clave del servidor fijada).
  Los servidores pares usan claves autorizadas.
- El servidor valida cada acción. Los clientes son entrada no confiable.
- Las contraseñas usan Argon2id. La existencia de una cuenta nunca se filtra por mensajes ni por tiempos de
  respuesta.
- Hay límites en todos los bordes: tamaño de frame, timeout del handshake, límites de frecuencia de intentos,
  capacidad.

## 7. Verificación y commits

- Compruébalo con el sistema real: los archivos del cliente H5, un servidor en ejecución, una captura de
  pantalla o una medición. Las pruebas unitarias por sí solas no demuestran que una funcionalidad sirve.
- Las pruebas de base de datos son `#[ignore]` y se ejecutan con `CANASTRA_TEST_DATABASE_URL`.
- Haz commit de cada paso terminado con Conventional Commits (`type(scope): summary`, como en `git log`), un
  cuerpo en prosa que diga qué cambió y por qué, y sin trailers.
- Haz push después de cada commit.
