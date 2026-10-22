# Consultor CanopyDB: implementado, verificado y aceptado

Informe de cierre de `canopy-db-consultor` (2026-10-22). Enfoque de ejecución: ODD.
Fuente de producto: [canopy-db-consultor.prd.md](../../docs/prd/chicago-crime/canopy-db-consultor.prd.md).
Documento de ejecución: [canopy-db-consultor.md](canopy-db-consultor.md).

Rama `feat/canopy-db-consultor` en la copia de trabajo `C:\wt\chicago-crime-canopy-db-consultor`, con 11 commits
locales desde `a78a773` (el primero es el del PRD) más el de este informe. No se ha fusionado en la base ni se
ha subido nada: la fusión espera a que el desarrollador diga `Fusiona el cambio en la base`.

## Resultado

- **Especificación:** S1–S17 del documento de ejecución, que recogen el PRD entero (KC1–KC9).
  - `crime-canopy-query` es el consultor original sobre la base que publica el generador CanopyDB: las mismas
    opciones, consultas, lote, salidas, mensajes y códigos.
  - Lo propio de CanopyDB:
    - la espera de 30 s, con su aviso, su rechazo y Ctrl+C;
    - el rechazo de otro motor, de otro esquema y de otra versión de CanopyDB;
    - los errores del motor con su línea de detalle;
    - `schema_version` como texto en el JSON;
    - el conjunto fijo F1–F11.
  - Pruebas:
    - Windows: `cargo test --workspace`, **560 pruebas en verde, 0 fallos**. La base tenía 529, así que hay
      31 nuevas.
    - Linux (Ubuntu 24.04.1 sobre WSL2, clon nuevo de la rama en `64c94a2`): **562 en verde, 0 fallos**, en 61
      binarios de prueba.
    - fmt y clippy, limpios.
- **Aceptación:** sobre el extracto de referencia, en el commit `d576c40`:
  - `run-canopy-query-acceptance.ps1`: **151 de 151 comprobaciones en PASS**, en 22 min 11 s.
  - `run-query-acceptance.ps1` (consultor original, sin cambios en su uso): **106 de 106 en PASS (36
    criterios)**, el mismo recuento que antes del cambio (AC-KC8).
  - Los informes están en `work/acceptance/canopy-query-acceptance-report.md` y
    `work/acceptance/query-acceptance-report.md`.
  - Cifras principales:

    | Medida | CanopyDB | Original (SSTables) |
    |---|---|---|
    | Pico de memoria: contar `THEFT` / contar `RITUALISM` / lote F1–F11 (AC-M8) | 2,1 / 1,0 / 2,4 MiB (cociente 2,53) | 14,5 / 7,6 / 19,3 MiB (cociente 2,55) |
    | F11, los 1.838.502 robos en CSV | 10,5 s | 67,0 s |
    | Consulta por ID, proceso entero (AC-C7) | 22–25 ms; la primera, en frío, 514 ms | — |
    | Rechazo con el conjunto abierto por otro proceso (AC-M5) | dentro de los 30–35 s tras el aviso; Ctrl+C sale con 130 en 0,04 s | — |
    | Archivos nuevos tras todas las consultas (AC-M6) | ninguno; manifiesto idéntico; las seis tablas iguales según el verificador | — |
    | Publicación de otra versión a mitad de lote (AC-M7) | todas las consultas leen la versión que abrió el lote | — |

  - Es el primer pico de memoria de un lote medido con el consultor original, como pedía K8, y cumple el
    cociente.
- **Revisiones y verificación:**
  - **Verificador independiente** (sonnet, high) sobre `a78a773..d576c40`, con compilaciones de la base y de
    HEAD en una copia aparte: sin bloqueos.
    - El consultor original da salidas idénticas a la base en 40 comandos (ítem de riesgo 3).
    - El nuevo coincide con el original en 40 consultas.
    - Los 34 casos de manifiesto y de base dan su mensaje exacto.
    - La espera rechaza a los 30,17 s.
    - Los errores del motor salen con su línea de detalle.
  - **Comprobación diferencial propia** de la T1: 16 comandos del consultor original (ayuda, consultas,
    errores, lote y archivo de salida) idénticos a un binario de `a78a773`.
  - **RDD:** cinco revisiones, todas aprobadas (ver *Revisiones RDD*). Una sexta no llegó a empezar por el
    tamaño del tramo.
