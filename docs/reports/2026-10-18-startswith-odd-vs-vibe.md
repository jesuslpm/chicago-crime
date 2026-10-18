# ODD frente a vibe coding: el mismo PRD implementado dos veces

Comparación de dos ejecuciones reales del [PRD de búsqueda por prefijo](../prd/chicago-crime/startswith.prd.md)
(`--startswith CAMPO=PREFIJO` en el consultor), el 2026-10-18. Las dos partieron de la misma base (`0971a37`, que
añade el PRD encima de `main`), con el mismo agente, en paralelo, en dos copias de trabajo separadas: una con ODD y
otra con vibe coding. Es un cambio pequeño y bien especificado: entre 1.300 y 1.400 líneas, entre código y tests.

## Conclusión

**Las dos ejecuciones entregaron lo mismo, con una calidad de código parecida. ODD terminó un 18 % antes y deja más
trazabilidad; vibe coding necesitó menos ceremonia y probó más de extremo a extremo.** En un cambio de este tamaño
con un PRD claro, los dos enfoques son válidos. Las diferencias de producto entre ellos no salen del enfoque, sino
de los huecos del PRD: donde el PRD no fija algo, cada ejecución lo decidió de una manera.

## Resultado

| | ODD | Vibe coding |
|---|---|---|
| Criterios del PRD (AC-S1…AC-S12) | 12/12 | 12/12 |
| Aceptación sobre el CSV de referencia (8.656.904 registros) | 91/91 PASS | 87/87 PASS |
| Regresiones (58 filas AC-C, 425 tests de partida) | 0 | 0 |
| Defectos de producto encontrados durante la ejecución | 0 | 0 |
| Tests nuevos (`#[test]`) | 28 (total 453) | 22 (total 447) |
| Commits (sin el informe) | 6 | 8 |
| Revisiones RDD | 2, aprobadas | 3, aprobadas |
| Hallazgos RDD | 1 WARNING y 4 SUGGESTION; 3 corregidos | 1 WARNING (informativo) y 2 SUGGESTION; 1 corregido |
| Subagentes | 1 escritor en paralelo (selector y script de aceptación) | ninguno |
| Aceptación sobre el último commit | no (el último es una refactorización cubierta solo por tests) | sí |

## Velocidad de desarrollo

| | ODD | Vibe coding |
|---|---|---|
| Estimación | 1–2 h | 1 h 30 min – 2 h 30 min |
| **Activo desde el arranque** | **30 min 36 s** | **37 min 23 s** |
| Activo total, con la exploración previa | 34 min 05 s | 42 min 24 s |
| Real frente al mínimo estimado | 51 % | 42 % |
| Esperas por el usuario | 18 s | 20 s |
| Corridas de aceptación (en segundo plano) | 2, 15 min 11 s | 3, 22 min 42 s |

Tiempo activo por fase (registro de fases de cada ejecución):

| Fase | ODD | Vibe coding |
|---|---|---|
| Exploración y plan | 7 min 11 s (21 %) | 5 min 01 s (12 %) |
| Código y tests | 8 min 33 s (25 %)¹ | 13 min 29 s (32 %) |
| Checks (fmt, clippy, tests) | 2 min 42 s (8 %) | 1 min 24 s (3 %) |
| Aceptación en primer plano | 7 min 03 s (21 %) | 0 (toda en segundo plano) |
| Revisión RDD | 6 min 38 s (19 %) | 8 min 01 s (19 %) |
| Correcciones y cierre | 1 min 58 s (6 %) | 14 min 29 s (34 %)² |

1. Sin el escritor en paralelo: su tiempo no quedó en el registro.
2. Las etiquetas de los dos últimos tramos quedaron corridas; solo vale la suma.

Escribir código fue la parte menor: entre un 25 % y un 32 % del activo. Verificar (aceptación, revisión RDD y
correcciones) ocupó cerca del 45 %. ODD ganó tiempo con el escritor en paralelo y con una corrida de aceptación
menos. Vibe coding perdió una corrida con un dato de prueba mal elegido (`ZZ` sí existe en el CSV) y, como ODD,
otra que escribió fuera de la carpeta `work`.

## Velocidad de las consultas

Los dos binarios de release, sobre el mismo conjunto de datos y en la misma máquina, sin otra carga. Mediana de
3 ejecuciones tras una de calentamiento, con `--count`. Los dos dan los mismos resultados en todas las consultas.

