# Generador CanopyDB terminado: verificado y sin fusionar

Informe de cierre de `canopy-db-generador` (2026-10-21). Enfoque de ejecución: ODD.
Origen de producto: [PRD del generador CanopyDB](../../docs/prd/chicago-crime/canopy-db-generador.prd.md) (hijo 1 de
[canopy-db.prd.md](../../docs/prd/chicago-crime/canopy-db.prd.md)). Documento de ejecución:
[canopy-db-generador.md](canopy-db-generador.md).

Rama `feat/canopy-db-generador` en la copia de trabajo `C:\wt\chicago-crime-canopy-db-generador`, 14 commits locales
desde `18ee4ba` (último de código y documentación: `01a6e7e`), más el commit de este informe. No se ha fusionado
en la base ni se ha subido nada: en una copia de trabajo, la fusión la decide el desarrollador
(`Fusiona el cambio en la base`).

## Resultado

- **Especificación:** los 16 criterios del PRD (AC-KG1 a AC-KG14, AC-M3 y AC-M4) y las anotaciones de cierre
  (S1–S17 del documento de ejecución) están cumplidos. La única salvedad es el caso de AC-KG7 "error en sus
  archivos temporales", que no puede darse: ver *Pendiente*. Pruebas: `cargo test --workspace`
  508 pasan y 0 fallan en Windows, y 512 pasan y 0 fallan en Linux (Ubuntu 24.04 en WSL2, con las pruebas que
  solo existen allí). `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` están
  limpios.
- **Aceptación:** `run-canopy-acceptance.ps1` sobre el extracto de referencia da **PASS: all 17 checks passed**
  en 29 min. AC-M4 tuvo un pico de 219,5 MiB, tanto en el resumen como medido desde fuera. AC-M3 dio 8.656.904
  entradas iguales en cada tabla entre dos generaciones (con y sin compresión) y frente a las SSTables del
  generador original. AC-KG4 también pasó: leer la base dos veces no cambia ningún nombre ni el manifiesto.
  - El verificador CanopyDB sobre una generación completa, con `--reference`, da PASS en 19 comprobaciones,
    incluidas `THEFT` = 1.838.502 y "sin valor" = 613.720.
  - En Linux, AC-M4 se comprobó a mano: 212,2 MiB, en 1 min 55 s.
  - La aceptación del generador original sigue pasando: PASS en 15 comprobaciones, con un pico de 93,4 MiB
    (AC-KG11).
- **Revisiones y verificación:**
  - Tres revisiones RDD con cuatro perspectivas cada una, todas aprobadas sin hallazgos bloqueantes (ver
    *Revisiones RDD*).
  - Un verificador independiente (sonnet) hizo pruebas en una copia aparte y no encontró bloqueantes. Dio
    PASS en S2, S4–S8, S10–S14, S16 y S17 casi siempre con pruebas reales; en S1, S3 y S15 lo comprobó en
    pruebas y código, y la aceptación completa cubrió las filas con el CSV completo.
  - Su comparación diferencial del generador, el verificador y el consultor originales entre `18ee4ba` y HEAD
    salió idéntica: ayuda, errores de uso, resumen, SHA-256 de las seis `.sst`, manifiesto e informe del
    verificador.
