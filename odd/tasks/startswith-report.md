# Búsqueda por prefijo en el consultor: implementada, verificada y sin fusionar

Informe de cierre de `startswith` (2026-10-18). Enfoque de ejecución: ODD.
Fuente de producto: [startswith.prd.md](../../docs/prd/chicago-crime/startswith.prd.md) v1.0. Documento de
ejecución: [startswith.md](startswith.md).

Rama `feat/startswith-odd` en la copia `C:\wt\chicago-crime-startswith-odd`, siete commits locales desde
`0971a37` (este informe incluido). No se ha fusionado en la base ni se ha subido nada: en un worktree se fusiona
solo cuando el desarrollador lo pide.

## Resultado

- **Especificación:** S1–S10 del documento de ejecución entregadas. `cargo test --workspace`: 453 tests en
  verde (425 al empezar, +28). `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D
  warnings` limpios sobre `d6e3ae6`.
- **Aceptación:** `run-query-acceptance.ps1` sobre el CSV de referencia (8.656.904 registros): **91 de 91
  comprobaciones en PASS, 30 criterios** (AC-C1…AC-C18 y AC-S1…AC-S12). La corrida usó los binarios de
  `c190bef`. El commit posterior `d6e3ae6` es una refactorización sin cambio de comportamiento, cubierta por
  la suite de tests, pero no pasó por una nueva corrida de aceptación.
- **Revisiones y verificación:** dos revisiones RDD, las dos aprobadas (ver la sección RDD). No hubo
  verificador independiente: ningún commit toca un elemento de alto riesgo.
- **Hallazgos corregidos por el camino:** los tres hallazgos de legibilidad de la primera revisión se
  corrigieron en `d6e3ae6`. Los tests no encontraron defectos de producto; solo dos fallos de los propios tests
  nuevos (formato de fecha del helper `at()` y un tipo que clippy marcó como complejo).

## Cobertura de los criterios de aceptación

| Criterio | Comprobación | Resultado |
|---|---|---|
| AC-S1 | `--startswith "Primary Type=crim\|Crim\|CRIM" --count` contra el selector; listado CSV | PASS: 1.256.646 en las tres; el tipo sale en mayúsculas |
| AC-S2 | `--startswith "Case Number=JA" --count`, sola | PASS: 269.655, `Índices usados: Case Number.` |
| AC-S3 | `--where "Primary Type=THEFT" --startswith District=01 --count` | PASS: 813.623 = selector; `Índices usados: Primary Type, District.` |
| AC-S4 | Location Description `resid` con un mes; y sola | PASS: = selector, `Filtros sin índice: Location Description.`; sola, mensaje de C10 y código 1 |
| AC-S5 | `CRIM` y `CRIMINAL D` | PASS: 1.256.646, igual que `CRIM` solo |
| AC-S6 | `--where THEFT` + `--startswith CRIM` | PASS: 3.095.148 = selector = cifra del PRD |
| AC-S7 | District `8` y `00` | PASS: `8` da 0 sin error ni aviso; `00` = selector |
| AC-S8 | Pico de memoria de `H` y de `JK438266` | PASS: 15,2 MiB (5.762.199 resultados) frente a 7,5 MiB (1); cociente 2,02, límite 3 |
| AC-S9 | Tabla, CSV y JSON del mismo prefijo; condición en el JSON | PASS: 114 registros iguales; `"startswith":["crim"]` |
| AC-S10 | `--limit 100` con CRIM | PASS: 100 distintos, todos del resultado sin límite |
| AC-S11 | Cada fila de la tabla de validaciones | PASS: mensaje exacto y código de C6 en cada una |
| AC-S12 | AC-C1…AC-C18 repetidos | PASS: 58 filas AC-C, ninguna fuera de PASS |

Tests de la suite que cubren las reglas: `tests/query.rs` (11 nuevos: alternativas, minúsculas, filtros,
mensajes, orden, JSON), `tests/cli.rs` (3), `tests/exec.rs` (6, entre ellos un contraste aleatorio de 80
consultas con prefijos contra el filtrado directo de las filas) y `crime-acceptance/tests/select.rs` (8).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `0971a37..c190bef` | resilience, readability, reliability | aprobada | R2-001 WARNING: `on_record` mutado con `remove(0)` para elegir el conductor; R2-002 SUGGESTION: `disjoint_runs` dependía sin decirlo de prefijos distintos; R2-003 SUGGESTION: `Equality::matches` repetía `Value::accepts`. Los tres corregidos en `d6e3ae6` | `review-de594d3b5bfc6638` |
| `c190bef..d6e3ae6` | readability, reliability | aprobada | R2-001 SUGGESTION: `Value::from_index` crea un `String` por valor y registro al comprobar en el registro; R3-001 SUGGESTION: `disjoint_runs` sigue dependiendo de prefijos distintos (ahora documentado; el `Builder` los deduplica). Quedan como mejoras pendientes | `review-f5c0d1972beb4cb1` |

