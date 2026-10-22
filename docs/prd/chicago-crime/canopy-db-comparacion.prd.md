# Banco de comparación CanopyDB — PRD

**Versión**: 1.0
**Fecha**: 2026-10-22
**Documento padre**: [Almacén de delitos de Chicago sobre CanopyDB — PRD](canopy-db.prd.md) (v1.0)

## Objetivo y contexto

Entregar el cuarto hijo del módulo CanopyDB (orden 4 de [Reparto en PRD hijos](canopy-db.prd.md#reparto-en-prd-hijos)):

- el **banco de comparación** (K3 del padre): una herramienta del desarrollador que genera el mismo CSV con las
  dos parejas, ejecuta con los dos consultores el conjunto fijo de consultas, sueltas y en lote, mide tiempos,
  memoria y tamaño en disco, comprueba que las dos parejas devuelven lo mismo (AC-M2) y escribe un informe en el
  repositorio;
- el **primer informe**, medido con el CSV de referencia en el equipo de referencia.

Este documento hereda todas las reglas de su ascendencia y no las repite. Solo fija lo que el padre le asigna
(columna "Fija" del reparto): el número de repeticiones, la medición en frío y en caliente, cómo se mide el pico
de memoria, las opciones de los generadores durante la medición, la tolerancia entre dos ejecuciones, qué hace el
banco si los resultados de las dos parejas no coinciden, los sistemas en que se ejecuta y la forma del informe.
Acota una de ellas: la medición en frío no se hace (KM2).

**Por qué ahora**: los dos consultores y el conjunto fijo de consultas ya existen (`canopy-db-consultor`), y las
cifras de rendimiento que se conocen salen de verificaciones y pruebas sueltas, no de una medición controlada.
El hallazgo H5 lo dice expresamente: sus cifras "no son de un banco controlado; las mide `canopy-db-comparacion`".
Sin banco no hay forma de saber si una versión nueva de CanopyDB, o una corrección propia, mejora algo.

### Estado de partida

Verificado el 2026-10-22 sobre el repositorio:

| Aspecto | Situación |
|---|---|
| Banco de comparación | No existe ninguno que compare las dos parejas. Lo que mide hoy son las cuatro verificaciones de aceptación, en PowerShell y solo para Windows, y la reproducción mínima de H5, que mide la escritura en CanopyDB sin el resto del proyecto. |
| Verificaciones de aceptación | Reciben `-WorkDir` (obligatoria), `-Csv` (por defecto `chicago-crime-data.csv` en la raíz del repositorio, que por convención es el extracto de referencia) y `-SkipBuild`, entre otros. Compilan las aplicaciones, miden la duración de cada ejecución y su pico de memoria desde fuera (el máximo de memoria privada comprometida del proceso), y escriben en la carpeta de trabajo un informe Markdown en inglés cuya cabecera tiene líneas `- **Result**:`, `- **Date**:`, `- **Commit**:` (`<commit> with uncommitted changes` si hay cambios; `unknown` sin git), `- **Machine**:`, `- **CSV**:`, la línea de modo del CSV y `- **Work folder**:`. Terminan con 0 (`PASS`) o 1 (`FAIL`). Cada una borra al empezar sus propias entradas de la carpeta de trabajo. |
| Conjunto fijo de consultas | F1 a F11 (decisión KC7 de [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md#conjunto-fijo-de-consultas)), en un archivo de lote de la verificación. La línea de F2 lleva `<número>`, que se sustituye antes de ejecutar por el primer número de caso compartido del CSV. |
| Generadores | `crime-generator` y `crime-canopy-generator`, con las mismas opciones: entrada, salida, carpeta temporal (por defecto la del sistema), log, sobrescritura y sin compresión. Por defecto, el original comprime con Snappy y el CanopyDB con LZ4 los valores grandes. El resumen final muestra los registros válidos y rechazados, la duración, el pico de memoria privada comprometida y el tamaño de cada archivo publicado, sin total. No tienen opción de hilos. |
| Consultores | `crime-query` y `crime-canopy-query`. Cada consulta escribe en la salida de errores su línea de estadísticas, con el número de resultados y el tiempo de la consulta sin contar la apertura; el lote añade una cabecera por consulta y `Lote completado: <n> consultas en <t> s.`, con el tiempo de la ejecución entera. Ninguno muestra su memoria. |
| Cifras conocidas, generación | Original: 65–75 s y unos 1,1 GB con Snappy (README); pico de 93,4 MiB. CanopyDB: mejor generación medida 1 min 41 s y una base de 2.718.945.280 bytes (H5); pico de 219,5 MiB en Windows y 212,2 MiB en Linux (informe de cierre de `canopy-db-generador`). En una verificación con el equipo ocupado por otros procesos, una generación del original tardó 251,5 s. |
| Cifras conocidas, consultas | Del informe de cierre de `canopy-db-consultor`, una sola ejecución: F11 tarda 67,0 s con el original y 10,5 s con CanopyDB; picos de 14,5 / 7,6 / 19,3 MiB (conteo `THEFT`, conteo `RITUALISM`, lote F1–F11) con el original y 2,1 / 1,0 / 2,4 MiB con CanopyDB; la consulta por ID del consultor CanopyDB tarda 22–25 ms de proceso completo, y 514 ms la primera vez. |
| Variación entre ejecuciones | Nunca se ha medido. H5 anota que la variación entre sus ejecuciones no está controlada. |
| CanopyDB | Las aplicaciones usan CanopyDB 0.2.5 publicada, sin fork. El manifiesto CanopyDB anota la versión y `fork_commit` `null`. |
| H5 y su prototipo | H5 (rendimiento): CanopyDB no tiene carga masiva de entradas ordenadas. Un parche de prueba en la documentación del hallazgo acelera unas 2,5 veces la escritura de su reproducción mínima; las aplicaciones no lo usan. |
| Equipo de referencia | Windows 11, 32 hilos, 64 GB de RAM (documento raíz). No consta el modelo de procesador ni el disco. |
| Linux | Ubuntu 24.04 sobre WSL2 es el Linux de referencia (L1). Las verificaciones de aceptación no se ejecutan en Linux, y las guías de Linux lo dicen. |

**Huecos de producto**: ninguno abierto. Cada punto de la columna "Fija" del padre queda como regla en este
documento o en [Queda a criterio de la implementación](#queda-a-criterio-de-la-implementación).

## Actores

- **Desarrollador**: ejecuta el banco, lee el informe, decide si lo conserva en el repositorio y da de alta los
  hallazgos de rendimiento que aparezcan (flujo E del padre).
- **Responsable del proyecto** (el usuario): lee los informes para decidir si un motor transaccional compensa y
  qué contribuciones a CanopyDB merecen la pena.

## Flujo

1. El desarrollador ejecuta `run-canopy-comparison.ps1` (KM1) en Windows, con el equipo sin otra carga, con una
   carpeta de trabajo propia y, opcionalmente, el CSV, el número de repeticiones `R`, la opción sin compresión y
   la tabla de datos de un informe anterior con el que comparar (KM10).
2. El banco hace las comprobaciones previas de [Validaciones y mensajes](#validaciones-y-mensajes), en su orden,
   y compila las aplicaciones en modo de producción, salvo con `-SkipBuild`. Cualquier fallo en este paso
   termina con 1 sin ejecutar ninguna aplicación ni escribir informe.
3. **Generación** (KM3): repite `R` veces (ejecuciones 1 a `R`) la generación con los dos generadores,
   alternándolos (original y después CanopyDB en cada repetición), cada vez en una carpeta de salida vacía y con
   la carpeta temporal dentro de la carpeta de trabajo, con las mismas opciones de línea de comandos para los
   dos. Toma del resumen final de cada generador la duración, los registros, el pico y el tamaño del conjunto
   publicado (KM4). De cada pareja se conserva el conjunto de su última generación terminada bien.
4. Si alguna pareja no tiene ninguna generación terminada bien, el banco no ejecuta consultas: escribe el
   informe con lo medido y termina con 1.
5. **Consultas sueltas** (KM2): sobre el conjunto conservado de cada pareja, ejecuta como un proceso nuevo cada
   consulta del conjunto fijo, de F1 a F11. Para cada consulta: la ejecución 0 con el consultor original y con
   el CanopyDB, y después las ejecuciones 1 a `R`, cada una con el original y luego con el CanopyDB; entonces
   pasa a la consulta siguiente. Las consultas sueltas usan exactamente el texto de las líneas del lote. Para
   cada ejecución toma el tiempo de la consulta (de la línea de estadísticas), el número de resultados, el tiempo
   del proceso y el pico de memoria (KM4).
6. **Lote**: después de todas las consultas sueltas, ejecuta el lote con las once consultas: la ejecución 0 con
   el original y con el CanopyDB, y después las ejecuciones 1 a `R`, igual. Para cada ejecución toma el tiempo y
   el número de resultados de cada consulta, el tiempo de `Lote completado`, el del proceso y el pico de memoria
   del proceso entero. Un lote que falla es una sola ejecución fallida; las consultas que no llegó a responder
   quedan sin medidas.
7. **Mismos resultados** (AC-M2, KM6): aplica la regla de AC-M2 de
   [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md#criterios-de-aceptación), que ya acota la del padre
   (el mismo conjunto de registros solo en F1, F2 y F11, y F10 dentro de F11), a las consultas sueltas y al lote,
   con esta generalización para cualquier CSV: F10 devuelve tantos registros como el menor entre 1.000 y el
   número de resultados de F11, todos del resultado de F11.
8. Escribe el informe y su tabla de datos (ver [Salida](#salida)), termina la consola con dos líneas,
   `Result: <resultado>` y `Report: <ruta del informe>`, y termina con 0 si todas las ejecuciones terminaron
   bien y todas las consultas son `SAME`, o con 1 en otro caso (KM6).

Una ejecución termina bien cuando la aplicación termina con 0 y el banco obtiene todas sus medidas; si termina
con 0 pero falta alguna medida (por ejemplo, no hay línea de estadísticas), es una ejecución fallida.

## Validaciones y mensajes

El banco es una herramienta del desarrollador, como las verificaciones de aceptación: sus mensajes y su informe
están en inglés, y las comprobaciones previas fallan como en ellas, con un error de PowerShell que contiene el
mensaje y el código 1. Comprobaciones previas, en este orden; la primera que falla termina el banco sin ejecutar
ninguna aplicación ni escribir informe:

| Orden | Situación | Mensaje |
|---|---|---|
| 1 | `-Repetitions` no es un número entero | El error de PowerShell al leer el parámetro |
| 2 | `-Repetitions` menor que 1 | `-Repetitions must be at least 1.` |
| 3 | El CSV no existe | `Missing file: <ruta>` |
| 4 | `-Baseline` no existe | `Baseline file not found: <ruta>` |
| 5 | La primera línea de `-Baseline` no es exactamente la cabecera de la [Tabla de datos](#tabla-de-datos) | `Baseline file is not a comparison bench data file: <ruta>` |
| 6 | El CSV no tiene ningún número de caso compartido, así que F2 no se puede escribir | `The CSV has no shared case number, so F2 cannot be built.` |
| 7 | Ya existe el informe o la tabla de datos con el nombre de esta ejecución | `Report already exists: <ruta>` |
| 8 | Falta espacio libre en la carpeta de trabajo | `Only <libre> free in <carpeta>; about <n> GB are needed.` (`<n>` depende del espacio que exija el banco) |
| 9 | Falla la compilación | `cargo build failed:` seguido de la salida de errores de cargo |
| 10 | Falta una de las cuatro aplicaciones | `Missing binary: <ruta>` |

La carpeta de trabajo se crea si no existe, y la carpeta de informes del repositorio que contiene el script,
también. El banco solo borra lo que crea él mismo, en esta ejecución o en una anterior del banco; lo que deja una
ejecución cancelada lo limpia la siguiente.

Durante la medición:

| Situación | Regla | Código |
|---|---|---|
| Una ejecución de una aplicación falla (código distinto de 0 o falta alguna medida) | Sigue; la ejecución queda en la tabla de datos y en `Notes`, y no cuenta en las medianas | 1 |
| Los registros válidos o rechazados de las dos parejas no coinciden | Sigue; se anota en `Notes` | sin efecto |
| Ninguna generación de una pareja termina bien | Paso 4 del flujo | 1 |
| Las dos parejas devuelven resultados distintos en una consulta | Sigue midiendo; la consulta queda como `DIFFERENT` (KM6) | 1 |
| Una repetición se sale de la tolerancia de KM5 respecto a la mediana de su serie | Sigue; cuenta en la columna `Out of tolerance` | sin efecto |
| Una cifra supera un límite heredado (KM7) | Sigue; se anota en `Notes` | sin efecto |
| Ctrl+C | Para la aplicación en curso y termina sin escribir el informe ni la tabla de datos | distinto de 0 |

## Salida

### Informe

Un archivo Markdown en inglés por ejecución del banco (KM8), en `docs/canopy-db/comparisons/`, llamado
`<AAAA-MM-DD-hhmmss>-<commit>.md` con la fecha y hora de inicio y el commit corto (`unknown` sin git). Se escribe
siempre que el banco pase las comprobaciones previas y no se cancele; conservarlo en el repositorio es decisión
del desarrollador. Nunca se modifica después de escribirlo. Contenido, en este orden:

```text
# CanopyDB comparison bench

- **Result**: <resultado>
- **Date**: <inicio> (duration <duración>)
- **Commit**: <commit>
- **Machine**: <procesador>, <n> logical processors, <n> GiB RAM, <sistema>
- **CSV**: `<ruta>`, <bytes> bytes
- CSV: the reference extract (SHA-256 <huella>).
- **CanopyDB**: <versión> (fork <commit del fork o none>)
- **CanopyDB configuration**: generator: <ajustes>; query app: <ajustes>
- **Options**: <R> repetitions, compression <snappy o none> (SSTables) and <lz4 o none> (CanopyDB), temp folder `<ruta>`
- **Build**: <release build o skipped (-SkipBuild)>
- **Baseline**: `<ruta tal como se pasó>` o none
- **Work folder**: `<ruta>`
```

- `<resultado>`: `PASS: <n> runs, same results in all 11 queries`; con algún fallo o diferencia,
  `FAIL: <f> of <n> runs failed, <d> of 11 queries returned different results`; y si no se ejecutaron consultas
  (paso 4 del flujo), `FAIL: <f> of <n> runs failed, queries not run`. `<n>` cuenta todas las ejecuciones de
  aplicaciones que lanzó el banco, ejecuciones 0 incluidas; `<d>`, las consultas `DIFFERENT`.
- `Date`, `Commit`, `Machine` y `CSV` tienen exactamente el formato de las verificaciones de aceptación (fecha
  `AAAA-MM-DD hh:mm`, duración, bytes del CSV con separador de miles, rutas resueltas); las reglas
  comunes de abajo valen para las secciones. Al decidir si hay cambios sin confirmar no cuenta
  `docs/canopy-db/comparisons/`, para que un informe recién escrito no marque el siguiente.
- La línea de modo cambia respecto a la de las verificaciones, porque el banco no exige cifras: es la de arriba
  con el extracto de referencia y `CSV: not the reference extract (SHA-256 <huella>).` con otro CSV. La huella es
  la del manifiesto del conjunto original conservado; si no lo hay, la calcula el banco sobre el CSV.
- `CanopyDB` sale del manifiesto del conjunto CanopyDB conservado; si no lo hay, la línea es `- **CanopyDB**: unknown`.
- `<ajustes>`: cada ajuste de CanopyDB que la aplicación cambia respecto al valor por defecto de CanopyDB, como
  `<nombre>=<valor>` separados por `, `, con el nombre que le da CanopyDB, los tamaños en MiB y el tamaño de la
  caché de páginas siempre el primero.

Después, estas secciones:

- `## Generation`: tabla `| Pair | Runs | Duration median | Duration min | Duration max | Peak median | Size | Valid | Rejected | Out of tolerance |`,
  una fila `SSTables` y una `CanopyDB`. `Runs` es `<terminadas bien> of <R>`. `Size`, `Valid` y `Rejected`
  son los de la generación conservada.
- `## Results`: tabla `| Query | Line | SSTables single | CanopyDB single | SSTables batch | CanopyDB batch | Check |`,
  una fila por consulta F1–F11; `Line` es la línea ejecutada, entre comillas invertidas, con el número de caso
  ya sustituido; los números de resultados son los de la ejecución 0; `Check` es `SAME`, `DIFFERENT`,
  `INCOMPLETE` o `NOT RUN` (KM6).
- `## Single queries`: tabla `| Query | Pair | Runs | Query time median | Process time median | Process time first run | Peak median | Out of tolerance |`,
  dos filas por consulta (`SSTables` y `CanopyDB`). `Runs` es `<terminadas bien> of <R>`, sin contar la 0.
- `## Batch`: tabla `| Query | SSTables query time median | CanopyDB query time median |` con una fila por
  consulta, y tabla `| Measure | SSTables | CanopyDB |` con las filas `Runs` (como en `Single queries`),
  `Batch total median`, `Process time median`, `Process time first run`, `Peak median`, `Peak first run` y
  `Out of tolerance`.
- Si no se ejecutaron consultas (paso 4 del flujo), `Single queries` y `Batch` tienen todas sus filas, con
  `0 of <R>` en `Runs` y `—` en las medidas.
- `## Baseline`, solo con `-Baseline`: tabla `| Section | Row | Measure | Baseline | This run | Change | Status |`,
  con una fila por cada mediana de `Generation`, `Single queries` y `Batch` y por `Size`, `Valid` y `Rejected`
  de `Generation`, en el orden en que aparecen en el informe: en las tablas con una fila por pareja, fila a fila
  y, en cada una, columna a columna; en las tablas con una columna por pareja (las dos del lote), fila a fila y,
  en cada una, `SSTables` y después `CanopyDB`. `Section` es el título de la sección; `Row`, la
  pareja (`SSTables` o `CanopyDB`), precedida de la consulta en `Single queries` y en la tabla por consulta del
  lote (`F3 CanopyDB`); `Measure`, el nombre de la columna o de la fila. `Change` es la diferencia respecto a la
  referencia en porcentaje, con signo y un decimal (`+12.3 %`); `Status` es `inside tolerance` o
  `outside tolerance` (KM5).
- `## Notes`: primero las cifras que superan un límite heredado (KM7), en el orden de la lista y con `SSTables`
  antes que `CanopyDB`; después los registros que no coinciden; después las ejecuciones fallidas, en el orden
  en que se ejecutaron; o `none`:
  - `- AC-M4: <pair> generation peak median <x> MiB exceeds 512 MiB.`
  - `- AC-C7: <pair> F1 process time median <x> s exceeds 1 s.`
  - `- AC-M8: <pair> peak ratio <x> exceeds 3 (F3 <a> MiB, F4 <b> MiB, batch <c> MiB).`, con las medianas; el
    cociente es la mayor de las tres medianas dividida por la menor, con dos decimales.
  - `- Records differ: SSTables <válidos> valid and <rechazados> rejected, CanopyDB <válidos> valid and <rechazados> rejected.`,
    comparando las generaciones conservadas; no sale si alguna pareja no tiene generación conservada.
  - `- Failed run: <phase> <pair> run <r>: <motivo>.`, con la consulta tras `<pair>` en las consultas sueltas
    (`single CanopyDB F7 run 3`); `<phase>` es `generation`, `single` o `batch`; `<motivo>` es
    `exit code <c>: <última línea no vacía de su salida de errores, o no error output>`, o, si terminó con 0,
    `missing <medida>` con la primera medida que falta en el orden de la tabla de datos, como su `metric`
    precedida de la consulta en el lote (`missing F7 query_s`).

`<pair>` es `SSTables` o `CanopyDB` en todo el informe.

Reglas comunes:

- **Medianas**: de las ejecuciones 1 a `R` que terminaron bien; con un número par, la media de las dos
  centrales. La ejecución 0 nunca entra en una mediana. Se calculan en decimal, no en coma flotante binaria,
  con los valores de la tabla de datos, y se redondean al formato del informe con los empates lejos de cero
  (0,0415 da 0,042); la tolerancia y `Change` usan las medianas ya redondeadas.
- **`Out of tolerance`**: cuántas de las ejecuciones 1 a `R` que terminaron bien de esa fila tienen alguna de
  las medidas que la fila resume en medianas (tiempos y picos) fuera de la tolerancia de KM5 respecto a la
  mediana de su serie; en `Batch`, las del lote entero (total, proceso y pico). Las diferencias en resultados y
  registros no cuentan aquí: van a `Check` y a `Records differ`. Con `R` = 1 es 0.
- **Formatos**: tiempos en segundos con tres decimales (`0.041 s`); memoria en MiB con un decimal
  (`14.5 MiB`); tamaños en bytes y MiB (`1177432100 bytes (1122.9 MiB)`); enteros sin separador de miles;
  punto decimal. Una celda sin dato muestra `—`; en particular, una serie sin ejecuciones que terminaran bien,
  una ejecución 0 que no terminó bien, una pareja sin consultas o sin generación conservada, y en `Baseline` una
  medida que falta en la referencia o una referencia de valor 0 (en `Change` y `Status`).

### Tabla de datos

Junto al informe, con el mismo nombre y extensión `.csv`: UTF-8 sin BOM, separado por comas, sin comillas, con
punto decimal, finales de línea LF, un salto de línea final y una fila por medida, en el orden en que se
ejecutaron las ejecuciones y, dentro de cada una, en el orden de las medidas de la lista; en el lote, de F1 a
F11 con `query_s` y `results` cada una, y después las del lote entero. Al leer un
`-Baseline`, un retorno de carro al final de una línea se ignora. Cabecera:

```text
phase,pair,query,run,exit_code,metric,value
```

- `phase`: `generation`, `single` o `batch`. `pair`: `sstable` o `canopydb`.
- `query`: `F1`–`F11`; vacío en la generación y en las medidas del lote entero.
- `run`: de `1` a `R` en la generación; de `0` a `R` en las consultas.
- `exit_code`: el código con que terminó la ejecución.
- `metric` y su unidad: en `generation`, `duration_s`, `peak_mib`, `size_bytes`, `records_valid` y
  `records_rejected`; en `single`, `query_s`, `process_s`, `peak_mib` y `results`; en `batch` por consulta,
  `query_s` y `results`; en `batch` del lote entero, `batch_total_s`, `process_s` y `peak_mib`.
- `value`: segundos con tres decimales, MiB con un decimal y enteros sin separador; vacío si la ejecución no
  llegó a dar esa medida.

Cada ejecución lanzada tiene todas las filas de su fase, también las fallidas; las consultas que no se
ejecutaron (paso 4 del flujo) no tienen filas. Las medianas del informe salen de esta tabla, y es lo que lee
`-Baseline`.

## Documentación

- **Ayuda del script**, como la de las verificaciones de aceptación.
- **README**: un apartado del banco: para qué sirve, cómo se ejecuta, sus parámetros, su duración medida, dónde
  deja el informe, cómo se compara con uno anterior y que necesita una carpeta de trabajo propia y el equipo sin
  otra carga.
- **Guías de Linux**: el banco, como las verificaciones, solo se ejecuta en Windows.
- **Registro de hallazgos**: H5 enlaza el primer informe como su medición controlada.

## Reutilización

- **Verificaciones de aceptación**: sus comprobaciones previas y mensajes, la compilación, la medición de memoria
  desde fuera, el formato de su cabecera y la recogida del commit, del equipo y de la huella del CSV.
- **Conjunto fijo de consultas** y la sustitución de `<número>` de la verificación del consultor CanopyDB, sin
  cambiar su archivo.
- **Comprobación de AC-M2** de esa verificación: cómo compara conteos y conjuntos de registros.
- **Medición del pico de memoria** del documento raíz en Windows: el máximo de memoria privada comprometida.
- **Resumen final de los generadores** y **línea de estadísticas y línea final del lote** de los consultores,
  como fuente de la duración, los registros, el pico de los generadores, los tiempos de consulta y los
  resultados.

## Fuera de alcance

- Ejecutar el banco en Linux (KM9).
- Medir en frío, con la caché de archivos del sistema operativo vacía, reiniciando el equipo o con herramientas
  externas (KM2). La ejecución 0 se informa aparte, pero no es una medición en frío.
- Umbrales de rendimiento propios o fallar por una cifra: el banco solo falla por ejecuciones fallidas o
  resultados distintos (KM6, KM7). Tratar un criterio como "no cumplido por CanopyDB" (K10) es del informe de
  cierre, no del banco.
- Medir el parche de H5 o cualquier fork: no hay ningún fork aprobado (K4). Cuando lo haya, el banco lo mide sin
  cambios, porque el informe anota la versión y el commit del fork del manifiesto.
- Comprobar que un `-Baseline` se midió con el mismo CSV, las mismas opciones o el mismo equipo: el desarrollador
  elige con qué compara.
- Comprobar que las aplicaciones compiladas antes de `-SkipBuild` corresponden al commit del informe.
- Ejecutar el banco a la vez que otro banco, una verificación de aceptación u otro proceso sobre la misma carpeta
  de trabajo, y un tiempo máximo para una aplicación que no termina.
- Elegir qué consultas se ejecutan, un `R` distinto por fase, parar a mitad y reanudar, juntar varios informes y
  la integración continua.
- Cambiar las aplicaciones, sus opciones o sus mensajes para medir mejor, y ajustar CanopyDB o las aplicaciones
  para que salgan mejor en el banco.
- Del flujo E, todo salvo el alta en el registro de hallazgos.

## Queda a criterio de la implementación

- Adónde va la salida estándar de las consultas mientras se miden, siempre que sea igual para los dos
  consultores en todas las ejecuciones y que el banco pueda comparar los resultados de AC-M2.
- Cómo obtiene el banco los ajustes de CanopyDB que muestra, siempre que sean los del commit medido.
- La organización de la carpeta de trabajo, lo que deja en ella al terminar y el espacio libre que exige,
  calculado a partir de una ejecución medida.
- La forma de mostrar el progreso por consola, salvo la línea `Result` y la ruta del informe del final.
- Cómo provocan las pruebas una ejecución fallida, resultados distintos y los fallos de las comprobaciones
  previas (AC-KM1 a AC-KM3), siempre que no añada ninguna opción visible al banco ni a las aplicaciones y no
  cambie una ejecución normal.

## Criterios de aceptación

Las cifras corresponden al CSV de referencia, con la misma salvedad que en el padre. AC-M2 y AC-M10 son los
criterios del módulo que el padre asigna a este hijo; se comprueban al cerrar, ejecutando el banco en el equipo
de referencia sin otra carga, y se anotan en el informe de cierre. Las pruebas del banco se ejecutan en Windows.

1. **AC-M2** (con el banco) — Dado el CSV de referencia, cuando se ejecuta el banco, entonces la sección
   `Results` del informe dice `SAME` en las once consultas y el banco termina con 0.
2. **AC-M10** (K3, KM5) — Dado el banco, cuando se ejecuta sobre el CSV de referencia, entonces el informe tiene
   la cabecera y todas las secciones de [Informe](#informe), sin `—` en las secciones `Generation`,
   `Single queries` y `Batch`; y cuando se ejecuta otra vez, sin cambios en el código entre las dos, con
   `-Baseline` apuntando a la tabla de datos de la primera, entonces ninguna fila de `Baseline` dice
   `outside tolerance`, con la tolerancia de KM5 o la que fije su ampliación por ruido del equipo.
3. **AC-KM1** (KM6) — Dado un banco en el que un consultor devuelve en una consulta un número de resultados
   distinto del otro, cuando termina, entonces esa consulta dice `DIFFERENT`, la línea `Result` dice
   `FAIL: 0 of <n> runs failed, 1 of 11 queries returned different results`, las demás secciones tienen sus
   cifras y el código es 1.
4. **AC-KM2** (KM6) — Dada una ejecución de una aplicación que termina con un código distinto de 0, cuando el
   banco termina, entonces `Notes` la muestra con su código, la tabla de datos tiene sus filas con ese código,
   no cuenta en las medianas, `Result` dice `FAIL` y el código es 1; y cuando fallan todas las generaciones de
   una pareja, el banco no ejecuta consultas, `Results` dice `NOT RUN` en las once y `Result` termina en
   `queries not run`.
5. **AC-KM3** — Dado cada caso de las comprobaciones previas de
   [Validaciones y mensajes](#validaciones-y-mensajes), cuando se ejecuta el banco, entonces muestra su mensaje,
   termina con 1 y no ejecuta ninguna aplicación ni escribe informe; y con dos casos a la vez, muestra el del
   orden menor.
6. **AC-KM4** (KM8) — Dado un informe, cuando se lee su tabla de datos, entonces tiene la cabecera y una fila
   por medida de cada ejecución, incluidas la 0 y las fallidas, con los valores de
   [Tabla de datos](#tabla-de-datos), y cada mediana del informe coincide con la calculada de ella; con
   `-Repetitions 1` y `-Repetitions 2`, las medianas siguen las reglas comunes.
7. **AC-KM5** — Dado el CSV de referencia en el equipo de referencia, cuando se cierra este hijo, entonces el
   primer informe y su tabla de datos están confirmados en el repositorio con `Result: PASS`, y el README, la
   ayuda del script, las guías de Linux y H5 tienen lo que pide [Documentación](#documentación).
8. **AC-KM6** — Dado el repositorio en Windows y en Linux (Ubuntu 24.04 sobre WSL2), cuando se pasan las
   pruebas, entonces pasan todas, y en Windows las cuatro verificaciones de aceptación siguen pasando.

## Decisiones

Todas se tomaron con el usuario el 2026-10-22.

- **KM1 — Forma del banco**: un script de PowerShell, `run-canopy-comparison.ps1`, junto a las verificaciones de
  aceptación, con `-WorkDir` (obligatoria), `-Csv` (el mismo valor por defecto que ellas), `-Repetitions` (por
  defecto 5), `-NoCompression`, `-Baseline <tabla de datos>` y `-SkipBuild`. Motivo: reutiliza la medición de
  memoria, la recogida de datos del equipo y las comprobaciones previas de las verificaciones. Se descartó un
  banco de pruebas de Rust, que mide funciones y no procesos completos.
- **KM2 — Primera ejecución y caliente**: cada consulta y cada lote se ejecutan una primera vez (ejecución 0),
  que se informa aparte, y luego `R` veces más, cuyas medianas son las cifras del informe. Ninguna es una
  medición en frío: la caché del sistema operativo tiene los archivos recién generados. Acota "en frío y en
  caliente" del padre. Motivo: vaciar esa caché en Windows exige permisos de administrador; la ejecución 0
  muestra lo que cuesta la primera consulta después de generar, que no está en las medianas.
- **KM3 — Repeticiones y opciones de generación**: `R` (5 por defecto) generaciones de cada pareja, alternadas,
  cada una en una carpeta vacía, con la carpeta temporal en la carpeta de trabajo y la compresión por defecto de
  cada generador; con `-NoCompression`, los dos sin compresión. Motivo: medir lo que obtiene el operador con las
  opciones por defecto, en el mismo disco para los dos, y que la deriva del equipo afecte a las dos parejas por
  igual.
- **KM4 — Pico de memoria, tamaño y tiempo del proceso**: en los generadores, el pico que muestra su resumen
  final (el de AC-M4); en los consultores, el máximo de memoria privada comprometida que el sistema registra
  para el proceso, leído al terminar y no por muestreo, para que una consulta de pocos milisegundos tenga su
  pico, como en AC-M8; en el lote, el del proceso entero. El tamaño de un conjunto es la suma de los tamaños de
  archivo que muestra el resumen final de su generador, así que es el de la publicación, antes de consultarlo
  (K6 permite que el consultor CanopyDB cambie el tamaño de los archivos internos). El tiempo del proceso va del
  arranque al final del proceso, sin esperas del banco por medio. Motivo: es la misma medida en todo el módulo,
  y el mínimo de 0,050 s de KM5 exige que el banco no añada su propio retraso.
- **KM5 — Tolerancia**: una medida está dentro de la tolerancia de otra de referencia si cumple al menos una de
  sus dos condiciones: en los tiempos, que la diferencia no supere el 20 % de la referencia o no supere 0,050 s;
  en la memoria, el 10 % o 2 MiB; en el tamaño, el 1 %. El número de resultados y los registros tienen que ser
  iguales. Se aplica a cada repetición frente a la mediana de su serie y, con `-Baseline`, a cada mediana frente
  a la de referencia. Motivo: los tiempos cortos varían mucho en proporción, así que necesitan un mínimo
  absoluto. La variación entre ejecuciones nunca se había medido al fijar estas cifras; el primer informe la
  mide. Si al cerrar AC-M10 falla porque la variación real del equipo de referencia sin otra carga supera estas
  cifras, y no por un defecto, la ejecución amplía la tolerancia de las medidas afectadas hasta cubrir la
  variación medida, lo anota en su log y en el informe de cierre, y anota el cambio en este documento.
- **KM6 — Resultados distintos y fallos**: el banco sigue midiendo, marca la consulta como `DIFFERENT` o la
  ejecución como fallida y termina con 1. Para cada consulta, `Check` es:
  - `NOT RUN` si no se ejecutaron consultas (paso 4 del flujo);
  - `INCOMPLETE` si en la ejecución 0 de algún consultor, suelta o en lote, falta el número de resultados de
    esa consulta (un lote 0 que falla en F7 deja `INCOMPLETE` de F7 a F11, no las anteriores);
  - `DIFFERENT` si no, cuando el número de resultados no es el mismo en todas las ejecuciones que terminaron bien
    de los dos consultores, sueltas y en lote, o las ejecuciones 0 no cumplen el resto de la regla de AC-M2
    (paso 7 del flujo);
  - `SAME` en otro caso.

  Solo deja de consultar si una pareja no tiene ninguna generación terminada bien. Motivo: las cifras de las
  demás consultas siguen sirviendo para investigar, y un código 1 impide tomar el informe por bueno.
- **KM7 — Límites heredados**: el banco no comprueba criterios de aceptación; si una mediana supera el pico de
  AC-M4 (512 MiB, en cualquiera de los dos generadores), el segundo de AC-C7 (tiempo del proceso de F1, en
  cualquiera de los dos consultores) o el cociente de AC-M8 (la mayor de las medianas de F3, F4 y el lote de
  cada consultor dividida por la menor), lo anota en `Notes` sin cambiar el resultado. Motivo: el padre dice que el informe no tiene
  umbrales; las verificaciones de aceptación ya los comprueban.
- **KM8 — Informe**: Markdown en inglés, como los de las verificaciones, en `docs/canopy-db/comparisons/`, con
  la fecha, la hora con segundos y el commit en el nombre, más una tabla de datos CSV con cada medida. Motivo:
  dos ejecuciones del mismo commit no chocan (AC-M10), y la tabla permite comparar sin releer el Markdown.
- **KM9 — Sistemas**: solo Windows. Motivo: las verificaciones de aceptación, de las que el banco reutiliza la
  medición, son solo de Windows, y K3 pide comparar las dos parejas en el mismo equipo, no en varios sistemas.
- **KM10 — Comparación con un informe anterior**: `-Baseline` recibe la tabla de datos de otro informe y añade la
  sección `Baseline`, sin cambiar el resultado ni el código de salida. Motivo: el objetivo del banco es ver qué
  cambia con una versión nueva de CanopyDB o una corrección propia, y AC-M10 necesita comparar dos ejecuciones.
  Se descartó comparar a mano dos informes.

## Referencias

- Documento padre: [canopy-db.prd.md](canopy-db.prd.md)
- Hermanos: [canopy-db-generador.prd.md](canopy-db-generador.prd.md),
  [canopy-db-lote.prd.md](canopy-db-lote.prd.md) y [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)
- Documento raíz: [chicago-crime.prd.md](chicago-crime.prd.md)
- [PRD de soporte para Linux](soporte-linux.prd.md) y [PRD de aceptación con cualquier
  CSV](aceptacion-cualquier-csv.prd.md)
- Registro de hallazgos: `docs/canopy-db/hallazgos.md`

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 70d459ca | 2026-10-22 10:31 UTC+01:00 | 32 min | 0 min | 32 min | 26 min | 7 min | 2 |

<!-- prd-time: [{"session": "70d459ca-5e0b-4e1d-886c-fe43c945c851", "start": "2026-10-22T09:31:12.503000+00:00", "wall": 1942, "discarded": 0, "agent": 1536, "human": 406, "interventions": 2, "idle_minutes": 10}] -->
