# Lote de consultas en el consultor: implementado, verificado y aceptado

Informe de cierre de `canopy-db-lote` (2026-10-21). Enfoque de ejecución: ODD.
Fuente de producto: [canopy-db-lote.prd.md](../../docs/prd/chicago-crime/canopy-db-lote.prd.md). Documento de
ejecución: [canopy-db-lote.md](canopy-db-lote.md).

Rama `feat/canopy-db-lote` en la copia de trabajo `C:\wt\chicago-crime-canopy-db-lote`, con 6 commits locales
desde `18ee4ba` más el de este informe. No se ha fusionado en la base ni se ha subido nada: la fusión espera a
que el desarrollador diga `Fusiona el cambio en la base`.

## Resultado

- **Especificación:** S1–S10 del documento de ejecución, que recogen el PRD entero (B1–B7). El consultor
  original admite `--dataset <CARPETA> --batch <ARCHIVO>`. Comprueba el archivo entero antes de abrir el
  conjunto, abre el conjunto una vez, ejecuta cada consulta como suelta bajo una cabecera en la salida de
  errores y termina con `Lote completado: <n> consultas en <t> s.`. `cargo test --workspace`: **474 pruebas
  en verde, 0 fallos**; la base tenía 453, así que hay 21 nuevas. fmt y clippy, limpios.
- **Aceptación:** `run-query-acceptance.ps1` sobre el CSV de referencia, en el commit `bbdf250`: **106 de 106
  comprobaciones en PASS (36 criterios)**. Incluyen AC-B1, AC-B2, AC-B4, AC-B5, AC-B8 con un Ctrl+C real y
  AC-B10, que exige que las 91 filas anteriores (AC-C y AC-S) sigan pasando.
  - La primera ejecución falló antes de la primera comprobación por un error mío al pasar la carpeta de
    trabajo; ver *Pendiente*.
  - El informe está en `work/acceptance/query-acceptance-report.md`.
- **Revisiones y verificación:**
  - **Verificador independiente** (sonnet, high) sobre `18ee4ba..6c1070d`: sin bloqueos. Hizo unas 120 pruebas
    con el binario y comparó 18 consultas sueltas con la base: todas idénticas.
  - **Comprobación diferencial propia:** 14 consultas sueltas dan la misma salida estándar, salida de errores,
    código y archivo que un binario compilado de `18ee4ba`. Solo cambian las dos líneas que usan `--batch`.
  - **RDD:** dos revisiones, las dos aprobadas (ver *Revisiones RDD*).