- **Hallazgos corregidos por el camino:**
  - En cancelaciones y abortos, el almacenamiento se cerraba después de borrar la carpeta de versión. Ahora se
    cierra antes (RDD 1).
  - La causa de los errores del motor se perdía. Ahora queda en la línea `Detalle del error de CanopyDB:` (RDD 1,
    decisión KG7).
  - El verificador, si no podía leer las opciones guardadas, abría la base con las suyas y reescribía `OPTIONS`.
    Ahora no la abre y lo informa (RDD 2).
  - Comparar un conjunto consigo mismo esperaba 5 s por su propio bloqueo y fallaba. Ahora lo lee una vez (RDD 2).
  - El escritor del script de aceptación vio que `$runs` y `$RunS` eran la misma variable, porque PowerShell no
    distingue mayúsculas. Lo corrigió antes de entregar.

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-KG1 (AC-M1, generador) | `run-canopy-acceptance.ps1` (AC-G1, AC-G3–AC-G9 con el verificador CanopyDB; extracto de referencia → AC-A1) | PASS. AC-A2 y AC-A3 (otra descarga del portal) no se ejecutaron: ver *Pendiente* |
| AC-M3 | filas `AC-M3` y `AC-M3 (SSTables)` de la aceptación; `two_generations_and_both_compressions_hold_the_same_content`, `the_base_holds_the_same_keys_and_values_as_the_leveldb_tables` | PASS, 8.656.904 entradas iguales por tabla |
| AC-M4 | fila `AC-M4` (Windows); generación completa en WSL a mano | PASS: 219,5 MiB (Windows), 212,2 MiB (Linux) |
| AC-KG2 | `the_version_folder_holds_only_the_manifest_and_the_base`, `the_manifest_has_the_canopydb_fields_in_order`, `the_manifest_files_and_the_summary_list_every_file_of_the_base`, `a_csv_with_only_the_header_publishes_six_empty_tables` | PASS |
| AC-KG3 | `the_original_query_app_refuses_a_canopydb_dataset`; prueba real del verificador independiente | PASS: `No se encontró un conjunto de datos válido en <carpeta>.`, código 1 |
| AC-KG4 | fila `AC-KG4` de la aceptación; `reading_the_base_twice_adds_no_file_and_leaves_the_manifest_identical`, `reading_the_base_leaves_its_stored_options_byte_identical` | PASS |
| AC-KG5 | `a_leveldb_dataset_is_refused_without_overwrite_and_replaced_with_it`, `the_original_generator_treats_a_canopydb_dataset_the_same_way` | PASS |
| AC-KG6 | `orphan_version_folders_of_both_engines_are_discarded_with_one_message`, `only_the_stale_sort_folders_of_this_generator_are_discarded` | PASS |
| AC-KG7 | `tests/errors.rs` (disco lleno, fallo de escritura, fallo interno, pánico, error de varias líneas) | PASS; el caso de archivos temporales no es alcanzable (ver *Pendiente*) |
| AC-KG8 | `the_no_compression_option_is_recorded_in_the_manifest`, `the_help_is_the_original_one_with_the_three_changes`, `compression_makes_a_large_value_take_less_space` (con mutación comprobada) | PASS |
| AC-KG9 | `crates/crime-canopy-acceptance/tests/compare.rs` (10 pruebas) | PASS |
| AC-KG10 | `docs/canopy-db/hallazgos.md`; `cargo run -- h2` y `cargo run -- h3` en `docs/canopy-db/repro` | PASS: H2 falla a los 5,0 s con `EnvironmentLocked`, H3 reproducido |
| AC-KG11 | `cargo test --workspace` en Windows y Linux; aceptación del generador original | PASS: 508 / 512 pruebas, original 15/15 |
| AC-KG12 | `the_help_is_the_original_one_with_the_three_changes`; README y guías de Linux | PASS |
| AC-KG13 | validación del script (cuatro errores exactos); rama `NOT MET` en una copia con el límite a 100 MiB | PASS (la ejecución con el CSV de referencia no la necesitó) |
| AC-KG14 | `both_generators_share_the_lock_of_the_output_folder`; prueba real con dos procesos del verificador independiente | PASS |
| Anotaciones al cerrar | `generador.prd.md` y `generador.prd.en.md`, G5 y AC-G2 (`27d7b1d`) | Hecho |

## Revisiones RDD

