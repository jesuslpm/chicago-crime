# Búsqueda por prefijo en el consultor: implementada, verificada y sin fusionar

Informe de cierre de `startswith` (2026-10-18). Enfoque de ejecución: vibe coding.
Fuente de producto: [PRD de búsqueda por prefijo](../prd/chicago-crime/startswith.prd.md). Documento de
ejecución: ninguno (vibe coding). Registro de fases: [2026-10-18-startswith-phases.tsv](2026-10-18-startswith-phases.tsv).

Rama `feat/startswith-vibe` en la copia `C:\wt\chicago-crime-startswith-vibe`, 8 commits locales sobre `0971a37` más
el de este informe. No se ha fusionado en la base (copia de worktree: se fusiona cuando el desarrollador lo pida)
ni se ha subido nada.

## Resultado

- **Especificación:** los 12 criterios del PRD (AC-S1 a AC-S12) entregados. `cargo test --workspace`: 447 tests
  en verde (425 de partida + 22 nuevos); `cargo fmt --all --check` y
  `cargo clippy --workspace --all-targets -- -D warnings` sin avisos.
- **Aceptación:** `run-query-acceptance.ps1 -WorkDir work/acceptance` sobre el CSV de referencia (8.656.904
  registros): **87/87 PASS** en la ejecución final (58 filas AC-C, 28 filas AC-S y la del conjunto sin rechazos).
  Todas las cifras fijas del PRD coinciden con el consultor y con el selector independiente.
- **Revisiones:** tres revisiones RDD, las tres aprobadas sin bloqueos (detalle abajo).
- **Hallazgos corregidos por el camino:**
  - La primera ejecución de aceptación dio 86/87: la fila "ningún registro empieza por el prefijo" de AC-S11
    usaba `Case Number=ZZ`, y el CSV de referencia tiene 27 números de caso que empiezan por `ZZ` (confirmado con
    `crime-acceptance select`). El consultor acertaba; la comprobación estaba mal elegida. Se cambió a `~`
    (0 en el CSV, también contrastado con el selector) y la segunda ejecución dio 87/87.
  - Una ejecución anterior se lanzó con `-WorkDir work\\acceptance` desde bash, que la convirtió en
    `workacceptance` (fuera de la carpeta ignorada). Se detuvo y se relanzó en `work/acceptance`; la carpeta
    sobrante queda en *Limpieza manual*.

## Cobertura de los criterios de aceptación

Las filas "aceptación" son de `work/acceptance/query-acceptance-report.md` (ejecución final, 09:36–09:43).

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-S1 | Aceptación: `--startswith "Primary Type=crim/Crim/CRIM" --count` contra el selector; listado de 50 en CSV. Test `tests/exec.rs` `a_prefix_alone_finds_every_record_whose_value_starts_with_it` | PASS: las tres dan 1.256.646 (= selector = PRD); los tipos salen como en el CSV (`CRIM SEXUAL ASSAULT`) |
| AC-S2 | Aceptación: `--startswith "Case Number=JA" --count` sola | PASS: 269.655 (= selector = PRD); estadísticas "Índices usados: Case Number" |
| AC-S3 | Aceptación: `--where "Primary Type=THEFT" --startswith District=01`. Test `prefixes_combine_with_every_other_condition` | PASS: 813.623 (= selector); "Índices usados: Primary Type, District" |
| AC-S4 | Aceptación: `--startswith "Location Description=resid"` + mes de agosto de 2019; y la misma sola | PASS: 5.310 (= selector); "Filtros sin índice: Location Description"; sola, rechazo C10 con código 1 |
| AC-S5 | Aceptación: `CRIM` + `CRIMINAL D`. Test `overlapping_prefixes_and_values_of_one_field_return_each_record_once` | PASS: 1.256.646, igual que `CRIM` solo |
| AC-S6 | Aceptación: `--where "Primary Type=THEFT" --startswith "Primary Type=CRIM"` | PASS: 3.095.148 (= selector = PRD) |
| AC-S7 | Aceptación: `District=8` y `District=00`. Test `a_prefix_keeps_leading_zeros_and_never_finds_records_without_a_value` | PASS: `8` da 0, código 0, sin aviso; `00` da 4.077.929 (= selector) |
| AC-S8 | Aceptación: pico de memoria privada de `Case Number=H` y de `Case Number=JK438266` | PASS: 15,2 MiB (5.762.199 registros, = selector = PRD) frente a 7,5 MiB (1 registro); proporción 2,02 (regla ≤ 3, la de AC-C13) |
| AC-S9 | Aceptación: `--startswith "Case Number=JA1000"` en tabla, CSV y JSON. Tests `tests/binary.rs` `a_startswith_query_gives_the_same_records_in_every_format`, `tests/query.rs` `the_interpreted_query_shows_the_prefixes_lowercased` | PASS: 65 registros iguales en los tres formatos; el JSON lleva `{"field":"Case Number","indexed":true,"prefixes":["ja1000"]}` |
| AC-S10 | Aceptación: `--startswith "Primary Type=CRIM" --limit 100 --format csv`. Test `a_prefix_query_reports_its_index_or_filter_and_honours_the_limit` | PASS: 100 registros distintos, todos en los 1.256.646 ID del selector |
| AC-S11 | Aceptación: una fila por validación. Tests `tests/binary.rs` `every_startswith_validation_shows_its_message_and_exit_code`, `tests/query.rs` (validaciones), `tests/cli.rs` `a_startswith_condition_without_equals_sign_is_rejected` | PASS: prefijo vacío, campo no de texto, Block, campo inexistente y solo filtros sin índice con código 1; sin `=` con código 2; prefijo sin coincidencias (`~`) da 0, código 0 y sin aviso |
| AC-S12 | Aceptación: las 58 filas AC-C1 a AC-C18 en la misma ejecución; los 425 tests de partida | PASS: 58/58 y 425/425 |