- **Hallazgos corregidos por el camino:**
  - Las pruebas encontraron dos defectos antes del commit:
    - `--count` contaba como escrito junto a `--batch`, porque clap le da un valor por defecto. Ahora solo
      cuentan los valores que vienen de la línea de comandos.
    - `--batch ""` daba el error de clap en vez de `No se puede leer el lote .`. Ahora el valor se lee como
      texto.
  - La revisión RDD 1 dejó tres correcciones menores, aplicadas en `1c4dd7e`.

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-B1 (AC-M7) | `each_query_runs_as_it_would_alone_and_the_batch_reports_every_one`; aceptación AC-B1 (todas las consultas que la verificación ejecuta sueltas, en un lote) | PASS |
| AC-B2 (AC-M7) | `every_query_reads_the_version_the_batch_opened`; aceptación AC-B2 (republicación con el lote detenido) | PASS |
| AC-B3 | `the_file_form_accepts_bom_crlf_comments_and_shell_like_quotes` y 4 pruebas unitarias de palabras, comentarios y rutas | PASS |
| AC-B4 | `a_wrong_line_rejects_the_batch_before_anything_runs`; aceptación AC-B4 (dos filas) | PASS |
| AC-B5 | `every_batch_rule_has_its_message_and_code`, `output_paths_are_compared_without_case_on_windows`, `a_dataset_that_can_no_longer_be_read_stops_the_batch`, `a_closed_stdout_stops_the_batch_quietly_and_another_failure_with_1`; aceptación AC-B5 (cuatro filas) | PASS |
| AC-B6 | `each_query_runs…`, `a_batch_of_one_query_ends_with_one_query_in_the_last_line` | PASS |
| AC-B7 | `a_query_that_fails_stops_the_batch_and_earlier_results_stay` (carpeta inexistente y ruta que es una carpeta) | PASS |
| AC-B8 | `a_cancellation_during_a_query_stops_the_batch_and_earlier_files_stay`, `…before_the_first_query…`, `…between_two_queries…`; aceptación AC-B8 con Ctrl+C real | PASS |
| AC-B9 | `the_help_describes_the_batch`; README, sección *Batches* | PASS |
| AC-B10 (AC-M9) | `cargo test --workspace` (474); aceptación AC-B10 (91 filas anteriores sin cambios) | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Lineage |
|---|---|---|---|---|
| `f8f60b6..bbdf250` (base `18ee4ba`) | resilience, readability, reliability | Aprobada y confirmada | R2-001 SUGGESTION, tabla de códigos incompleta en `main.rs`: corregida en `1c4dd7e`. R2-002 y R3-001 WARNING, `Get-UnlimitedIds` del script de aceptación solo conoce las consultas limitadas de THEFT y CRIM (acoplamiento oculto con AC-B1): documentado en los dos sitios en `1c4dd7e`. R2-003 SUGGESTION, `split_words` exportada sin usos: pasa a privada en `1c4dd7e`. R4-001 SUGGESTION, la comprobación de salidas repetidas es cuadrática en el número de `--output`: no se corrige (el límite de tamaño del lote está fuera de alcance; despreciable en lotes reales). | `review-1f8103dcc942fb9b` |
| `1c4dd7e..4828ba8` (base `bbdf250`) | readability, reliability | Aprobada y confirmada | R2-001 SUGGESTION, la anotación de C6 solo nombra el código 2: es el texto que fija el PRD. R2-002 SUGGESTION, el comentario del script nombra las consultas limitadas de hoy. R2-003 SUGGESTION, falta una nota de por qué `split_words` es privada. R3-001 WARNING, quitar la exportación podría romper a otro usuario: no existe ninguno (el workspace compila y pasan sus 474 pruebas). Ninguno se corrige. | `review-7fc125509a265fd3` |

La primera consulta de estado de la revisión 1 agotó su tiempo mientras corría la aceptación; se reintentó al
terminar esta. La evaluación del último tramo dio riesgo medio y bajo el umbral, y se revisó igualmente por
la regla de cierre del proyecto.

## Reutilización

- **Ejecución de una consulta:** `run` se partió en la comprobación de la consulta y `run_query`
  ([run.rs](../../crates/crime-query/src/run.rs)), que ejecuta una consulta ya comprobada sobre un conjunto
  abierto. El lote llama a esa misma función, así que sus resultados, avisos, estadísticas, archivo de salida,
  cancelación y tubería cerrada son los de la consulta suelta.
- **Análisis de una línea:** `cli::parse_line` pasa las palabras por el mismo parser de clap y la misma
  validación que la línea de comandos ([cli.rs](../../crates/crime-query/src/cli.rs)). Las condiciones pasan
  por `Query::build`. No hay reglas paralelas.
- **Formato del tiempo:** se extrajo `seconds` de la línea de estadísticas
  ([messages.rs](../../crates/crime-query/src/messages.rs)) y lo usa también la línea final del lote.
- **Código nuevo:** [batch.rs](../../crates/crime-query/src/batch.rs), con la lectura del archivo, la
  separación en palabras, la comparación léxica de rutas, la comprobación previa y el bucle del lote.
- **Aceptación:** reutiliza las ayudas del script: `Invoke-Query`, la retención de la salida estándar de AC-C12
  para AC-B2 y `send-ctrl-c.ps1` para AC-B8.

## Tiempo

- Estimación: 1 h 30 min – 3 h. Arranque real: 2026-10-21 07:58:20 +0100. Fin: 2026-10-21 08:30:09 +0100.
- **Duración activa desde el arranque: 31 min 37 s.** Es cerca de un tercio del mínimo estimado, como en las
  ejecuciones anteriores, que acabaron en la mitad o menos. La mayor parte se fue en las revisiones RDD
  (12 min 36 s, el 40 %) y en el código con sus pruebas (8 min 18 s). Dos cosas acortaron la ejecución: el
  script de aceptación se escribió en paralelo con un escritor delegado, y el verificador y la aceptación
  corrieron en segundo plano.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 1 min 56 s | 6 % | 1 |