| Commits | Perspectivas | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `5ded28e..094af06` (T1, T2) | risk, resilience, readability, reliability | aprobada | 7 WARNING y 2 SUGGESTION, ninguno bloqueante. Orden de cierre del almacenamiento (×2) y causa perdida del error del motor (×2): corregidos en T4. Compresión sin prueba: prueba añadida en T4. El error interno de claves desordenadas nombra un `.sst`: aceptado. El `Cargo.lock` mostraba el crate de T3, por el trabajo en paralelo: coherente tras `0d25696`. Duplicación entre `Built` y `RunSummary`, y errores de cierre invisibles: aceptados | `review-8455fdd65dc89407` |
| `094af06..b04477c` (T3, documentos de ejecución) | risk, resilience, readability, reliability | aprobada | 5 WARNING y 4 SUGGESTION, ninguno bloqueante. Opciones guardadas ilegibles → el verificador reescribía `OPTIONS` (×3), prueba de relectura solo por nombres, lista de tablas repetida, comentario de `FIELDS`, comparar la misma base, fallo de apertura en modo CSV sin prueba: corregidos en T3b (`1228833`) | `review-cb918a1f9303d9a1` |
| `b04477c..01a6e7e` (T4, T3b, T5, T6, T7, anotaciones) | risk, resilience, readability, reliability | aprobada | 1 WARNING y 7 SUGGESTION, ninguno bloqueante. El gancho de pánicos de Rust también escribe en la salida de errores (×2): aceptado, porque K9 admite el detalle en la salida de errores y antes el proceso caía con el código 101. Lista de tablas por índice, ayudas de las pruebas duplicadas o mal documentadas, argumento sin usar, K9 frente a KG7, fallos inyectados solo en `commit`, límite de 4 s en una prueba de comparación: aceptados | `review-7b945d672441c0cb` |

Otras evaluaciones:
- La de toda la rama (`18ee4ba..HEAD`, 6.033 líneas) fue rechazada por la herramienta con
  `lens_context_budget_exceeded`, sin crear autoridad. Por eso se revisó por porciones, con HEAD fijado en
  cada commit.
- El PRD (`5ded28e`) se evaluó como `passive`.
- T1 sola dio `under_budget` y se revisó junto con T2.

## Tiempo

- Estimación: 4–7 h. Inicio real: 2026-10-21 08:18:02 +0100. Fin de la verificación: 09:58:23.
- **Duración activa desde el arranque: 1 h 40 min**, frente a 4–7 h: el 42 % del mínimo estimado. Como en las
  ejecuciones anteriores, se acabó antes de la mitad de lo estimado. Las causas:
  - la prueba previa de CanopyDB despejó el mayor riesgo, la memoria, en los primeros minutos;
  - el verificador y el script de aceptación los escribieron escritores en paralelo;
  - las tres aceptaciones largas (57 min en total) corrieron en segundo plano mientras seguía el trabajo.
- Esperas por el usuario: 28 s, de 08:17:34 a 08:18:02 (confirmación de la estimación).

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 0 min 36 s | 1 % | 1 |
| Plan (documento de ejecución) | 6 min 10 s | 6 % | 1 |
| Código y tests | 21 min 07 s | 21 % | 5 |
| Checks (fmt, clippy, suite de tests) | 3 min 58 s | 4 % | 3 |
| Revisión RDD | 27 min 18 s | 27 % | 2 |
| Correcciones de hallazgos | 1 min 49 s | 2 % | 1 |
| Documentación | 39 min 59 s | 40 % | 2 |
| **Total activo** | **1 h 40 min** | **100 %** | **15** |

- Inicio: 2026-10-21 08:16:58 +01:00. Fin: 2026-10-21 09:58:27 +01:00.
- Arranque confirmado: 08:18:02. Activo desde el arranque: **1 h 40 min** (compárese con la estimación); antes del arranque: 0 min 36 s.
- Esperas por el usuario (fuera del activo): 0 min 28 s.
  - 08:17:34–08:18:02 (0 min 28 s): confirm estimate
- Pausas entre sesiones (fuera del activo): 0 min 04 s.
  - 2026-10-21 09:58:23 +01:00 – 2026-10-21 09:58:27 +01:00 (0 min 04 s)