- **Hallazgos corregidos por el camino:**
  - **Memoria (AC-M8).** La medida sobre el CSV completo antes de la aceptación dio un cociente de 9,9 con una
    caché de páginas de 8 MiB: contar un tipo raro usa 1 MiB y contar `THEFT` llenaba la caché. Con 1 MiB el
    cociente baja a 2,3, y contar `THEFT` pasa de 0,43 a 0,51 s (`f33a37b`).
  - **Margen de la espera (AC-M5).** El agente del script vio el rechazo 30,005 s después del aviso: el retraso
    de la tubería podía dejarlo por debajo de 30 s. Se añadieron 100 ms al plazo (`c18fe80`).
  - **Comprobación de AC-M7.** Por la precedencia de la coma en PowerShell, nunca se miraba el F2 de la segunda
    ronda. Se corrigió en `2e60df3` y se comprobó a mano sobre las salidas de la ejecución.
  - **Pruebas.** Encontraron dos defectos de las propias utilidades de prueba: un borrado de tabla sin volcado
    a disco y un enmascarado de tiempos que se comía el aviso.
  - **Revisiones RDD 3 y 4.** Ver la tabla.

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-KC1 (AC-M1) | Aceptación: filas AC-C1–AC-C18, AC-S1–AC-S12 y del lote con el consultor CanopyDB; `compare::every_query_gives_the_same_results_as_the_original_app` | PASS |
| AC-M2 | Aceptación `AC-M2-F1` … `AC-M2-F11`; `compare::*` | PASS |
| AC-M5 | Aceptación AC-M5 (cuatro filas, Ctrl+C real); `wait::*` (5 pruebas) | PASS |
| AC-M6 | Aceptación AC-M6; `compare::querying_adds_no_file_and_keeps_the_manifest` | PASS |
| AC-M7 | Aceptación AC-M7 (dos filas; la segunda revisada a mano, ver L13); `compare::a_batch_gives_the_same_results_as_the_original_app` | PASS |
| AC-M8 | Aceptación `AC-M8` y `AC-M8 (SSTables)` | PASS, cocientes 2,53 y 2,55 |
| AC-KC2 | Aceptación AC-KC2 (16 casos); `open::*`, pruebas unitarias de `manifest` | PASS |
| AC-KC3 | Aceptación AC-KC3 (2 casos); `open::a_dataset_of_another_canopydb_version_is_rejected_before_opening_the_base` | PASS |
| AC-KC4 | Aceptación AC-KC4 (4 casos); `open::a_base_without_one_of_the_six_tables_…` (la tabla que falta solo en la prueba de Rust) | PASS |
| AC-KC5 | Aceptación AC-KC5 (solo lectura y base dañada, sueltas y en lote); `engine::*`; pruebas unitarias de `guarded` | PASS |
| AC-KC6 | Aceptación AC-KC6; `compare::the_json_output_shows_the_manifest_schema_as_text` | PASS |
| AC-KC7 | Aceptación AC-KC7 (regla NOT MET y asociaciones inválidas) | PASS |
| AC-KC8 | `cargo test --workspace` en Windows (560) y Linux (562); aceptación del original 106/106 | PASS |
| AC-KC9 | `open::the_help_is_the_original_one_with_the_name_the_description_and_the_wait`; README *CanopyDB query app* | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Lineage |
|---|---|---|---|---|
| `ff15a0d..428a81c` (base `a78a773`) | readability, reliability | Aprobada y confirmada | R2-parse-line-hardcoded-app WARNING: documentado en `7728a7d`. R2-stale-derive-name SUGGESTION: comentario en `7728a7d`. R3-open-failed-untested y R3-help-note-untested WARNING: cubiertos por las pruebas del crate nuevo en `7728a7d`. R3-key-panics SUGGESTION: contrato de `KeyCursor::key` documentado en `7728a7d`. | `review-f93504240a83ecef` |
| `428a81c..7728a7d` | risk, resilience, readability, reliability | Aprobada y confirmada | R4-wait-retry-vanished-base WARNING: la apertura se reinicia si la versión desaparece durante la espera (`894dc17`). R2-001 WARNING, comentario huérfano en el generador: quitado. R2-002 WARNING, sufijo `_l` sin explicar: comentado. R2-003 SUGGESTION, dos listas de tablas: `table_names`. R2-004 SUGGESTION, alias `tree_name`: quitado. R3-wait-timing-flaky WARNING: límite ampliado. R3-base-unchanged-after-successful-query WARNING: no se cambia (K6 permite cambios en los archivos internos y AC-M6 compara el contenido lógico). | `review-5e50a20a0d62138a` |
| `7728a7d..894dc17` | resilience, reliability, readability | Aprobada y confirmada | R2-002 y R2-003 WARNING, "su criterio de memoria" ambiguo en las guías de Linux: reescrito en `64c94a2`. R3-wait-bound-loosened WARNING, R4 y R2-005 SUGGESTION, límite de 3 s: 2 s con su motivo. R2-001 y R3 SUGGESTION, `table_names` de tamaño fijo: tamaño desde `IndexKind::ALL`. R2-004 SUGGESTION, el archivo de consultas se empareja por posición: dicho en el archivo. R4-page-cache-latency SUGGESTION: no se cambia (coste medido de 0,08 s, documentado en el código). | `review-f0b1d5826e7e64b1` |
| `894dc17..64c94a2` (último tramo) | readability, reliability | Aprobada y confirmada | R2-wait-upper-bound-comment y R3-001 WARNING, el comentario del límite de 2 s se lee como argumento para más margen. R2-table-names-doc-drift y R3-002 SUGGESTION, la doc de `table_names` dice "seis". No se cambian: serían otro tramo solo por redacción. | `review-b3a4137d91e6ed01` |