Además, `the_plan_with_prefixes_returns_what_a_brute_force_filter_of_the_rows_returns` (`tests/exec.rs`) compara
80 consultas aleatorias con prefijos, igualdades, ID, fechas y filtros con un filtrado por fuerza bruta de 3.000
filas, y comprueba que ningún registro sale dos veces.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Lineage |
|---|---|---|---|---|
| `0971a37..26aae2b` (6 commits) | readability, reliability | aprobada | R2-1 SUGERENCIA: una línea del doc de módulo de `cli.rs` pasaba del ancho de línea → corregida en `b64fce2`. R2-2 SUGERENCIA: `Query::applied` es densa; extraer el reparto valores/prefijos de los filtros → no se cambió (ver *Pendiente*). Fiabilidad: ninguno | `review-683b3b6c7441ecb0` |
| `26aae2b..b64fce2` | readability | aprobada | ninguno | `review-aa1d647b1cf7ce2f` |
| `b64fce2..b3ba171` | reliability | aprobada | R3-001 ADVERTENCIA (informativa): el prefijo `~` supone que ningún número de caso empieza por `~`; en otro CSV podría fallar por los datos → anotado en *Pendiente* | `review-6625c364bc974c42` |

Cada commit se evaluó al hacerlo; todas las evaluaciones dieron `under_budget` (riesgo medio), así que las
revisiones se pidieron al cerrar, por tramos, y se acusaron una a una.

## Reutilización

- **Reutilizado tal cual:** `messages::unknown_field`, `field_not_conditionable` y `no_indexed_condition`
  (mismos mensajes que la igualdad, como pide el PRD); `index_text` para el minúsculas (C1); `record_value` y
  `Value::accepts` para comprobar sobre el registro; `IdStream` y su `seek` entre bloques; `Field::find`; en el
  script, `Test-Count`, `Start-Select`, `Read-CsvRecords`, `Get-Stats`.
- **Extendido:** `IdStream` con el límite `Bound::Prefix` y `Dataset::ids_with_prefix`; `Equality` con sus
  prefijos y `Equality::matches`; `Filter` y `Value` con `Value::Prefix`; `AppliedCondition` con `prefixes`;
  `messages::malformed_condition` recibe la forma (`CAMPO=VALOR` o `CAMPO=PREFIJO`); en el selector,
  `Wanted::Prefix`; en el script, `Test-Formats`, `Test-Rejection` y `Test-MemoryGroup` reciben el criterio en
  vez de llevarlo fijo.
- **Extraído:** `Builder::equality` y `Builder::filter` (el "buscar o crear" que antes estaba escrito dentro de
  `add_equal`), `Filter::add`, `Value::from_index` (sustituye a `applied_index_value`, que repetía la
  conversión), `split_at_equals` en la CLI y `Selection::add_alternative` en el selector.
- **Generalizado:** `DateScan` pasa a `Scan`, que recorre "tramos" disjuntos de un índice: intervalos de fechas,
  valores o prefijos.