Evaluaciones sin revisión: `55e0278` medium/under_budget; el tramo `0971a37..c190bef` medium/under_budget, que se
revisó por la regla de cierre del proyecto. Consentimiento concedido en las dos según esa regla.

## Reutilización

- **Consulta:** el prefijo es una alternativa más de la `Equality` o del `Filter` del campo
  (`crates/crime-query/src/query.rs`), con la misma forma de texto (`index_text`) y los mismos mensajes de
  campo inexistente o sin condiciones. Se extrajeron `Builder::equality` y `Builder::filter`, que ahora usan
  igualdad y prefijo.
- **Plan:** el recorrido de fechas (`DateScan`) se generalizó en `Scan` con `Runs` (fechas o alternativas de un
  campo), así que el prefijo reutiliza las comprobaciones por ID (búsqueda binaria y consultas puntuales). La
  lectura de índice reutiliza `IdStream` con un límite nuevo `Bound::Prefix`.
- **Aceptación:** la sección AC-S reutiliza los ayudantes del script (`Invoke-Query`, `Start-Select`,
  `Test-Formats`, `Test-Rejection`, `Test-MemoryGroup`), con un parámetro `-Criterion` opcional. El selector
  comparte `add_alternative` entre `--where` y `--startswith`.

## Tiempo

- Estimación: 1–2 h. Inicio real: 2026-10-18 09:03:19 +0100. Fin de la verificación: 09:33:55.
- **Duración activa desde el arranque: 30 min 36 s**, frente a 1–2 h: la mitad del mínimo estimado, como en las
  ejecuciones anteriores. El escritor delegado (selector y script de aceptación) trabajó en paralelo con el
  código del consultor, y la aceptación y las revisiones corrieron de fondo. El tiempo del escritor no está en
  el registro de fases: corrió en paralelo y no se marcó como tarea de fondo.
- Esperas por el usuario: 18 s, de 09:03:01 a 09:03:19 (confirmar la estimación).

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 3 min 29 s | 10 % | 1 |
| Plan (documento de ejecución) | 3 min 42 s | 11 % | 1 |
| Código y tests | 8 min 33 s | 25 % | 2 |
| Checks (fmt, clippy, suite de tests) | 2 min 42 s | 8 % | 3 |
| Aceptación | 7 min 03 s | 21 % | 1 |
| Revisión RDD | 6 min 38 s | 19 % | 2 |
| Correcciones de hallazgos | 1 min 58 s | 6 % | 1 |
| **Total activo** | **34 min 05 s** | **100 %** | **11** |

- Inicio: 2026-10-18 08:59:32 +01:00. Fin: 2026-10-18 09:34:02 +01:00.
- Arranque confirmado: 09:03:19. Activo desde el arranque: **30 min 36 s** (compárese con la estimación); antes del arranque: 3 min 29 s.
- Esperas por el usuario (fuera del activo): 0 min 18 s.
  - 09:03:01–09:03:19 (0 min 18 s): estimate
- Pausas entre sesiones (fuera del activo): 0 min 07 s.
  - 2026-10-18 09:33:55 +01:00 – 2026-10-18 09:34:02 +01:00 (0 min 07 s)
- Fase abierta, no contada: Cierre (informe y merge) desde 09:34:02.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | run-query-acceptance (AC-C, AC-S) | 09:17:21 | 09:18:18 | 0 min 57 s |
| Revisión RDD | RDD reviewers | 09:20:40 | 09:21:12 | 0 min 32 s |
| Revisión RDD | RDD reviewers last slice | 09:25:11 | 09:25:35 | 0 min 24 s |
| Aceptación | run-query-acceptance (AC-C, AC-S) rerun | 09:18:33 | 09:32:47 | 14 min 14 s |
| **Aceptación (total)** | | | | **15 min 11 s** |
| **Revisión RDD (total)** | | | | **0 min 56 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `55e0278` | feat(query): accept --startswith prefix conditions in the query model | T1 | 11 min 40 s | Exploración y mapa 3 min 29 s, Plan (documento de ejecución) 3 min 42 s, Código y tests 4 min 29 s |
| `abcb963` | feat(query): resolve indexed prefixes with a scan of their index runs | T1, T2 | 4 min 04 s | Código y tests 4 min 04 s |
| `f159556` | feat(acceptance): add --startswith to the independent selector | T3 | 0 min 15 s | Checks (fmt, clippy, suite de tests) 0 min 15 s |
| `1a34a39` | test(acceptance): check AC-S1 to AC-S12 of the startswith PRD in the query acceptance | T3 | 0 min 10 s | Checks (fmt, clippy, suite de tests) 0 min 10 s |
| `c190bef` | docs: document --startswith in the README | T3, T5 | 0 min 22 s | Checks (fmt, clippy, suite de tests) 0 min 22 s |
| `d6e3ae6` | refactor(query): name the prefix driver and reuse the value comparison | T5, RDD slice, R2-001..003 | 6 min 42 s | Checks (fmt, clippy, suite de tests) 1 min 07 s, Revisión RDD 4 min 14 s, Correcciones de hallazgos 1 min 21 s |
| — | después del último commit | R2-001..003, RDD last slice, wait for acceptance run, final checks | 10 min 52 s | Checks (fmt, clippy, suite de tests) 0 min 48 s, Aceptación 7 min 03 s, Revisión RDD 2 min 24 s, Correcciones de hallazgos 0 min 37 s |
| **Total** | | | **34 min 05 s** | |