- Fase abierta, no contada: Cierre (informe y merge) desde 09:58:27.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Checks (fmt, clippy, suite de tests) | T1 differential | 08:33:00 | 08:33:29 | 0 min 29 s |
| Checks (fmt, clippy, suite de tests) | T1 differential | 08:33:48 | 08:34:21 | 0 min 33 s |
| Aceptación | T8 full CanopyDB run | 08:57:07 | 09:00:54 | 3 min 47 s |
| Checks (fmt, clippy, suite de tests) | T8 Linux tests and AC-M4 | 08:58:05 | 09:01:54 | 3 min 49 s |
| Aceptación | T8 verifier on full run | 09:01:04 | 09:04:04 | 3 min 00 s |
| Aceptación | T8 run-canopy-acceptance full | 09:06:07 | 09:35:11 | 29 min 04 s |
| Aceptación | T8 original acceptance | 09:36:19 | 09:57:26 | 21 min 07 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **4 min 51 s** |
| **Aceptación (total)** | | | | **56 min 58 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `5ded28e` | docs(prd): add the canopy-db-generador child PRD |  | 6 min 21 s | Exploración y mapa 0 min 36 s, Plan (documento de ejecución) 5 min 45 s |
| `7b61063` | refactor(generator): share the pipeline through a storage trait and an app identity | T1 | 13 min 12 s | Plan (documento de ejecución) 0 min 25 s, Código y tests 11 min 40 s, Checks (fmt, clippy, suite de tests) 1 min 07 s |
| `094af06` | feat(canopy-generator): add crime-canopy-generator, the CanopyDB generator | T1, T2 | 6 min 13 s | Código y tests 4 min 11 s, Checks (fmt, clippy, suite de tests) 2 min 02 s |
| `0d25696` | feat(canopy-acceptance): add the CanopyDB verifier with a compare mode |  | 0 min 00 s |  |
| `bfa31d5` | docs(odd): add the canopy-db-generador execution document and phase log | T2, T1-T3 | 0 min 31 s | Checks (fmt, clippy, suite de tests) 0 min 06 s, Revisión RDD 0 min 25 s |
| `b04477c` | docs(odd): log T2, T3 and the first RDD attempt | T1-T3 | 1 min 02 s | Revisión RDD 1 min 02 s |
| `17ae3da` | feat(canopy-generator): report engine failures with the original messages and a detail line | T1-T3, T4 | 7 min 24 s | Código y tests 2 min 53 s, Revisión RDD 4 min 31 s |
| `1228833` | fix(canopy-acceptance): never open a base whose stored options cannot be read | T4, T3b | 1 min 47 s | Código y tests 0 min 06 s, Correcciones de hallazgos 1 min 41 s |
| `873a29b` | build: lock the fail-points dev-dependency of crime-canopy-generator | T3b, T5 | 1 min 33 s | Código y tests 1 min 25 s, Correcciones de hallazgos 0 min 08 s |
| `0d9b159` | docs(canopy-db): add the findings register with H1-H4 and the H2/H3 reproductions |  | 0 min 00 s |  |
| `a25e60a` | docs: document the CanopyDB generator, its verifier and the Linux check of AC-M4 | T5, T6, T7 | 1 min 28 s | Código y tests 0 min 52 s, Documentación 0 min 36 s |
| `78790d8` | feat(acceptance): add run-canopy-acceptance.ps1 for the CanopyDB generator | T7 | 9 min 03 s | Documentación 9 min 03 s |
| `27d7b1d` | docs(prd): annotate G5 and AC-G2 of the generator PRD for the CanopyDB generator | T7, T9 | 0 min 48 s | Documentación 0 min 48 s |
| `01a6e7e` | docs: document the CanopyDB acceptance script with its measured figures | T9 | 29 min 20 s | Documentación 29 min 20 s |
| — | después del último commit | T9, last slice, final | 22 min 15 s | Checks (fmt, clippy, suite de tests) 0 min 43 s, Revisión RDD 21 min 20 s, Documentación 0 min 12 s |
| **Total** | | | **1 h 40 min** | |