Un intento sobre `428a81c..d576c40` (unas 4.000 líneas, cuatro lentes) fue rechazado con
`lens_context_budget_exceeded` antes de crear nada. Como la herramienta revisa siempre del límite a HEAD, se
revisó por tramos, con HEAD situado de forma temporal en `7728a7d` para el segundo.

## Reutilización

- **Consultor original entero.** El crate nuevo no repite nada: el consultor original se reorganizó en `ab526a7`
  para leer el conjunto a través de un rasgo, `Source`, y para recibir el nombre, la ayuda y la apertura como
  una `App`, con un `main` compartido.
  - El motor nuevo solo implementa tres lecturas primitivas (`Tables`): un valor de la tabla principal, una
    clave de un índice y un cursor de claves.
  - Recibe el resto de `TablesSource`: las claves, la decodificación de registros, los recorridos por valor,
    prefijo y rango, la resolución de consultas, las salidas y el lote.
- **Contrato compartido.** Los nombres de la forma CanopyDB, la versión de CanopyDB, la lista de tablas y la línea
  de detalle de un error del motor pasaron del generador CanopyDB a `crime_dataset::canopy`. Los usan los dos.
- **Pruebas.** Las del crate nuevo reutilizan las utilidades y las filas de las pruebas del consultor original
  (`#[path]`) y la canalización del generador original para publicar con CanopyDB.
- **Aceptación.** `run-query-acceptance.ps1` recibe ganchos opcionales y el modo CanopyDB reutiliza todas sus
  comprobaciones. Sin `-CanopyDb` hace lo mismo que antes, como prueba su ejecución 106/106.
- **Separado a propósito.** El verificador CanopyDB tiene su propio lector de la base y sus propias constantes,
  porque debe ser independiente de las aplicaciones que comprueba. El consultor tiene el suyo por la misma
  razón.

## Tiempo

- Estimación: 2 h 30 min – 4 h. Inicio real: 2026-10-22 06:37:04 +01:00. Fin: 2026-10-22 08:10:33 +01:00.
- **Duración activa desde el arranque: 1 h 33 min**, el 62 % del mínimo estimado. Lo que más pesó:
  - *Código y tests* (40 min);
  - *Verificación independiente* (34 min), que en el registro incluye la espera a la aceptación, lanzada en
    segundo plano mientras el verificador trabajaba.
- Esperas por el usuario: ninguna. El tramo de 4 s marcado como espera fue un error de marca corregido en el
  momento.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 2 min 55 s | 3 % | 1 |
| Plan (documento de ejecución) | 2 min 02 s | 2 % | 1 |
| Código y tests | 40 min 19 s | 43 % | 3 |
| Checks (fmt, clippy, suite de tests) | 3 min 43 s | 4 % | 1 |
| Verificación independiente | 34 min 02 s | 37 % | 1 |
| Revisión RDD | 6 min 19 s | 7 % | 4 |
| Correcciones de hallazgos | 3 min 54 s | 4 % | 2 |
| **Total activo** | **1 h 33 min** | **100 %** | **13** |