- **Nuevo:** `runs_of` (tramos disjuntos de las alternativas de un campo: descarta prefijos y valores cubiertos
  por un prefijo más corto), `messages::prefix_not_on_text` y `empty_prefix`, la opción `--startswith`.
- **Separado a propósito:** el selector independiente (`crime-acceptance select`) vuelve a escribir la regla del
  prefijo en vez de reutilizar la del consultor: su valor es justamente no compartir código con él.

## Tiempo

- Estimación: 1 h 30 min – 2 h 30 min. Inicio real: 2026-10-18 09:06:32 (arranque confirmado). Fin: 09:43:55.
- **Duración activa desde el arranque: 37 min 23 s**, un 25–40 % de la estimación: más rápido aún que la mitad
  habitual. El código salió en verde casi a la primera (la capa de índice ya tenía el `seek` difícil resuelto) y
  la aceptación y las revisiones RDD corrieron en paralelo.
- Esperas por el usuario: 09:06:12–09:06:32 (20 s), la confirmación de la estimación.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 5 min 01 s | 12 % | 1 |
| Código y tests | 13 min 29 s | 32 % | 1 |
| Checks (fmt, clippy, suite de tests) | 1 min 24 s | 3 % | 1 |
| Revisión RDD | 8 min 01 s | 19 % | 1 |
| Correcciones de hallazgos | 7 min 14 s | 17 % | 1 |
| Cierre (informe y merge) | 7 min 15 s | 17 % | 1 |
| **Total activo** | **42 min 24 s** | **100 %** | **6** |

- Inicio: 2026-10-18 09:01:11 +01:00. Fin: 2026-10-18 09:43:55 +01:00.
- Arranque confirmado: 09:06:32. Activo desde el arranque: **37 min 23 s** (compárese con la estimación); antes del arranque: 5 min 01 s.
- Esperas por el usuario (fuera del activo): 0 min 20 s.
  - 09:06:12–09:06:32 (0 min 20 s): estimate

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | query acceptance | 09:19:44 | 09:24:20 | 4 min 36 s |
| Aceptación | query acceptance (rerun) | 09:24:53 | 09:35:59 | 11 min 06 s |
| Aceptación | query acceptance (run 2) | 09:36:45 | 09:43:45 | 7 min 00 s |
| **Aceptación (total)** | | | | **22 min 42 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `6d717a8` | feat(query): stream the IDs of every index value that starts with a prefix |  | 6 min 00 s | Exploración y mapa 5 min 01 s, Código y tests 0 min 59 s |
| `454e594` | feat(query): type startswith conditions as alternatives of equalities and filters |  | 2 min 30 s | Código y tests 2 min 30 s |
| `58c614a` | feat(query): resolve startswith conditions through index runs and record checks |  | 3 min 51 s | Código y tests 3 min 51 s |
| `a3e5bcc` | feat(query): add the --startswith option with its help and docs |  | 2 min 03 s | Código y tests 2 min 03 s |
| `0379c2e` | feat(acceptance): filter the CSV by prefix in the independent selector |  | 1 min 18 s | Código y tests 1 min 18 s |
| `26aae2b` | test(acceptance): check the startswith criteria AC-S1 to AC-S12 on the reference CSV |  | 2 min 16 s | Código y tests 2 min 16 s |
| `b64fce2` | docs(query): wrap the command line module doc to the line width |  | 7 min 48 s | Código y tests 0 min 32 s, Revisión RDD 7 min 16 s |
| `b3ba171` | fix(acceptance): use a case number prefix no record has for AC-S11 |  | 9 min 24 s | Checks (fmt, clippy, suite de tests) 1 min 24 s, Revisión RDD 0 min 45 s, Cierre (informe y merge) 7 min 15 s |
| — | después del último commit |  | 7 min 14 s | Correcciones de hallazgos 7 min 14 s |
| **Total** | | | **42 min 24 s** | |

Lectura de la tabla: el código y sus tests se llevaron un tercio; la revisión RDD y la aceptación corrieron
solapadas. Las etiquetas de los últimos tramos están corridas: la corrección de AC-S11 se hizo dentro del tramo
marcado "Cierre", y el tramo "Correcciones de hallazgos" cubre su revisión RDD, la segunda aceptación y este
informe. La primera fila de aceptación (09:19–09:24) es la ejecución lanzada en la carpeta equivocada; su fin
se anotó a mano al detenerla.