Lectura de la tabla: la fase de documentación (40 min) es en realidad, sobre todo, espera activa de las
aceptaciones en segundo plano. La fase quedó marcada como `docs` mientras corrían la aceptación completa
(29 min) y el verificador independiente, sin un `mark` intermedio. La revisión RDD (27 min) incluye los tres
ciclos de cuatro revisores; los de las dos primeras porciones corrieron mientras seguía el trabajo de T4 y T5.
La fila de `0d25696` y la de `0d9b159` salen a cero porque el reparto por commit asigna su tiempo a los commits
vecinos: el primero lo hizo un escritor delegado en segundo plano, y el segundo es el registro preparado mientras
corría una revisión.

## Informe de cambios (`18ee4ba..01a6e7e`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/crime-canopy-acceptance` | 2.319 | 0 | 1.932 | 173 | 10 |
| `crates/crime-canopy-generator` | 1.519 | 0 | 1.223 | 127 | 7 |
| `docs` | 1.435 | 3 | 178 | 20 | 9 |
| `scripts` | 870 | 0 | 640 | 148 | 1 |
| `crates/crime-generator` | 583 | 154 | 436 | 100 | 8 |
| `crates/crime-acceptance` | 203 | 73 | 161 | 32 | 3 |
| `odd` | 95 | 0 | — | — | 2 |
| `README.md` | 78 | 1 | — | — | 1 |
| `.gitignore` | 3 | 0 | — | — | 1 |
| `Cargo.toml` | 1 | 0 | 1 | 0 | 1 |
| **Total** | **7.106** | **231** | **4.571** | **600** | **43** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+464 −3), `docs/canopy-db/repro/Cargo.lock` (+664 −0)

No hubo previsión de líneas.

## Resumen

Ya existe un segundo generador, `crime-canopy-generator`, que publica el mismo conjunto de delitos en una base
CanopyDB en lugar de en SSTables. Tiene las mismas opciones, mensajes, códigos de salida, límite de memoria,
cancelación, publicación atómica y limpieza de restos que el original, porque comparte su cadena entera.

Para eso, el generador original pasó a escribir sus tablas a través de un `Storage` intercambiable y a recibir
su identidad como parámetro, sin cambiar su comportamiento.

La base, una sola base de datos con seis árboles, se escribe:
- sin registro de escritura anticipada;
- con una caché de 64 MiB;
- y se vuelca y se cierra antes de publicarse.

Así queda en 220 MiB de pico y lista para leer sin añadir archivos. El manifiesto CanopyDB usa
`schema_version` `"canopydb-2"` para que el consultor original lo rechace con un mensaje existente.

Un verificador independiente, que reutiliza las comprobaciones del original, valida la base contra el CSV y
compara conjuntos de los dos motores entrada a entrada. El registro de hallazgos arranca con H1–H4 y sus
reproducciones.

## Valores elegidos sin preguntar

- **Organización interna de la base:** una base de datos `crime` con seis árboles. Opciones del motor: sin
  registro de escritura anticipada, caché de 64 MiB, transacciones de 100.000 entradas, límites de 32 MiB y
  128 MiB. Fijado tras la prueba previa (L3).
- **Archivo del informe de aceptación:** `canopy-acceptance-report.md`, y todas sus entradas de trabajo llevan
  el prefijo `canopy-*`, para poder compartir la carpeta de trabajo con el script original. El PRD solo fijaba
  el título.
- **Espacio libre exigido por el script:** 18 GB. El pico estimado es de unos 14 GB; al terminar se miden 8,7 GB.
- **Verificador:** relee las opciones guardadas de la base en vez de imponer las suyas, para no reescribir
  `OPTIONS` (desviación del encargo, aceptada).
