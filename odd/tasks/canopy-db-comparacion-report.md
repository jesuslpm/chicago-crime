# Banco de comparación CanopyDB terminado: verificado, con AC-M10 pendiente de una repetición con el equipo sin carga

Informe de cierre de `canopy-db-comparacion` (2026-10-22). Enfoque de ejecución: ODD.
Fuente de producto: [canopy-db-comparacion.prd.md](../../docs/prd/chicago-crime/canopy-db-comparacion.prd.md).
Documento de ejecución: [canopy-db-comparacion.md](canopy-db-comparacion.md).

Rama `feat/canopy-db-comparacion` en la copia `C:\wt\chicago-crime-canopy-db-comparacion`, con commits locales
desde `8786248` hasta el commit de este informe. No se ha fusionado con la base ni se ha subido nada: la fusión
se hace solo cuando el desarrollador dice `Fusiona el cambio en la base`.

## Resultado

- **Especificación:** el banco `scripts/acceptance/run-canopy-comparison.ps1` (con su biblioteca
  `canopy-comparison.ps1` y su ayuda), el primer informe y la documentación. Las 19 pruebas del banco pasan,
  con aplicaciones simuladas. Las comprobaciones de Rust (`cargo fmt --check`, `cargo clippy -D warnings`)
  están limpias. `cargo test --workspace` da 560 en verde en Windows y 562 en Linux (WSL Ubuntu, clon nuevo
  de la rama), 0 fallos, en 61 binarios. No cambia ningún archivo de Rust.
- **Aceptación con datos reales:** dos ejecuciones del banco con el extracto de referencia y 5 repeticiones.
  - La primera dio `PASS: 154 runs, same results in all 11 queries` en 3.966,6 s
    ([informe](../../docs/canopy-db/comparisons/2026-10-22-120128-fea8c09.md)).
  - La segunda, con `-Baseline` la primera, dio el mismo resultado en 3.210,0 s
    ([informe](../../docs/canopy-db/comparisons/2026-10-22-130851-82ffd76.md)). En ella 37 filas de tiempos
    quedan `outside tolerance`, y todas las de memoria, tamaño y conteos quedan dentro.
  - Las cuatro verificaciones de aceptación siguen pasando con el código nuevo: consultor 106 de 106,
    consultor CanopyDB 151 de 151, generador 15 de 15 y generador CanopyDB 17 de 17.
- **Revisiones y verificación:**
  - Un verificador independiente (sonnet, high) dio por cumplidas S1 y S3 a S7 y la mitad del informe de S2.
    Recalculó por su cuenta 224 cifras de cada informe a partir de su tabla de datos, sin ninguna diferencia, y
    confirmó que el C# movido es idéntico byte a byte.
  - Hubo cuatro revisiones RDD, todas aprobadas (ver más abajo).