| Consulta | Resultados | ODD | Vibe coding |
|---|---|---|---|
| `--where "Primary Type=THEFT"` (sin prefijo) | 1.838.502 | 0,41 s | 0,41 s |
| `Case Number=JA` | 269.655 | 0,08 s | 0,09 s |
| `Primary Type=CRIM` | 1.256.646 | 0,27 s | 0,26 s |
| `CRIM` + `CRIMINAL D` | 1.256.646 | 0,28 s | 0,25 s |
| `THEFT` o `CRIM` | 3.095.148 | 0,61 s | 0,63 s |
| `District=00` | 4.077.929 | 0,79 s | 0,79 s |
| `Case Number=H` | 5.762.199 | 1,25 s | 1,26 s |
| `Location Description=resid` + agosto de 2019 | 5.310 | 0,18 s | 0,18 s |
| `THEFT` + `District=01` | 813.623 | **11,28 s** | 14,29 s |
| `ID=10695923` + `Case Number=H`¹ | 1 | 1,14–1,19 s | **0,08–0,11 s** |

1. Dos ejecuciones por binario, sin calentamiento; se lanzó después para comprobar el plan.

Los dos planes empatan salvo en dos casos, que muestran sus decisiones de diseño:

- **`THEFT` + `District=01`:** ODD hace que dirija el prefijo de distrito y comprueba `THEFT` con consultas
  puntuales por ID. Vibe coding hace que dirija `THEFT` y lee el registro de cada uno de los 1,8 millones para
  comprobar el distrito.
- **ID + prefijo:** ODD deja que el prefijo dirija aunque haya un ID, y recorre los 5,7 millones de números de caso
  que empiezan por `H`. Vibe coding hace que dirija el ID: es 13 veces más rápido.

La memoria es la misma en los dos: 15,2 MiB para el prefijo `H`, frente a 7,5 MiB para un solo número de caso.

## Calidad del código

Análisis de solo lectura de los dos diffs (`0971a37..d6e3ae6` y `0971a37..b3ba171`); los dos pasan
`cargo fmt`, `clippy -D warnings` y toda la suite.

| Métrica | ODD | Vibe coding |
|---|---|---|
| `crime-query`, código de producción (+/−) | +355 / −84 | +374 / −113 |
| `crime-query`, tests | +566 | +701 |
| `crime-acceptance` (selector), código / tests | +55 / +176 | +38 / +43 |
| Funciones nuevas en producción | 16 | 18 |
| Líneas de comentario o documentación nuevas | 94 | 79 |
| Función más larga tocada | `Execution::start`, unas 90 líneas | `Query::applied`, 56 líneas (31 en ODD) |
| `unwrap`/`expect` nuevos fuera de los tests | 0 | 0 |

| Dimensión | ODD | Vibe coding | Mejor |
|---|---|---|---|
| Reutilización | Deja duplicados pequeños: `malformed_prefix` casi igual a `malformed_condition`, dos conversores de valor de índice, la lista de campos repetida en el selector | Comparte más: `split_at_equals`, `malformed_condition` con la forma como parámetro, `Value::from_index` sustituye al conversor antiguo | Vibe coding |
| Modelo del prefijo | Listas de prefijos aparte; el tipo `Value` no cambia | Variante `Value::Prefix` dentro de los valores del filtro: más fácil de extender, pero ajena a fechas y números | Empate |
| Plan de ejecución | El prefijo dirige aunque haya ID (13 veces más lento en ese caso) | Dirigen el ID o la igualdad; lee el registro para comprobar el prefijo (27 % más lento con `THEFT` + `District=01`) | Empate |
| Coste por registro | Sin asignaciones nuevas en la comprobación | `Equality::matches` clona los prefijos en cada registro | ODD |
| Tests del consultor | De comportamiento, por la API pública | Igual, y además prueban el binario real: código de salida y mensaje de cada validación, mismos registros en tabla, CSV y JSON | Vibe coding |
| Tests del selector y de la aceptación | +176 líneas en el selector; script más completo | +43 líneas en el selector | ODD |
| Legibilidad | Documentación más extensa; `Query::applied` sencillo | Enum de tramos más plano; `Query::applied` es la función más difícil de leer de las dos ramas | ODD |
| Estabilidad del JSON | Mantiene `"values": []` y añade `"startswith"` | Usa `"prefixes"` y omite `"values"` cuando un campo solo tiene prefijos | ODD |

