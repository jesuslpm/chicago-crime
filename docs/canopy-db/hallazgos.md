# Registro de hallazgos de CanopyDB

Registro común del módulo CanopyDB ([PRD del módulo](../prd/chicago-crime/canopy-db.prd.md), sección "Registro
de hallazgos"; forma fijada por la decisión KG9 del [PRD del generador
CanopyDB](../prd/chicago-crime/canopy-db-generador.prd.md)). Cada comportamiento de CanopyDB que se aparte de lo que
su documentación promete o de lo que exige el contrato del proyecto tiene aquí una entrada con su reproducción
mínima y la versión afectada (flujo E del PRD del módulo).

- Los identificadores siguen el orden de alta y nunca se reutilizan.
- Ninguna acción pública fuera del proyecto (issue, PR, comentario, fork o commits en el fork) se hace sin la
  aprobación del responsable del proyecto, una por una (decisión K4); cada aprobación se anota en su entrada.
- Los issues y PR se redactan en inglés, el idioma del repositorio de CanopyDB.
- Un campo sin contenido lleva `—`.

## Reproducciones

Las reproducciones de los hallazgos que no son de documentación están en [`repro/`](repro/): un programa que solo
depende de CanopyDB y de la biblioteca estándar, fuera del workspace del proyecto. Se ejecutan con una orden desde
esa carpeta:

```text
cargo run -- h2
cargo run -- h3
```

Cada una trabaja en una carpeta nueva de la carpeta temporal del sistema, muestra lo que observa y termina con
`REPRODUCED` y el código 0 si reproduce el comportamiento del hallazgo, o con `NOT REPRODUCED` y el código 1 si no.

## Resumen

| Id | Tipo | Estado | Título |
|---|---|---|---|
| H1 | Documentación | Detectado | El tercer ejemplo del README configura la caché, pero no usa esas opciones |
| H2 | Limitación | Detectado | Dos procesos no pueden abrir a la vez la misma base |
| H3 | Limitación | Detectado | No hay apertura de solo lectura |
| H4 | Documentación | Detectado | El tamaño por defecto de la caché no aparece en la portada de la documentación |

## H1 — El tercer ejemplo del README configura la caché, pero no usa esas opciones