- **Hallazgos corregidos por el camino:**
  - B1 (verificador): una ruta relativa en `-Csv` o `-Baseline` llegaba sin resolver a .NET, que la lee desde
    la carpeta del proceso. Ahora se resuelven al comprobarlas, con prueba de regresión (RED → GREEN).
  - Tres sugerencias de la revisión 2: el nombre de `ConvertFrom-SpanishDecimal`, la constante del marcador
    `<número>` y el mensaje del PRD ante una tabla de referencia dañada, con prueba.
  - Al escribir el banco, la primera pasada de pruebas falló en 15 de 17. Un ejemplo: `Runs` decía `1 of 2`
    porque `@()` volvía a envolver la serie. Se corrigió antes del primer commit del banco.

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-M2 | Las dos ejecuciones reales del banco, sección `Results` | PASS: `SAME` en las 11 consultas, código 0 en las dos |
| AC-M10 (informe) | Informe de cada ejecución; recálculo del verificador | PASS: cabecera y todas las secciones, sin `—` en `Generation`, `Single queries` ni `Batch` |
| AC-M10 (repetición) | Segunda ejecución con `-Baseline` la primera | **NO CUMPLIDO**: 37 filas de tiempos `outside tolerance` por carga externa en el equipo (ver Pendiente) |
| AC-KM1 | `AC-KM1: different numbers of results…`, `AC-KM1: the same counts with different records…` | PASS |
| AC-KM2 | `AC-KM2: a failed run shows in Notes…`, `AC-KM2: when every generation of a pair fails…`, `A run that ends with 0 without a measure…`, `KM6: a batch run 0 that fails at F7…` | PASS; el verificador lo reprodujo además con ejecutables reales que terminan con 3 |
| AC-KM3 | `AC-KM3: each preliminary check…` (las 9 comprobaciones y el orden), `AC-KM3: a -Repetitions that is not a number…`, `AC-KM3: a baseline with the header but a damaged row…` | PASS |
| AC-KM4 | `AC-KM4: the data table has one row per measure…`, `AC-KM4: with -Repetitions 1…`; recálculo del verificador sobre las dos tablas reales (878 filas) | PASS |
| AC-KM5 | Primer informe y su tabla confirmados con `Result: PASS`; README, ayuda, guías de Linux y H5 | PASS |
| AC-KM6 | `cargo test` en Windows (560) y Linux (562); las cuatro verificaciones de aceptación en Windows | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `8f30aec..4826d1a` | readability, reliability | Aprobada | R2-001 y R3-001 WARNING, `query-acceptance-canopy.ps1` usaba `Split-BatchWords` sin decir de dónde viene: nota en su cabecera. R2-002 y R3-002 SUGGESTION, la guarda `-as [type]` conserva un tipo viejo en una sesión larga: comentario. R2-003 SUGGESTION, los comentarios nombraban el banco antes de existir: resuelto con el banco. R2-004 SUGGESTION, T1 seguía `pending`: actualizado. | `review-406a602ce04b176f` |
| `4826d1a..44ef53a` | resilience, readability, reliability | Aprobada | R2-001 SUGGESTION, nombre engañoso del convertidor: `ConvertFrom-SpanishDecimal`. R2-002 SUGGESTION, marcador sin nombre: `$FixedQueryPlaceholder`. R3-002 SUGGESTION, una tabla de referencia dañada daba un error de conversión: mensaje del PRD y prueba. R3-001 WARNING, `Split-FirstBatch` reparte por los conteos de la ejecución 0: se mantiene, porque ese caso ya es una ejecución fallida y sus consultas, `INCOMPLETE`. | `review-d9eb9280bf3778a4` |
| `44ef53a..b037f18` | readability | Aprobada | R2-001 SUGGESTION, el informe 1 nombra la carpeta mal escrita `workcomparison`: se mantiene, porque es salida generada (explicado en L8 y aquí). R2-002 SUGGESTION, L8 mezclaba las dos comprobaciones de tolerancia: aclarado. R2-003 SUGGESTION, BOM nuevo en `run-canopy-comparison.ps1`: intencionado (PowerShell 5.1). | `review-3314685d6ed26845` |
| `b037f18..5a34839` | reliability, readability | Aprobada | R2-001 SUGGESTION, la fila de T5 daba 17 pruebas: ahora dice 19. R3-001 WARNING, el cambio de nombre sin alias: comprobado, ningún script usa el nombre viejo. R2-002 y R3-002 SUGGESTION, comentario de `Read-ComparisonData` y pruebas de dos casos más de tabla dañada: pendientes. | `review-1a642aa7c1bc8975` |

Evaluaciones que no abrieron revisión:

- `4826d1a..44ef53a` dio `medium` / `under_budget`.
- El tramo final completo `4826d1a..3bb79ad` se rechazó en START con `lens_context_budget_exceeded` (sin
  autoridad creada). Por eso se revisó como los tres tramos de las filas 2 a 4, dos de ellos con HEAD
  temporalmente en `44ef53a` y `b037f18`.

## Reutilización

- **Movido, no copiado:**
  - las clases C# de las verificaciones a `scripts/acceptance/QueryAcceptance.cs` y
    `CanopyQueryAcceptance.cs`;
  - los lectores de la salida de los consultores, los lectores JSON y la lectura del conjunto fijo a
    `query-output.ps1`.
  - Los usan la verificación del consultor y el banco. `Initialize-FixedQueries` ahora llama a
    `Read-FixedQueries`.