Ningún análisis encontró defectos de corrección en ninguna de las dos. Las dos evitan los duplicados igual: reducen
las alternativas de un campo a tramos disjuntos del índice. Las dos leen un tramo del índice cada vez, sin cargar
los ID en memoria. **Veredicto de calidad: empate.** ODD tiene una ligera ventaja en mantenibilidad y estabilidad
de la salida; vibe coding, en reutilización y en el alcance de sus tests.

## Diferencias de producto donde el PRD deja libertad

| Punto | ODD | Vibe coding | Qué dice el PRD |
|---|---|---|---|
| Clave JSON de los prefijos | `"startswith"` | `"prefixes"` | Solo que el JSON incluya la condición con el prefijo en minúsculas |
| Campo solo con prefijos en el JSON | `"values": []` | sin `"values"` | Nada |
| `--startswith Location=…` | "El campo … no admite condiciones" | "'Empieza por' solo se admite en campos de texto" | No menciona `Location` |
| Elección del plan | Dirige el prefijo, salvo si hay fecha | Dirigen la fecha, el ID o la igualdad; el prefijo, solo si no hay nada más | No exige plan (P5: sin objetivo de tiempo) |

## Pros y contras

**ODD**

- A favor:
  - Más rápido (−18 % de tiempo activo desde el arranque), gracias al escritor en paralelo.
  - El documento de ejecución deja trazados los requisitos, las decisiones (D1–D5) y las evidencias.
  - Selector y aceptación más probados.
  - El JSON no cambia de forma.
  - La comprobación por registro no hace asignaciones.
- En contra:
  - Exploración y plan se llevan el 21 % del tiempo.
  - La última refactorización no volvió a pasar por la aceptación.
  - Quedaron duplicados pequeños.
  - El plan ignora el ID cuando hay un prefijo.
  - El tiempo del escritor en paralelo no quedó medido.

**Vibe coding**

- A favor:
  - Menos ceremonia: sin documento ni subagentes, y exploración del 12 %.
  - Código más DRY.
  - Tests de extremo a extremo con el binario.
  - Mejor plan cuando hay ID o igualdad.
  - La aceptación final se corrió sobre el último código.
- En contra:
  - Un 22 % más de tiempo activo desde el arranque.
  - Tres corridas de aceptación, una de ellas por un dato de prueba mal elegido.
  - `Query::applied` es larga.
  - Clona prefijos en cada registro.
  - Cambia la forma del JSON en las condiciones con solo prefijos.
  - Las etiquetas del registro de fases quedaron corridas al final.

## Lecciones

1. **El PRD debe fijar toda salida pública**: claves y forma del JSON, y los mensajes de los campos que no
   menciona. Es la única fuente de divergencia visible entre las dos ejecuciones.
2. **La estimación sigue duplicando lo real.** Para un cambio de este tamaño, lo real fue de 31 a 37 minutos
   activos desde el arranque.
3. **Verificar cuesta más que programar.** Cerca del 45 % del tiempo se fue en aceptación, revisión y
   correcciones. Recortar ahí es lo que más acorta una ejecución.
4. **Las dos ejecuciones repitieron el mismo fallo de herramienta.** `-WorkDir work\\…` pasado desde bash dejó
   carpetas fuera de `work` (787 MB y 2,8 GB). Conviene usar rutas con `/` en los comandos de bash.
5. **El registro de fases funciona, pero exige disciplina.** Hay que marcar los subagentes como tareas en
   segundo plano y cambiar de fase en el momento en que se cambia de trabajo.

## Límites de esta comparación

- Una sola ejecución por enfoque y un solo cambio, pequeño y con un PRD claro. Con cambios grandes o
  ambiguos, el resultado puede ser distinto.
- Las dos ejecuciones corrieron a la vez en la misma máquina, así que compitieron por CPU y disco durante la
  aceptación.
- El binario de ODD medido es el de `c190bef`. La refactorización `d6e3ae6`, sin cambio de comportamiento, no
  estaba compilada en release. El de vibe coding corresponde al código final.

## Fuentes

- ODD: rama `feat/startswith-odd` (`4353af5`), con `odd/tasks/startswith.md`, `startswith-report.md` y
  `startswith-phases.tsv`.
- Vibe coding: rama `feat/startswith-vibe` (`d3d274f`), con
  `docs/reports/2026-10-18-startswith-report.md` y `2026-10-18-startswith-phases.tsv`.
- Velocidad de las consultas: medida el 2026-10-18 entre las 10:29 y las 10:32, con los binarios de release de
  cada copia y el conjunto de datos generado por la aceptación de vibe coding.
