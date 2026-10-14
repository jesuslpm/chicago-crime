# Consultor terminado: verificado y mergeado a `main`

Informe de cierre del feature `crime-query` (2026-10-13). El detalle completo (specs, decisiones, log y
resultados) está en [crime-query.md](crime-query.md).

`main` está en `c00ae9a` (merge fast-forward de `feat/crime-query`). **No se hizo ningún push.** Siguen sin
subir `293e891`, `4c74bfe` y los 13 commits del feature.

## Resultado

- **Especificación:** S1–S10 entregados, con 423 tests en verde.
- **Aceptación en el CSV de referencia:** 58/58 PASS (AC-C1…AC-C18) en tres ejecuciones, una de ellas del
  verificador independiente. Algunas cifras: THEFT 1.838.502, área comunitaria sin valor 613.720, consulta por
  ID en 39–51 ms. La aceptación del generador con esquema 2 sigue en 15/15.
- **Revisiones:** cada commit pasó por un verificador independiente sin blockers y tiene una revisión RDD
  aprobada y reconocida, con el consentimiento siempre concedido.
  - T1 se escaló a riesgo alto porque cambia el formato persistido de los índices.
  - El slice final excedía el presupuesto de los revisores y se partió en 3 rangos encadenados.
- **Hallazgos corregidos por el camino:**
  - `sstable` saltaba en silencio los bloques corruptos. Ahora se valida el CRC32C de cada bloque y un bloque
    corrupto da error.
  - Un bug del `seek` de `sstable` hacía que algunos rangos de fechas devolvieran resultados vacíos. Tiene un
    parche y un test que lo fija.
  - El CSV de salida escribía `""` en los campos vacíos, mientras que el origen escribe `,,`. Ahora las filas
    salen idénticas byte a byte.

## Tiempo

- Inicio a las 16:06:57, cierre a las ~19:22: **~3 h 15 min reales**, frente a una estimación de 4–7 h. No
  hubo esperas por el usuario.
- **Corrección:** en el `## Log` del documento del feature se anotaron primero franjas horarias estimadas que
  se adelantaban hasta 3 h al reloj real. Se reemplazaron por las horas de los commits (entrada L17) y quedó
  registrada la regla: en el log solo van tiempos medidos.

## Informe de cambios (`4c74bfe..main`)

| Área | + | − | Archivos |
|---|---|---|---|
| `crates/crime-query` | 7.937 | 0 | 23 |
| `crates/crime-acceptance` | 1.527 | 32 | 8 |
| `scripts/acceptance` | 1.335 | 10 | 3 |
| `odd/tasks` | 287 | 0 | 1 |
| `crates/crime-dataset` | 238 | 32 | 7 |
| `README.md` | 123 | 8 | 1 |
| `crates/crime-generator` | 69 | 21 | 3 |
| **Total** | **11.516** | **103** | **46** |

`Cargo.lock`, aparte: +16. La previsión era de ~3.500 líneas y salieron 11.619 (3,3×): cerca de la mitad son
tests y 1.249 son el script de aceptación.

## Resumen

El cambio añade `crime-query`, el consultor de una sola ejecución:

- Abre la versión activa del conjunto como snapshot, así que una publicación concurrente no lo afecta, y
  rechaza el esquema 1.
- Las condiciones se escriben con `--where CAMPO=VALOR` y `--range Date=DESDE..HASTA`. El texto no distingue
  mayúsculas y `CAMPO=` significa "sin valor".
- Resuelve la consulta con el plan fijo de C10: recorre el índice de fecha y comprueba las demás condiciones
  contra sus índices, o, sin fecha, hace unión e intersección de flujos de ID ordenados. Los filtros sin índice
  se aplican al registro.
- La memoria no crece con el resultado: el pico va de 7 a 16 MiB.
- Escribe tabla, CSV o JSON (con la consulta interpretada) por stdout o en un archivo que se reemplaza de forma
  atómica. Los avisos y las estadísticas van por stderr, y los códigos de salida son 0/1/2/130.
- Incluye el cambio C1 en el generador (índices de texto en minúsculas, esquema 2), un oráculo independiente
  sobre el CSV (`crime-acceptance select`) y el script de aceptación.

## Valores por defecto elegidos sin consulta

- **Sintaxis:** la de la CLI descrita arriba.
- **Block, Description y Location:** rechazados con un mensaje nuevo, "no admite condiciones".
- **Igualdad en fechas:** abarca el día o el minuto entero.
- **Booleanos:** aceptan `true`, `false`, `sí`, `si` y `no`.
- **Códigos de salida:** 2 para errores de opciones, 1 para errores en el contenido de las condiciones.
- **`--count`:** imprime el número a secas.
- **Tabla:** anchos de columna fijos, para poder ir escribiendo sin cargar todo en memoria.

Todo está documentado en [crime-query.md](crime-query.md).

## Pendientes (no bloqueantes)

- **Oráculo `select --rows-out`:** etiqueta un error de lectura como si fuera de escritura, y no copia byte a
  byte un origen con BOM o con finales de línea mezclados.
- **AC-C1:** no deduplica los IDs de muestra. Con otro CSV compatible podría dar un FAIL falso; en el de
  referencia son distintos.
- **Archivo temporal:** el nombre suma 24 caracteres al del destino, así que un nombre cerca del límite de 255
  se reporta como no escribible.
- **Límites aceptados:**
  - Los valores no se recortan.
  - Si falla una lectura a mitad de consulta, las filas que ya salieron por stdout quedan ahí.
  - El plan guiado por fecha recorre todo el rango aunque se busque un solo ID.

**Riesgo:** items 1 y 3. Cambia el formato persistido de los índices (los conjuntos con esquema 1 hay que
regenerarlos) y se añade una CLI nueva. Ambos cambios están cubiertos por tests, verificadores, revisión y la
aceptación.

## Limpieza manual

`rm -rf` estaba denegado en la sesión, así que quedaron carpetas temporales que se pueden borrar:
`target/query-acceptance/dev` (~3,5 GB) y los clones de los verificadores bajo `%TEMP%\tmp.*` (el más grande,
~5,5 GB).