- **Reutilizado tal cual:** `LiveRun` (el instante de fin lo toma un hilo que espera el proceso),
  `PeakPrivateBytes`, `FirstSharedCaseNumber`, `CompareRowSetsHashed`, `CheckLimited`, `SplitBatchOutput`,
  `Compare-JsonDocuments`, `Test-ReferenceExtract` y el formato de cabecera de las verificaciones.
- **Nuevo:** la orquestación, las medianas y tolerancias, el informe, la tabla de datos y `-Baseline`.
  `Get-RustSettings` lee la configuración de CanopyDB del código fuente del commit medido.

## Tiempo

- Estimación: 3 h – 4 h 30 min. Inicio real: 2026-10-22 11:33:35 +01:00. Fin: 2026-10-22 17:07:04 +01:00.
- **Duración activa desde el arranque: 5 h 33 min**, 1 h 03 min por encima del máximo estimado. Es la primera
  ejecución que no acaba en la mitad de lo estimado. Casi todo es tiempo de máquina con las ejecuciones largas
  una tras otra: las dos del banco (1 h 06 min y 54 min, frente a los 40 min previstos) y las cuatro
  verificaciones de aceptación (2 h 07 min, más que sus cifras del README, con el equipo cargado). Ninguna de
  ellas podía solaparse con otra sin falsear las mediciones.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 1 min 25 s | 0 % | 1 |
| Plan (documento de ejecución) | 0 min 49 s | 0 % | 1 |
| Código y tests | 14 min 59 s | 4 % | 3 |
| Checks (fmt, clippy, suite de tests) | 2 h 43 min | 49 % | 2 |
| Aceptación | 2 h 02 min | 37 % | 1 |
| Revisión RDD | 7 min 01 s | 2 % | 3 |
| Correcciones de hallazgos | 23 min 45 s | 7 % | 2 |
| Cierre (informe y merge) | 0 min 01 s | 0 % | 1 |
| **Total activo** | **5 h 33 min** | **100 %** | **14** |

- Inicio: 2026-10-22 11:33:35 +01:00. Fin: 2026-10-22 17:07:04 +01:00.
- Arranque confirmado: 11:33:35. Activo desde el arranque: **5 h 33 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD T1 reviewers | 11:37:52 | 11:38:29 | 0 min 37 s |
| Checks (fmt, clippy, suite de tests) | smoke bench small CSV | 11:57:52 | 11:59:58 | 2 min 06 s |
| Aceptación | bench run 1 reference CSV | 12:01:22 | 13:07:47 | 1 h 06 min |
| Aceptación | bench run 2 with baseline | 13:08:43 | 14:02:33 | 53 min 50 s |
| Checks (fmt, clippy, suite de tests) | four acceptance checks | 14:03:42 | 16:10:46 | 2 h 07 min |
| Checks (fmt, clippy, suite de tests) | cargo checks Windows and WSL | 16:12:20 | 16:25:07 | 12 min 47 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **2 h 21 min** |
| **Aceptación (total)** | | | | **2 h 00 min** |
| **Revisión RDD (total)** | | | | **0 min 37 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `8f30aec` | docs(prd): add the canopy-db-comparacion PRD |  | 0 min 07 s | Exploración y mapa 0 min 07 s |
| `72843da` | refactor(acceptance): move the shared query helpers to their own files | T1 | 2 min 59 s | Exploración y mapa 1 min 18 s, Plan (documento de ejecución) 0 min 49 s, Código y tests 0 min 52 s |
| `4826d1a` | docs(odd): add the canopy-db-comparacion execution document | T1 | 0 min 22 s | Código y tests 0 min 14 s, Revisión RDD 0 min 08 s |
| `44ef53a` | feat(acceptance): add the CanopyDB comparison bench | T1, T2 | 23 min 49 s | Código y tests 13 min 53 s, Checks (fmt, clippy, suite de tests) 8 min 56 s, Revisión RDD 1 min 00 s |
| `fea8c09` | docs(odd): log the bench delivery of canopy-db-comparacion | T2, T4 | 0 min 31 s | Checks (fmt, clippy, suite de tests) 0 min 08 s, Aceptación 0 min 01 s, Revisión RDD 0 min 22 s |
| `82ffd76` | docs(odd): log the first bench run of canopy-db-comparacion | T4 | 1 h 07 min | Aceptación 1 h 07 min |
| `b037f18` | docs(canopy-db): add the first comparison reports and link them from H5 | T4 | 54 min 49 s | Aceptación 54 min 49 s |
| `dedc982` | fix(acceptance): resolve relative bench paths before reading them | T4, T5, B1 | 2 h 44 min | Checks (fmt, clippy, suite de tests) 2 h 34 min, Aceptación 0 min 08 s, Correcciones de hallazgos 10 min 28 s |
| `3bb79ad` | docs(odd): log the verification of canopy-db-comparacion | B1 | 0 min 18 s | Correcciones de hallazgos 0 min 18 s |
| `2a633e9` | docs(odd): update the phase log of canopy-db-comparacion | B1, closing slice | 0 min 59 s | Revisión RDD 0 min 58 s, Correcciones de hallazgos 0 min 01 s |
| `5a34839` | refactor(acceptance): address the review of the comparison bench | closing slice, review advisories | 11 min 42 s | Revisión RDD 4 min 33 s, Correcciones de hallazgos 7 min 09 s |
| — | después del último commit | review advisories | 5 min 50 s | Correcciones de hallazgos 5 min 49 s, Cierre (informe y merge) 0 min 01 s |
| **Total** | | | **5 h 33 min** | |