- **Formato del texto del modo de comparación:** por ejemplo `5 entries on each side, equal keys and values`.
  El PRD fijaba el contenido, no la redacción.
- **Reproducciones:** un programa con subcomandos (`h2`, `h3`) en `docs/canopy-db/repro`, fuera del workspace.
- **AC-M4 sin muestra externa:** con `-NotMetByCanopyDb` da `FAIL`, no `NOT MET`, porque una medición que falta
  es un fallo del script y no de CanopyDB (aviso del verificador independiente).

## Pendiente (no bloqueante)

- **AC-KG7, caso "error del motor en sus archivos temporales":** no es alcanzable. CanopyDB 0.2.5 solo crea
  archivos temporales para el lote de escritura de su registro de escritura anticipada, que está desactivado
  (`write_batch.rs`; L9). La regla del PRD dice "si los usa". Si una configuración futura activa ese registro,
  hará falta la atribución a la carpeta temporal y su prueba.
- **AC-A2 y AC-A3 (otra descarga del portal):** no se ejecutaron. La aceptación usó el extracto de referencia, y
  el script hereda sin cambios la lógica de los dos modos del original. Probarlos exige una descarga nueva
  (decisión A5).
- **Avisos aceptados de las revisiones:**
  - el gancho de pánicos de Rust escribe una línea más en la salida de errores;
  - la lista de tablas se construye por índice;
  - hay ayudas de prueba duplicadas;
  - los fallos se inyectan solo en `commit`;
  - una prueba de comparación tiene un límite de 4 s frente a la espera de 5 s del bloqueo, y puede ser frágil
    en una máquina muy cargada.

  Ninguno cambia la salida pública ni los criterios.
- **Reproducciones:** dejan sus carpetas en la carpeta temporal del sistema (`canopydb-repro-*`) y no las borran.

**Riesgo:** punto 3. El cambio reorganiza código que consumen las aplicaciones originales: la cadena del
generador detrás de `Storage` y `App`, las comprobaciones del verificador detrás de `Tables`, y
`Abort::WithDetail`. Sus pruebas, sus aceptaciones (15/15) y la comparación diferencial del verificador
independiente muestran que su comportamiento no cambia.

## Limpieza manual

Mi borrado de las carpetas temporales de los subagentes se denegó; las listo para que las borres tú:

| Ruta | Tamaño | Qué es |
|---|---|---|
| `System.Collections.Specialized.OrderedDictionary\` (raíz de la copia, sin seguimiento) | 25 MB | Salida de una generación de prueba que dejó el error `$runs`/`$RunS` del escritor de T6 |
| `work\acceptance\` | 8,7 GB | Aceptación CanopyDB (con su informe `canopy-acceptance-report.md`) |
| `work\acceptance-original\` | 2,1 GB | Aceptación del generador original |
| `work\full1\` | 2,6 GB | Generación completa y verificación de T8 |
| `work\differential\`, `work\t6\`, `work\t6-kg13\`, `work\t6-input\`, `work\smoke1\`, `work\canopy-branch.bundle`, `work\rdd*.json`, `work\*.py` | 0,8 GB | Pruebas diferenciales, del escritor de T6 y del ciclo RDD |
| `C:\Users\jesus.lopez\AppData\Local\Temp\tmp.fB7zwCVlTE` | 2,8 GB | Copia aparte del verificador independiente |
| `C:\Users\jesus.lopez\AppData\Local\Temp\tmp.G9TJwKTLZc` | 291 MB | Copia aparte del escritor de T3 |
| `C:\Users\jesus.lopez\AppData\Local\Temp\canopydb-repro-*` (9 carpetas) | pequeñas | Ejecuciones de las reproducciones H2/H3 |
| WSL: `~/canopy-db-generador` y `~/canopy-run` | 2,3 GB y 2,6 GB | Clon de la rama para las pruebas de Linux y la generación de AC-M4 |