- **Identificador**: H1
- **Fecha**: 2026-10-20
- **Versión de CanopyDB**: 0.2.5
- **Tipo**: Documentación
- **Descripción y efecto**: el tercer ejemplo del README ("Multiple Databases with Atomic Cross-Database
  Commits"), que también es la portada de la documentación en docs.rs, crea unas `EnvOptions` con un tamaño de
  caché, pero abre el entorno con `Environment::new`, que no recibe esas opciones. El ajuste no tiene efecto, y
  quien copie el ejemplo cree que ha limitado la memoria cuando la caché sigue con su valor por defecto. Para este
  módulo, que tiene un límite de 512 MiB, es justo el ajuste que hay que hacer bien (ver H4).
- **Reproducción mínima**: cita del README de CanopyDB 0.2.5, líneas 151 a 155:

  ```rust
  let _dir = tempfile::tempdir().unwrap();
  let mut options = canopydb::EnvOptions::new(_dir.path());
  // all databases in the same environment will share this 1GB cache
  options.page_cache_size = 1024 * 1024 * 1024;
  let env = canopydb::Environment::new(_dir.path()).unwrap();
  ```

  `options` no se vuelve a usar. La forma que aplica el ajuste es `Environment::with_options(options)`.
- **Estado**: Detectado
- **Aprobaciones**: —
- **Enlaces**: —
- **Fork**: —

## H2 — Dos procesos no pueden abrir a la vez la misma base

- **Identificador**: H2
- **Fecha**: 2026-10-20
- **Versión de CanopyDB**: 0.2.5
- **Tipo**: Limitación
- **Descripción y efecto**: CanopyDB admite muchos lectores a la vez dentro de un mismo proceso, pero abrir una
  base toma un bloqueo exclusivo de su carpeta. Si otro proceso la tiene abierta, la apertura espera
  `file_lock_timeout` (5 segundos por defecto) y falla con `EnvironmentLocked`. Para el módulo, mientras un
  consultor CanopyDB tiene un conjunto abierto, otro consultor del mismo conjunto espera y se rechaza (decisión
  K6), y el verificador y las consultas de la verificación se ejecutan de uno en uno.
- **Reproducción mínima**: `cargo run -- h2` en [`repro/`](repro/). El proceso A abre la base y lanza el proceso
  B, que intenta abrirla. Resultado en el equipo de referencia (Windows 11) el 2026-10-21:

  ```text
  process B could not open the base after 5.0 s: EnvironmentLocked (EnvironmentLocked)
  process B ended with code Some(3) after 5.0 s
  REPRODUCED
  ```
- **Estado**: Detectado
- **Aprobaciones**: —
- **Enlaces**: —
- **Fork**: —

## H3 — No hay apertura de solo lectura

- **Identificador**: H3
- **Fecha**: 2026-10-20
- **Versión de CanopyDB**: 0.2.5
- **Tipo**: Limitación
- **Descripción y efecto**: CanopyDB no tiene un modo de apertura de solo lectura. Abrir una base para leerla abre
  sus archivos internos para escritura, así que una base en una carpeta de solo lectura no se puede abrir; y si
  la base se cerró sin volcar su registro de escritura anticipada, solo leerla reorganiza ese registro. Además, al
  abrir una base de datos con unas opciones distintas de las guardadas, CanopyDB reescribe su archivo `OPTIONS`
  con las del que abre. Para el módulo, el consultor CanopyDB puede escribir en los archivos internos del motor
  (decisión K6), el generador publica la base ya volcada y sin registro de escritura anticipada, y el verificador
  abre la base con las opciones guardadas para no cambiar nada.
- **Reproducción mínima**: `cargo run -- h3` en [`repro/`](repro/). Primero intenta leer una base cuyos archivos
  son de solo lectura; después abre solo para leer una base cerrada sin volcado y compara sus archivos antes y
  después. Resultado en el equipo de referencia (Windows 11) el 2026-10-21:

  ```text
  a) the read-only base cannot be opened: Io(Os { code: 5, kind: PermissionDenied, message: "Access is denied." }) (...)
  b) read 1000 entries without writing
  b) WAL-0000000000: 1048576 bytes -> 34121 bytes
  REPRODUCED
  ```

  La reescritura de `OPTIONS` se observó en la prueba previa del generador: abrir con las opciones por defecto
  una base escrita con otras cambió `OPTIONS` de 300 a 299 bytes (registro de la ejecución
  [`canopy-db-generador`](../../odd/tasks/canopy-db-generador.md), L3).
- **Estado**: Detectado
- **Aprobaciones**: —
- **Enlaces**: —
- **Fork**: —

## H4 — El tamaño por defecto de la caché no aparece en la portada de la documentación

- **Identificador**: H4
- **Fecha**: 2026-10-20
- **Versión de CanopyDB**: 0.2.5
- **Tipo**: Documentación
- **Descripción y efecto**: la caché de páginas crece bajo demanda hasta 1 GiB por defecto, pero la portada de la
  documentación (el README, que docs.rs muestra como página principal) no lo dice: solo lo menciona el comentario
  del ejemplo de H1, como si fuera un valor elegido. El dato condiciona cualquier presupuesto de memoria. En la
  prueba previa del generador, leer la base completa con las opciones por defecto llegó a un pico de 1.161 MiB,
  frente a 73 MiB con la caché limitada a 64 MiB (registro de la ejecución
  [`canopy-db-generador`](../../odd/tasks/canopy-db-generador.md), L3).
- **Reproducción mínima**: cita de la documentación de la opción en CanopyDB 0.2.5 (`EnvOptions::page_cache_size`,
  archivo `src/options.rs`), el único sitio que da el valor por defecto:

  ```text
  Database pages read from the Database files are cached in the page cache.

  Note that all databases in the same Environment share the page cache.

  Default: `1 GB`
  ```

  El README no menciona ningún valor por defecto de la caché.
- **Estado**: Detectado
- **Aprobaciones**: —
- **Enlaces**: —
- **Fork**: —