Casi todo el tiempo fue de máquina. Las fases «Checks» y «Aceptación» suman el 86 % del activo porque mientras
corrían el banco y las verificaciones no se lanzó nada que cargara el equipo. Eso incluye al verificador
independiente: arrancó al acabar las verificaciones y trabajó dentro de la fase de checks, no como fase propia.
El código y sus pruebas llevaron 15 min.

## Informe de cambios (`8786248..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `scripts` | 2.075 | 558 | 1.332 | 158 | 8 |
| `docs` | 872 | 3 | — | — | 5 |
| `odd` | 173 | 0 | — | — | 2 |
| `README.md` | 37 | 2 | — | — | 1 |
| **Total** | **3.157** | **563** | **1.332** | **158** | **16** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `docs/canopy-db/comparisons/2026-10-22-120128-fea8c09.csv` (+879 −0), `docs/canopy-db/comparisons/2026-10-22-120128-fea8c09.md` (+94 −0), `docs/canopy-db/comparisons/2026-10-22-130851-82ffd76.csv` (+879 −0), `docs/canopy-db/comparisons/2026-10-22-130851-82ffd76.md` (+203 −0)

No hubo previsión de líneas. De las 558 líneas quitadas en `scripts`, la mayoría son el C# y los lectores que se
movieron a archivos compartidos. Las cifras se tomaron antes de añadir este informe y las dos últimas
anotaciones del documento de ejecución.

## Resumen

El desarrollador tiene ahora un banco que genera el mismo CSV con las dos parejas y ejecuta las once consultas
del conjunto fijo, sueltas y en lote, con los dos consultores. Comprueba que devuelven lo mismo y deja en el
repositorio un informe con medianas de tiempo, memoria y tamaño, junto a una tabla de datos con cada medida. Con
`-Baseline` compara con un informe anterior, que es lo que hará falta con una versión nueva de CanopyDB o con una
corrección propia.

El primer informe confirma las proporciones que se intuían: la base CanopyDB ocupa 2,5 veces lo que las SSTables
y su generación es más lenta (204,8 s frente a 156,4 s de mediana). En cambio, sus consultas de recorrido largo
son más rápidas: el listado de todos los robos (F11) tarda 26,7 s suelto frente a 51,2 s del original, y 24,4 s
frente a 50,0 s dentro del lote; el lote entero, 36,4 s frente a 82,7 s. Su memoria es una décima parte. Las cifras absolutas de tiempo de este equipo hay que leerlas con la
cautela de Pendiente.

En lo técnico, el banco es un script de PowerShell sobre la medición de las verificaciones. Mide el tiempo de
proceso sin sondeo y lee el pico al terminar. Además, las verificaciones y el banco comparten ahora sus lectores.

## Decisiones tomadas sin preguntar

- **Pruebas sin dependencias:** las del banco son un script de PowerShell sin Pester (solo hay Pester 3.4). El
  punto de inyección es el entorno que recibe la función principal, sin ninguna opción visible.