| Código y tests | 8 min 18 s | 26 % | 1 |
| Checks (fmt, clippy, suite de tests) | 2 min 03 s | 6 % | 1 |
| Aceptación | 2 min 12 s | 7 % | 1 |
| Revisión RDD | 12 min 36 s | 40 % | 4 |
| Correcciones de hallazgos | 1 min 17 s | 4 % | 1 |
| Documentación | 2 min 52 s | 9 % | 1 |
| Cierre (informe y merge) | 0 min 23 s | 1 % | 1 |
| **Total activo** | **31 min 37 s** | **100 %** | **11** |

- Inicio: 2026-10-21 07:58:32 +01:00. Fin: 2026-10-21 08:30:09 +01:00.
- Arranque confirmado: 07:58:32. Activo desde el arranque: **31 min 37 s** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | query-acceptance | 08:14:27 | 08:15:53 | 1 min 26 s |
| Aceptación | query-acceptance-2 | 08:16:18 | 08:25:03 | 8 min 45 s |
| **Aceptación (total)** | | | | **10 min 11 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `f8f60b6` | docs(prd): add the canopy-db-lote PRD for query batches | T1 | 2 min 02 s | Exploración y mapa 1 min 56 s, Código y tests 0 min 06 s |
| `6c1070d` | feat(query): run a batch of queries from a file with --batch | T1, T1-T2 | 10 min 08 s | Código y tests 8 min 12 s, Checks (fmt, clippy, suite de tests) 1 min 56 s |
| `8c71d58` | docs(readme): explain query batches | T1-T2, T4 | 1 min 42 s | Checks (fmt, clippy, suite de tests) 0 min 07 s, Revisión RDD 0 min 44 s, Documentación 0 min 51 s |
| `bbdf250` | test(acceptance): check query batches end to end | T4 | 2 min 01 s | Documentación 2 min 01 s |
| `1c4dd7e` | fix(query): address the batch review findings | T5, RDD, review-findings | 14 min 01 s | Aceptación 2 min 12 s, Revisión RDD 10 min 32 s, Correcciones de hallazgos 1 min 17 s |
| `4828ba8` | docs(prd): annotate D6, C6 and K8 for query batches |  | 0 min 00 s |  |
| — | después del último commit | last-slice, report | 1 min 43 s | Revisión RDD 1 min 20 s, Cierre (informe y merge) 0 min 23 s |
| **Total** | | | **31 min 37 s** | |

La fase "Revisión RDD" incluye la espera al verificador independiente, que corrió en segundo plano sin
registrarse como tarea aparte. También incluye la búsqueda de cómo declarar los archivos sin seguimiento ante
Gentle AI y el reintento tras el tiempo agotado. "Documentación" incluye la espera al escritor del script de
aceptación (T3), que trabajó en paralelo.

## Informe de cambios (`18ee4ba..4828ba8`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/crime-query` | 1.364 | 46 | 1.172 | 108 | 8 |
| `docs` | 738 | 7 | — | — | 7 |
| `scripts` | 538 | 11 | 440 | 71 | 1 |
| `README.md` | 45 | 6 | — | — | 1 |
| **Total** | **2.685** | **70** | **1.612** | **179** | **17** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos. No hay
lockfiles ni archivos generados. `docs` incluye el PRD y su traducción (`f8f60b6`), escritos en la sesión
anterior a esta ejecución.

No hubo una previsión de líneas. El commit `6c1070d` (+1.364/−46, unas 800 de ellas pruebas) supera la
referencia de unas 400 líneas porque la línea de comandos, el módulo del lote y la división de la ejecución no
compilan por separado.

## Resumen

El analista puede escribir un archivo con una consulta por línea, con las mismas opciones que una consulta
suelta, y ejecutarlo con `crime-query --dataset <carpeta> --batch <archivo>`.