El código y los tests del consultor se llevaron una cuarta parte del activo. La aceptación (7 min en primer
plano esperando la corrida) y las revisiones RDD (6 min, sobre todo la preparación de sus transiciones) suman
el 40 %. La primera corrida de aceptación (57 s) se detuvo porque escribía fuera de `work` (ver Limpieza
manual). Los commits `f159556`, `1a34a39` y `c190bef` muestran pocos segundos porque su código lo escribió el
escritor en paralelo, fuera del registro.

## Informe de cambios (`0971a37..d6e3ae6`, sin este informe)

| Área | + | − | Archivos |
|---|---|---|---|
| `crates/crime-query` | 921 | 84 | 9 |
| `crates/crime-acceptance` | 231 | 5 | 3 |
| `scripts` | 167 | 14 | 1 |
| `odd` | 76 | 0 | 1 |
| `README.md` | 32 | 1 | 1 |
| **Total** | **1.427** | **104** | **15** |

No hay archivos generados ni lockfiles. No hubo previsión de líneas.

## Resumen

El analista puede pedir al consultor los delitos cuyo campo de texto empieza por un texto, con
`--startswith CAMPO=PREFIJO`, solo o junto a las demás condiciones: `CRIM` en tipo principal, `JA` en número de
caso, `01` en distrito. Sobre número de caso, tipo principal y distrito es una condición indexada que puede ir
sola; sobre IUCR, Location Description, Beat y FBI Code es un filtro. Los prefijos y los valores de un mismo
campo son alternativas, y ningún registro sale dos veces. Por dentro, todos los valores que empiezan por un
prefijo están seguidos en el índice, así que el consultor recorre ese tramo de claves con un solo iterador. Sin
fecha, el primer campo indexado con prefijo dirige el plan y las demás condiciones se comprueban por ID. La
memoria no crece con los valores que abarca el prefijo: `H` (5,76 millones de números de caso) usa 15 MiB.

## Valores por defecto elegidos sin preguntar

- D1: en el JSON, cada condición conserva `"values"` (vacío si solo hay prefijos) y añade `"startswith"` con los
  prefijos en minúsculas solo cuando los tiene. Las condiciones sin prefijo no cambian de forma.
- D2: orden de comprobación de `--startswith`: campo inexistente, campo sin condiciones, campo que no es de
  texto, prefijo vacío. El prefijo vacío sale con código 1 (dato de la consulta); solo la falta de `=` es
  código 2, como dice el PRD.
- D3: plan fijo: si hay fecha, dirige la fecha; si no, el primer campo indexado con prefijo, por tramos
  disjuntos de su índice; si no, la mezcla ordenada de siempre. Un campo indexado con prefijo que no dirige se
  comprueba sobre el registro, y las estadísticas lo cuentan entre los índices usados.
- D4: el aviso de número de caso inexistente sigue saliendo solo para los valores de igualdad (P4).
- D5: AC-S1…AC-S12 van en una sección nueva de `run-query-acceptance.ps1`, que reutiliza su conjunto y sus
  ayudantes.

## Pendiente (no bloqueante)

- `Value::from_index` crea un `String` por valor y registro cuando un campo indexado con prefijo se comprueba
  sobre el registro (sugerencia R2-001 de la segunda revisión). No es un defecto; solo cuesta en ese camino.
- `disjoint_runs` depende de que los prefijos sean distintos (sugerencia R3-001). El `Builder` los deduplica y
  el comentario lo dice.
- Tiempo medido (P5, sin objetivo): THEFT con distrito `01` tardó 23,4 s, porque el prefijo de distrito
  (3,39 millones de registros) dirige y THEFT se comprueba con una consulta puntual por ID. Los demás conteos con
  prefijo tardaron menos de 1 s. Un plan por costes, que el consultor dejó fuera de alcance, elegiría THEFT.
- La aceptación corrió sobre `c190bef`. `d6e3ae6` solo está cubierto por la suite de tests.

**Riesgo:** ninguno. Se añade una opción (`--startswith`) sin cambiar las existentes, y el plan de las consultas
sin prefijo es el de antes: lo fija AC-S12 (58 filas AC-C en PASS) y lo cubre la suite completa.

## Limpieza manual

- `C:\wt\chicago-crime-startswith-odd\workquery-acceptance` (787 MB, sin seguimiento en git): restos de la
  primera corrida de aceptación, que recibió `-WorkDir work\\query-acceptance` desde bash y escribió en la raíz
  de la copia. El agente no tuvo permiso para borrarla; se puede borrar entera.
- `C:\wt\chicago-crime-startswith-odd\work\query-acceptance` y `work\query-acceptance.log`: la corrida buena,
  dentro de `work` (ignorada por git). Su informe es `work\query-acceptance\query-acceptance-report.md`.