- Inicio: 2026-10-22 06:37:15 +01:00. Fin: 2026-10-22 08:10:33 +01:00.
- Arranque confirmado: 06:37:15. Activo desde el arranque: **1 h 33 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 0 min 04 s.
  - 07:15:53–07:15:57 (0 min 04 s): T4 writer

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD 1 reviewers | 06:51:30 | 06:52:38 | 1 min 08 s |
| Checks (fmt, clippy, suite de tests) | full datasets for memory | 07:04:59 | 07:08:23 | 3 min 24 s |
| Aceptación | CanopyDB and original query acceptance | 07:22:27 | 07:57:02 | 34 min 35 s |
| Revisión RDD | RDD 3 reviewers | 07:58:44 | 07:59:53 | 1 min 09 s |
| Revisión RDD | RDD 4 reviewers | 08:03:25 | 08:04:49 | 1 min 24 s |
| Checks (fmt, clippy, suite de tests) | final checks Windows and Linux | 08:07:19 | 08:10:27 | 3 min 08 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **6 min 32 s** |
| **Aceptación (total)** | | | | **34 min 35 s** |
| **Revisión RDD (total)** | | | | **3 min 41 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `ff15a0d` | docs(prd): add the canopy-db-consultor PRD |  | 0 min 00 s |  |
| `ab526a7` | refactor(query): read the dataset through a storage-neutral source and app | T1 | 12 min 49 s | Exploración y mapa 2 min 55 s, Plan (documento de ejecución) 2 min 02 s, Código y tests 7 min 52 s |
| `428a81c` | refactor(dataset): share the CanopyDB dataset names and engine detail line |  | 0 min 00 s |  |
| `7728a7d` | feat(canopy-query): add the CanopyDB query app | T1, T3 | 14 min 38 s | Código y tests 13 min 10 s, Revisión RDD 1 min 28 s |
| `f3d1d46` | docs: document the CanopyDB query app and annotate its PRDs | T3 | 2 min 12 s | Código y tests 2 min 12 s |
| `f33a37b` | fix(canopy-query): use a 1 MiB page cache to keep the memory ratio | T3 | 8 min 50 s | Código y tests 8 min 50 s |
| `c18fe80` | fix(canopy-query): reject a held base just after the wait, not before | T3, T4 | 6 min 30 s | Código y tests 6 min 30 s |
| `d576c40` | test(acceptance): add the CanopyDB query app's acceptance check |  | 0 min 00 s |  |
| `2e60df3` | fix(acceptance): check the second round's F2 version in AC-M7 | T4, T6 | 35 min 39 s | Código y tests 1 min 45 s, Verificación independiente 33 min 54 s |
| `894dc17` | fix(canopy-query): address the review of the CanopyDB query app | T6, T3 | 4 min 55 s | Verificación independiente 0 min 08 s, Revisión RDD 2 min 15 s, Correcciones de hallazgos 2 min 32 s |
| `64c94a2` | fix: address the review of the CanopyDB query app's checks and docs | T3, T4 | 2 min 56 s | Revisión RDD 2 min 00 s, Correcciones de hallazgos 0 min 56 s |
| — | después del último commit | T4, last slice, T6 | 4 min 45 s | Checks (fmt, clippy, suite de tests) 3 min 43 s, Revisión RDD 0 min 36 s, Correcciones de hallazgos 0 min 26 s |
| **Total** | | | **1 h 33 min** | |

Las dos fases grandes se solapan con trabajo en segundo plano:
- *Código y tests* incluye la redacción del README y las anotaciones de los PRD (T5), y la medición de memoria
  que llevó a `f33a37b`.
- La T4 (el script de aceptación) la escribió un agente en paralelo durante 39 min, sin sumar al activo.
- La aceptación (34 min 35 s) corrió mientras el verificador comprobaba la rama.

## Informe de cambios (`a78a773..64c94a2`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/crime-canopy-query` | 2.037 | 0 | 1.671 | 194 | 12 |
| `scripts` | 1.707 | 90 | 1.331 | 228 | 5 |
| `docs` | 906 | 9 | — | — | 7 |
| `crates/crime-query` | 539 | 168 | 393 | 93 | 12 |
| `crates/crime-dataset` | 100 | 1 | 56 | 32 | 2 |
| `README.md` | 52 | 0 | — | — | 1 |
| `crates/crime-canopy-generator` | 8 | 40 | 6 | 2 | 1 |
| **Total** | **5.349** | **308** | **3.457** | **549** | **40** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+14 −0)

`docs` incluye el PRD y su traducción (unas 880 líneas). No hubo previsión de líneas.

## Resumen