- **Una apertura:** el consultor abre el conjunto una vez y todas las consultas leen la misma versión, aunque
  el generador publique otra mientras tanto.
- **Salida:** la salida estándar es exactamente la concatenación de las salidas sueltas. La identificación de
  cada consulta y el tiempo total van a la salida de errores, así que el CSV y el JSON se siguen pudiendo
  guardar tal cual (C5).
- **Errores previos:** cualquier error del archivo se detecta antes de ejecutar nada e indica su línea. Uno de
  esos errores es repetir un archivo de salida, que se detecta comparando rutas de forma léxica.
- **Fallos a mitad:** un fallo o un Ctrl+C a mitad para el lote, y las consultas anteriores conservan sus
  resultados.
- **Decisión técnica clave:** el lote no reimplementa nada de la consulta. Cada línea pasa por el mismo parser
  y la misma ejecución que una consulta suelta, y por eso la comprobación diferencial y las 91 filas de
  aceptación anteriores salen idénticas.

Esto deja listo el consultor original para el banco de comparación del módulo CanopyDB.

## Valores por defecto tomados sin preguntar

- La "guía del consultor" de AC-B9 es la sección del README: no existe una guía aparte.
- Sin `--format`, `--batch f --output x.csv` da el error existente del archivo de salida antes que el de
  `--batch`. Es lo que fija S1, "los errores de forma que ya existen ganan", y lo señaló el verificador.
- T1 y T2 van en un solo commit, porque no compilan por separado.
- Las anotaciones de cierre se commitearon después de la revisión RDD 1, para no mover su objetivo.

## Pendiente (no bloqueante)

- **Excepción a probar primero:** en T1–T2 el código se escribió antes que las pruebas, así que no se observó
  RED. Las pruebas nuevas no compilarían sobre la base. La comprobación diferencial y el verificador cubren el
  comportamiento anterior.
- **Advertencias del verificador sin corregir,** todas fuera del dominio del PRD:
  - `-hx` en una línea del lote da el error de argumentos de clap en vez de `la opción -h no se admite`.
  - Una palabra `-h=1` cuenta como `-h`.
- **Hallazgos informativos de RDD sin corregir:** R4-001 y los cuatro de la revisión 2, con el motivo de cada
  uno en *Revisiones RDD*.
- **Memoria del lote (AC-M8):** no se mide aquí. La mide `canopy-db-consultor` con los dos consultores, como
  fija el PRD.
- **Carpeta mal nombrada en la copia de trabajo:** la primera ejecución de la aceptación recibió
  `work\acceptance` sin comillas desde bash, y creó `workacceptance/` (66 MB) en la raíz de la copia. No se
  commitea; borrarla me fue denegado, así que queda para el desarrollador (ver *Limpieza manual*).

**Riesgo:** ítem 3. La línea de comandos y la ejecución de la consulta suelta se reorganizaron y las comparten
las consultas sin `--batch`. Lo cubren el verificador independiente, la comprobación diferencial contra la
base y las 91 filas anteriores de la aceptación, todos sin diferencias.

## Limpieza manual

- `C:\wt\chicago-crime-canopy-db-lote\workacceptance` (66 MB): restos de la primera ejecución fallida de la
  aceptación, sin seguimiento en git.
- `C:\wt\chicago-crime-canopy-db-lote\work\acceptance` (3,7 GB): conjuntos y salidas de la aceptación. git
  ignora la carpeta `work`; se puede borrar sin afectar al repositorio.
- `C:\Users\JESUS~1.LOP\AppData\Local\Temp\tmp.FONUejTUYE` (209 MB): copia compilada de la base para la
  comprobación diferencial.
- `C:\Users\JESUS~1.LOP\AppData\Local\Temp\tmp.Puql0Ni8ny` (585 KB): pruebas del verificador.
- `C:\Users\JESUS~1.LOP\AppData\Local\Temp\tmp.apiJ8IwbTO` (12 KB): prueba de humo del escritor de T3.
- `C:\Users\JESUS~1.LOP\AppData\Local\Temp\rdd*` y `phases-report.md`: salidas de las revisiones RDD.