Tiempo real de las consultas por prefijo (P5, sin objetivo; ejecución final, línea de estadísticas salvo
donde se indica): `Case Number=JA` 0,085 s, `CRIM` 0,37–0,39 s, `CRIM` + `CRIMINAL D` 0,24 s, `THEFT` o
`CRIM` 0,57 s, `District=00` 0,72 s, `Case Number=H` 1,4 s (proceso completo), y `THEFT` + `District=01`
17 s (lee el registro de cada uno de los 1,8 millones de THEFT para comprobar el distrito).

## Informe de cambios (`0971a37..b3ba171`)

| Área | + | − | Archivos |
|---|---|---|---|
| `crates/crime-query` | 1.075 | 114 | 11 |
| `scripts` | 133 | 11 | 1 |
| `crates/crime-acceptance` | 81 | 3 | 3 |
| `README.md` | 12 | 0 | 1 |
| **Total** | **1.301** | **128** | **16** |

No hay lockfiles ni archivos generados. No hubo previsión de líneas.

## Resumen

El analista puede pedir `--startswith CAMPO=PREFIJO` sobre los siete campos de texto, solo o junto a las
condiciones que ya existían. Sobre número de caso, tipo principal y distrito es una condición indexada: el
consultor salta al primer valor del índice que empieza por el prefijo y lee hasta el primero que no, sin cargar
ID en memoria, así que `H` (5,7 millones de números distintos) usa la misma memoria que un prefijo de un solo
número. Las alternativas de un campo (igualdades y prefijos) se reducen a tramos disjuntos del índice para que
ningún registro salga dos veces. Como los ID de un prefijo no salen ordenados por ID, no entran en la
intersección ordenada: si hay otra condición indexada, el prefijo se comprueba sobre el registro; si no, el
prefijo guía el recorrido. Sobre los demás campos de texto es un filtro, como la igualdad. El generador y el
formato publicado no cambian.

## Valores por defecto elegidos sin preguntar

- **Forma del JSON:** la condición lleva `"prefixes": [...]` en minúsculas; `"values"` solo aparece si hay
  igualdades en ese campo, así que la salida JSON de las consultas existentes no cambia.
- **Location** (derivada de las coordenadas) da el mensaje de "solo campos de texto", no el de C3.
- **Orden de validación:** campo inexistente, después campo que no es de texto, después Block o Description
  (C3), después prefijo vacío.
- **Plan de ejecución** sin modelo de costes (como C10): con fecha, guía la fecha; con ID o una igualdad sin
  prefijos, guía la intersección ordenada; si solo hay prefijos indexados, guía el primero.
- Un número de caso pedido con `--where` que no existe sigue avisando aunque el mismo campo lleve prefijos; los
  prefijos nunca avisan (P4).

## Pendiente (no bloquea)

- **R2-2 (sugerencia RDD):** `Query::applied` podría extraer el reparto de valores y prefijos de un filtro en un
  helper. No se hizo: es legibilidad, no cambia el comportamiento, y queda cubierta por los tests del JSON.
- **R3-001 (advertencia RDD):** la fila de AC-S11 supone que ningún número de caso empieza por `~`. En otro CSV
  podría fallar por los datos; la fila compara además con el selector, así que el fallo diría claramente que hay
  registros con ese prefijo.
- **Coste de un prefijo combinado con otra condición indexada:** `THEFT` + `District=01` tarda 17 s porque lee
  el registro de cada THEFT. El PRD no fija tiempo (P5) y la memoria no crece; un plan por costes, fuera de
  alcance, podría elegir el recorrido más barato.

**Riesgo:** ninguno de la lista de alto riesgo. Añade una opción y un campo opcional en el JSON; la salida de
las consultas existentes no cambia (AC-S12: 58/58 filas AC-C y 425 tests de partida en verde).

## Limpieza manual

- `workacceptance\` en la raíz de la copia (2,8 GB, sin seguimiento y sin ignorar): restos de la ejecución
  lanzada con la ruta mal escrita. Se puede borrar entera; el agente no tenía permiso para hacerlo.
- `work\acceptance\` (3,3 GB, ignorada) y `work\acceptance-run.log`, `work\acceptance-run2.log`,
  `work\query-acceptance-report-run1.md`, `work\zz.txt`, `work\zz.count`, `work\tilde.count`: evidencias de la
  aceptación; se pueden borrar cuando ya no hagan falta.