El analista puede consultar el conjunto que publica el generador CanopyDB con `crime-canopy-query`, igual
que el original: las mismas consultas, el mismo lote y los mismos registros. Lo propio de CanopyDB:
- espera hasta 30 s si otro proceso tiene la base abierta, avisándolo, y Ctrl+C la corta;
- rechaza un conjunto SSTable, otro esquema o una base escrita con otra versión de CanopyDB, sin llegar a
  abrirla;
- nunca crea archivos, bases ni tablas;
- muestra los errores del motor con su detalle.

El conjunto fijo F1–F11 y la verificación de aceptación comparan los dos consultores; los reutilizará el banco
de comparación.

Decisiones técnicas:
- **El consultor original no se duplica.** Lee a través de un rasgo, y CanopyDB solo aporta tres lecturas
  primitivas.
- **Recorridos por páginas.** Los índices se leen en páginas de 1.024 claves con un árbol abierto solo durante
  cada página, porque CanopyDB no deja abrir dos veces un árbol en la misma transacción.
- **Una sola transacción de lectura** sirve toda la ejecución.
- **Caché de páginas de 1 MiB.** Es memoria privada del proceso y cumple el cociente de AC-M8.

## Decisiones tomadas por defecto sin preguntar

- **Línea de detalle.** Sigue la regla de KG7, como dice KC5: cada grupo de saltos de línea pasa a un solo
  espacio. La fila del PRD decía un espacio por cada carácter. El PRD quedó anotado en las dos versiones
  (`f3d1d46`).
- **Caché de páginas de 1 MiB.** Se eligió midiendo, porque con 8 MiB no se cumplía AC-M8. La configuración
  queda a criterio de la implementación por el PRD.
- **Margen de 100 ms** sobre los 30 s de espera, dentro de la ventana de 30–35 s de AC-M5.
- **Un archivo `OPTIONS` ilegible o sin sentido** da `No se pudo leer…` sin línea de detalle: lo lee la
  aplicación, no el motor, igual que un manifiesto ilegible.

## Pendiente (no bloqueante)

- **AC-KC4, tabla que falta.** Solo lo cubre la prueba de Rust: el script de aceptación no puede fabricar una
  base sin una tabla.
- **Versión que desaparece durante la espera.** La apertura se reinicia (`894dc17`), pero no se puede
  provocar en Windows, donde los archivos abiertos no se borran. Le corresponde a `canopy-db-robustez`.
- **Fallo del motor a mitad de un listado sin `--output`.** Deja en la salida estándar la parte ya escrita,
  igual que el consultor original (C8 solo protege los archivos de salida).
- **Ctrl+C real.** El verificador independiente no pudo enviarlo desde Git Bash. Lo cubre la aceptación
  (AC-M5, 130 en 0,04 s) y las pruebas del crate.
- **Hallazgos de redacción de la última revisión** (comentario del límite de 2 s y "seis" en la doc de
  `table_names`): sin corregir para no abrir otro tramo.
- **Informe de la aceptación.** Su cabecera dice "with uncommitted changes": era solo el documento de
  ejecución, aún sin commit.
- **Lo que no corresponde a este hijo.** El banco de comparación (`canopy-db-comparacion`) y los escenarios de
  robustez (`canopy-db-robustez`) quedan para sus hijos.

**Riesgo:** ítem 3, porque el consultor original se reorganizó sobre un rasgo compartido. Las garantías:
- sus pruebas no cambiaron y pasan;
- su aceptación da 106/106;
- dos comprobaciones diferenciales (16 y 40 comandos) son idénticas a la base.

## Limpieza manual

- `work/acceptance` (13 GB) y `work/manual` (3,6 GB) en la copia de trabajo, ignorados por git: conjuntos y
  salidas de la aceptación y de la medición de memoria.
- `/tmp/tmp.zcK6f1ScAQ` (4,2 GB, verificador independiente), `/tmp/tmp.w0wTqPmAK9` (138 MB, compilación de la
  base) y `/tmp/tmp.nFJmietVvY` (4,4 MB, CSV de muestra). En Windows son
  `C:\Users\jesus.lopez\AppData\Local\Temp\tmp.*`.
- Archivos `/tmp/rdd*` (36, salidas de las revisiones RDD).
- El scratchpad de la sesión (108 MB: bundle de la rama, utilidades y datos del agente del script):
  `C:\Users\JESUS~1.LOP\AppData\Local\Temp\claude\c--wt-chicago-crime-canopy-db-consultor\71fbd401-ac30-467d-8555-d070e19606b9\scratchpad`.
- `~/canopy-db-consultor` en Ubuntu (WSL), 2,5 GB: el clon para las pruebas de Linux.