- **Configuración de CanopyDB en la cabecera:**
  - se lee del código Rust del commit medido;
  - lista lo que fija cada aplicación, y cada ajuste que ya no se puede leer sale como `unknown`;
  - deja fuera `compress_overflow_values`, porque la línea `Options` ya muestra la compresión.
- **Motivo de una ejecución fallida:** la nota conserva el punto final de la línea de error
  (`...existente..`), como da el literal `<motivo>.` del PRD.
- **Binarios de las mediciones:** las dos se hicieron con `-SkipBuild`, sobre los binarios de la prueba de humo,
  porque el código Rust no cambió.
- **AC-M10:** no se amplió la tolerancia de KM5. Su regla de ampliación es para la variación del equipo sin otra
  carga, y aquí hubo carga externa: cubrirla exigiría admitir diferencias de un 80 % y el banco dejaría de servir.
- **Revisión del último tramo:** se partió en tres porque superaba el presupuesto de contexto de RDD. Dos de los
  candidatos se revisaron con HEAD temporalmente en un commit anterior, y luego se volvió a la rama.

## Pendiente (no bloquea)

- **AC-M10, la mitad de la repetición:** hay que repetir el banco dos veces con el equipo sin otra carga (sin
  Docker, sin otras sesiones de compilación) y comparar la segunda con `-Baseline` la primera.
  - No bloquea porque la carga externa está documentada con los datos de cada ejecución, y lo que no depende de
    la carga (memoria, tamaños y conteos) se reproduce.
  - Los datos: en la ejecución 1, las ejecuciones 2 a 5 de F3 pasaron de 1,1 s a 4,5 s en las dos parejas a la
    vez. En la ejecución 2, las consultas fueron mucho más rápidas, pero la generación más lenta.
  - Si con el equipo quieto sigue fallando, se aplica la ampliación de KM5 que fija el PRD.
- **Revisión 4** (R2-002 y R3-002): reescribir el comentario de `Read-ComparisonData` y añadir pruebas de una
  fila con un número de campos distinto o con un código de salida no numérico. Hoy esas filas ya se rechazan.
- **Advertencias del verificador:**
  - una ejecución 0 suelta fallida muestra `—`, mientras que un lote 0 fallido conserva los conteos que dio
    (KM6 lo pide así en el lote);
  - la comprobación de la cabecera de `-Baseline` tolera un BOM;
  - Ctrl+C no tiene prueba (el PRD no le asigna criterio);
  - los 14 GB de espacio libre son una estimación (las carpetas que quedaron ocupan unos 8,4 GB).
- **El informe 1 nombra la carpeta `workcomparison`:** fue un error al escribir `-WorkDir` desde bash. La carpeta
  se movió a `work/comparison-run1`. El informe se deja tal como lo generó el banco.

**Riesgo:**
- **Ítem 3:** T2 cambia `query-output.ps1` e `Initialize-FixedQueries`, que usa la verificación del consultor.
  Lo cubren el verificador independiente y las cuatro verificaciones de aceptación, que pasan.
- **Ítem 6:** T1 mueve código de las verificaciones que solo prueban sus propias ejecuciones. Lo cubren las
  mismas ejecuciones.

## Limpieza manual

Carpetas ignoradas por git, todas en esta copia salvo la última, que se pueden borrar:

| Ruta | Tamaño |
|---|---|
| `work/comparison` y `work/comparison-run1` (conjuntos y salidas de las dos mediciones) | 8,3 GB cada una |
| `work/acceptance-canopy-query` | 9,0 GB |
| `work/acceptance-canopy-generator` | 8,7 GB |
| `work/acceptance-query` | 3,7 GB |
| `work/acceptance-generator` | 2,1 GB |
| `work/smoke` (CSV pequeño de la prueba de humo) | 258 MB |
| `work/branch.bundle`, `work/wsl-*` | 8,5 MB |
| `~/canopy-db-comparacion-161209` en Ubuntu (WSL), el clon de las pruebas de Linux | 2,5 GB |

El control de seguridad no dejó borrar el clon anterior `~/canopy-db-consultor` en WSL, así que sigue ahí.
