# Almacén consultable de delitos de Chicago sobre SSTable — PRD

**Versión**: 1.2
**Fecha**: 2026-10-12
**Documento padre**: ninguno

## Objetivo y contexto

Construir dos aplicaciones de línea de comandos en Rust que convierten el histórico de delitos de Chicago
(8,66 millones de registros, 2,3 GB de CSV) en un conjunto de datos ordenado y consultable sin base de datos:

1. **Generador**: lee el CSV una sola vez y produce una SSTable principal ordenada por ID, más una SSTable
   por cada índice secundario.
2. **Consultor**: responde preguntas sobre ese conjunto de datos (por ID, por número de caso, por valores de
   campos indexados y por rangos de fechas) sin cargarlo entero en memoria.

**Por qué**: el CSV no permite búsquedas eficientes; cada pregunta obliga a recorrer 2,3 GB. Un almacén
ordenado e inmutable permite consultas puntuales y por rango con una fracción de esa lectura, y sirve como
caso práctico de aprendizaje de Rust, ordenación externa y codificación de tuplas de FoundationDB.

**Restricción principal**: el generador debe funcionar con un pico de memoria de **512 MiB**, aunque el
conjunto de datos no quepa en ese espacio. Es una restricción de diseño deliberada, no una limitación del
equipo actual. El límite se refiere al pico de memoria privada comprometida del proceso generador, con el
runtime y las bibliotecas incluidos y sin contar la caché de archivos del sistema operativo. En Windows se
mide objetivamente como el máximo de bytes privados comprometidos del proceso. Superar el límite no
detiene la generación: se anota un aviso en el log y el resumen final lo indica.

### Estado de partida

Verificado el 2026-10-12 sobre el repositorio y los datos:

| Aspecto | Situación |
|---|---|
| Código | No existe código Rust ni proyecto creado. Solo hay el CSV y las notas de diseño previas. |
| Origen | Extracto de *Crimes — 2001 to Present* del portal de datos de Chicago, actualizado el 2026-10-10. |
| Volumen | 8.656.904 registros, 22 columnas en todas las filas; la fila más larga ocupa 305 caracteres. |
| Identificador | Todos los ID son enteros (de 634 a 14.353.993) y **no hay duplicados**. |
| Orden | El CSV **no** viene ordenado por ID (2,5 millones de saltos hacia atrás). La ordenación es obligatoria. |
| Número de caso | **No es único**: 632 registros repiten un número de caso ya visto. |
| Fechas | Todas con el mismo formato `MM/DD/AAAA hh:mm:ss AM/PM`, hora local de Chicago. Años 2001–2026. |
| Valores vacíos | Coordenadas y ubicación: 100.851 · Distrito electoral (Ward): 614.809 · Área comunitaria: 613.720 · Descripción de ubicación: 16.758 · Distrito policial: 47. |
| Cardinalidades | Tipo principal: 34 · Distrito: 26 · Área comunitaria: 78 · Ward: 51 · FBI: 26 · Beat: 306 · IUCR: 418 · Descripción de ubicación: 219. |
| Equipo de referencia | Windows 11, 32 hilos, 64 GB de RAM. |

**Huecos de producto**: ninguno; las decisiones están en [Decisiones](#decisiones).

## Actores

- **Operador del generador**: ejecuta el generador sobre un CSV descargado y obtiene un conjunto de datos
  listo para consultar.
- **Analista**: usa el consultor para buscar delitos y contar resultados.
- **Portal de datos de Chicago** (externo): publica el CSV. No hay integración directa; el operador descarga
  el archivo a mano.

## Flujo

### A. Generación

1. El operador indica el CSV de entrada, la carpeta de salida y, opcionalmente, una carpeta temporal para
   la ordenación.
2. El generador valida la cabecera antes de procesar ninguna fila.
3. Recorre el CSV en streaming, convierte cada campo a su tipo (ver [Reglas de los campos](#reglas-de-los-campos))
   y muestra el progreso (filas procesadas, fase actual). Cada registro inválido se anota en el log con su
   fila, columna, motivo y contenido original, y se omite sin detener el proceso (decisión D5).
4. Ordena los registros por ID numérico ascendente usando disco temporal cuando no caben en memoria.
5. Escribe la **SSTable principal**: clave = tupla FoundationDB con el ID; valor = tupla FoundationDB plana
   con todos los campos tipados en el orden del CSV, incluido el ID y sin Location (decisión D4).
6. Escribe una **SSTable por índice secundario**: fecha, tipo principal, distrito, área comunitaria y
   número de caso (decisión D2). Cada entrada relaciona un valor del campo indexado con un ID. Dentro de un
   mismo valor, los ID aparecen en orden creciente. Los valores vacíos se indexan como "sin valor" para que
   también sean consultables.
7. Escribe un **manifiesto** que describe el conjunto: versión del esquema, origen, fecha de generación,
   número de registros válidos y rechazados, e índices incluidos con su número de entradas.
8. Solo si todo terminó bien, publica el conjunto en la carpeta de salida de forma atómica: nunca queda un
   conjunto a medias visible. Una generación fallida, incluida una interrupción inesperada del proceso, no
   deja visible un conjunto incompleto ni invalida el conjunto anterior, que sigue siendo consultable. No se
   exige durabilidad frente a cortes de alimentación. El mecanismo técnico se elige durante la
   implementación.
9. Muestra un resumen final: registros válidos y rechazados, duración, pico de memoria y tamaño de cada
   archivo.

### B. Consulta

1. El analista ejecuta un comando con la carpeta del conjunto de datos y la consulta; el consultor
   responde y termina (decisión D6).
2. El consultor comprueba el manifiesto y que la versión del esquema sea compatible.
3. Tipos de consulta de la primera versión:
   - **Por ID**: uno o varios.
   - **Por número de caso**: devuelve todos los registros que lo comparten.
   - **Por igualdad** sobre un campo indexado, con uno o varios valores alternativos (O lógico).
   - **Por rango de fechas** (desde/hasta, ambos inclusivos).
   - **Combinación** de varias condiciones anteriores con Y lógico.
4. Opciones de salida: límite de resultados, solo conteo, y formato tabla legible, CSV o JSON.
5. Los resultados se devuelven en orden ascendente de ID.
6. El consultor informa del número de resultados, del tiempo empleado y de los índices usados.

### Reglas de los campos

| Campo | Tipo de negocio | Regla |
|---|---|---|
| ID | Entero | Obligatorio y único. Es la clave y además el primer campo del valor, para que el valor sea autocontenido. |
| Case Number | Texto | Obligatorio. No es único. |
| Date, Updated On | Fecha-hora | Hora civil local de Chicago tal como se publica, sin zona horaria ni conversión (decisión D3). Las horas inexistentes o repetidas del cambio de horario se conservan sin error. |
| Block, Primary Type, Description | Texto | Obligatorios. |
| Location Description | Texto | Opcional. |
| IUCR, Beat, District, FBI Code | Código | Texto que conserva los ceros a la izquierda (`0314`, `003`, `08B`). District es opcional. |
| Arrest, Domestic | Sí/No | Obligatorios. |
| Ward, Community Area | Entero | Opcionales. |
| X Coordinate, Y Coordinate | Entero | Opcionales. |
| Latitude, Longitude | Decimal | Opcionales. |
| Year | Entero | Obligatorio. |
| Location | Texto derivado | Redundancia de la entrada ("(latitud, longitud)"): no se almacena. |

## Validaciones y mensajes

### Generador

| Situación | Regla | Mensaje al usuario |
|---|---|---|
| Cabecera distinta de la esperada | Aborta antes de procesar filas | `La cabecera no coincide con el esquema: columna 5 esperaba "IUCR" y encontró "<valor>".` |
| Fila con número de columnas distinto de 22 | Se anota en el log y se omite | `Fila 1234 omitida: se esperaban 22 columnas y hay 21.` |
| ID no numérico | Se anota en el log y se omite | `Fila 1234 omitida: el ID "<valor>" no es un número entero.` |
| ID duplicado | Se conserva la primera aparición en el archivo; las demás se anotan y se omiten | `Fila 5678 omitida: el ID 14348063 ya apareció en la fila 1234.` |
| Campo tipado no convertible | Se anota en el log y se omite | `Fila 1234 omitida, columna "Date": "<valor>" no es una fecha válida (formato MM/DD/AAAA hh:mm:ss AM/PM).` |
| Campo obligatorio vacío | Se anota en el log y se omite | `Fila 1234 omitida: la columna "Case Number" es obligatoria y está vacía.` |
| Campo opcional vacío | Se guarda como "sin valor" | — |
| Generación terminada con rechazos | El conjunto se publica sin límite de rechazos; el resumen y el manifiesto indican cuántos registros se rechazaron | `Generación completada: 8.656.900 registros válidos, 4 rechazados (ver log).` |
| El pico de memoria supera 512 MiB | No detiene la generación; se anota un aviso en el log y el resumen final lo indica | `Aviso: el pico de memoria privada comprometida (580 MiB) supera el límite de 512 MiB.` |
| La carpeta de salida ya contiene un conjunto | Solo se reemplaza con confirmación explícita | `La carpeta de salida ya contiene un conjunto de datos. Usa la opción de sobrescritura para reemplazarlo.` |
| Falta espacio en disco temporal o de salida | Aborta y limpia temporales | `Espacio insuficiente en <carpeta>.` |
| Cualquier aborto | No deja conjunto parcial; el conjunto anterior, si existía, sigue intacto | `Generación cancelada. No se ha modificado el conjunto existente.` |

### Consultor

| Situación | Regla | Mensaje al usuario |
|---|---|---|
| Carpeta sin conjunto o incompleta | Rechaza | `No se encontró un conjunto de datos válido en <carpeta>.` |
| Versión de esquema incompatible | Rechaza | `El conjunto usa el esquema v2 y este consultor admite v1. Regenera el conjunto.` |
| ID inexistente | Resultado vacío, no error | `No existe ningún delito con ID 123.` |
| Campo desconocido | Rechaza | `El campo "<nombre>" no existe. Campos disponibles: ...` |
| Valor con formato inválido | Rechaza | `"<valor>" no es una fecha válida. Formato esperado: AAAA-MM-DD [hh:mm].` |
| Rango de fechas invertido | Rechaza | `La fecha inicial es posterior a la final.` |
| Condición sobre un campo sin índice | Se permite, avisando | `Aviso: "<campo>" no tiene índice; la consulta recorrerá todos los registros.` |

## Reutilización

- **Notas de diseño previas** (conversación resumida): ordenación por ID numérico, codificación de tuplas
  FoundationDB para clave y valor, tupla plana con nulos explícitos, presupuesto de 512 MiB y segmentos de
  ordenación medidos.
- **Codificación de tuplas de FoundationDB**: se reutiliza solo su formato de serialización, porque conserva
  el orden de enteros, textos y fechas al comparar bytes. No se usa FoundationDB como base de datos.
- **Formato de tabla de LevelDB** (decisión D1): todas las SSTables usan el formato de archivo de tabla de
  LevelDB, con orden de claves byte a byte, sumas de verificación por bloque y compresión Snappy opcional.
  No se inventa un formato propio. Se genera con una biblioteca Rust pura, sin dependencias de C++.

## Fuera de alcance

- Actualizaciones incrementales, borrados o mezcla de extractos: cada cambio de datos implica regenerar
  el conjunto completo.
- Descarga automática del CSV desde el portal.
- Conexión con un clúster real de FoundationDB.
- Abrir el conjunto como base de datos LevelDB o ingerirlo en RocksDB: los archivos son tablas LevelDB
  legibles una a una, no una base de datos completa.
- Servidor de red, API, interfaz gráfica o modo interactivo del consultor.
- Búsqueda de texto libre (en Block o Description) y búsqueda aproximada.
- Consultas geoespaciales (radio, polígono, cuadrícula).
- Agregaciones y agrupaciones más allá del conteo de resultados.
- Índices compuestos (por ejemplo, tipo + fecha) y orden de resultados distinto del ID.
- Control de acceso: los datos son públicos.

## Criterios de aceptación

Las cifras exactas de AC1, AC4 y AC5 corresponden al CSV de referencia analizado el 2026-10-12 (ver
[Estado de partida](#estado-de-partida)). Con otro CSV compatible, esos resultados deben coincidir con los
registros válidos realmente procesados.

1. **AC1** — Dado el CSV completo, cuando se ejecuta el generador, entonces la SSTable principal contiene
   8.656.904 entradas en orden estrictamente creciente de ID.
2. **AC2** — Dado el CSV completo, cuando se ejecuta el generador, entonces el pico de memoria privada
   comprometida medido (ver Restricción principal) no supera 512 MiB y el resumen final lo muestra.
3. **AC3** — Dada una muestra de ID (el primero, el último, uno sin coordenadas y uno sin Ward), cuando se
   consultan por ID, entonces cada campo devuelto es equivalente en valor al de la fila del CSV.
4. **AC4** — Dado el índice de tipo principal, cuando se cuenta `THEFT`, entonces el resultado es 1.838.502,
   igual que en el CSV.
5. **AC5** — Dado el índice de área comunitaria, cuando se cuentan los registros "sin valor", entonces el
   resultado es 613.720.
6. **AC6** — Dado un rango de fechas de un mes, cuando se consulta, entonces el conteo coincide con el
   obtenido filtrando el CSV directamente.
7. **AC7** — Dada una combinación de dos condiciones indexadas (por ejemplo `THEFT` y distrito `008`), cuando
   se consulta, entonces el conteo coincide con el filtrado directo del CSV.
8. **AC8** — Dado un número de caso compartido por varios registros, cuando se consulta, entonces se
   devuelven todos.
9. **AC9** — Dado un CSV con la cabecera alterada, cuando se ejecuta el generador, entonces aborta con el
   mensaje de la tabla y el conjunto anterior permanece intacto.
10. **AC10** — Dado un conjunto generado, cuando se consulta un ID existente, entonces la respuesta llega en
    menos de 1 segundo en el equipo de referencia.
11. **AC11** — Dada cualquier consulta, cuando se pide en CSV o JSON, entonces la salida es válida para ese
    formato y contiene los mismos registros que la salida en tabla.
12. **AC12** — Dado un registro cuya hora cae en el salto de primavera (inexistente en Chicago), cuando se
    genera el conjunto y se consulta por ID, entonces la generación no falla y la fecha devuelta es
    exactamente la publicada.
13. **AC13** — Dado un CSV con una fila de 21 columnas, una fecha inválida y un ID repetido, cuando se ejecuta
    el generador, entonces termina con éxito, el conjunto excluye esas tres filas, el log identifica cada una
    con su fila y motivo, y el resumen y el manifiesto indican 3 rechazados.

## Decisiones

No queda ninguna decisión abierta. Todas se tomaron el 2026-10-12 y ya están reflejadas en el cuerpo del PRD.

- **D1 — Formato de las SSTables** (2026-10-12): formato de tabla de LevelDB generado con Rust puro.
  Motivo: es un formato estándar y estable, cualquier lector de tablas LevelDB puede leer los archivos y
  compila en Windows sin toolchain de C++. Se descartó la compatibilidad con RocksDB por esa fricción de
  compilación.
- **D2 — Índices secundarios iniciales** (2026-10-12): fecha, tipo principal, distrito, área comunitaria y
  número de caso. Motivo: la fecha habilita consultas por rango y los demás son los filtros habituales con
  cardinalidad útil. Arrest y Domestic (Sí/No) quedan sin índice porque cada valor abarca millones de
  registros; se pueden usar como condición con recorrido completo.
- **D3 — Política de fechas** (2026-10-12): se guarda la hora civil local de Chicago tal como se publica,
  sin convertir a UTC, y las consultas se expresan también en hora local. Motivo: la fuente no indica zona
  ni desfase horario. Según lo medido, 253 registros tienen una hora inexistente (el salto de primavera) y
  1.663 caen en la hora repetida de otoño. Convertir a UTC obligaría a abortar o a inventar el instante.
  Consecuencia aceptada: las fechas no son instantes absolutos y no se comparan con datos de otras zonas.
- **D4 — Contenido del valor** (2026-10-12): el valor incluye el ID como primer campo y omite Location.
  Motivo del ID: el valor queda autocontenido y sigue el orden del CSV, lo que simplifica generar CSV o
  JSON. Cuesta 33 MiB (2,9 % del valor) y su efecto en memoria y CPU es despreciable. Motivo de Location:
  es una redundancia de la entrada que ocuparía 251 MiB (22 % del valor).
- **D5 — Registros inválidos** (2026-10-12): cada registro inválido se anota en el log y se omite; la
  generación continúa. Solo abortan los errores que afectan a todo el archivo: cabecera distinta, archivo
  ilegible o falta de espacio. Ante un ID duplicado se conserva la primera aparición en el archivo, por
  lejos que estén las repeticiones. No hay límite de rechazos. Motivo: los datos reales siempre traen
  registros malos, y abortar impediría aprovechar los buenos. Consecuencia aceptada: un archivo con la
  cabecera correcta pero el contenido roto genera un conjunto casi vacío que reemplaza al anterior; el
  resumen con el número de rechazados es la señal para detectarlo.
- **D6 — Forma de uso del consultor** (2026-10-12): comandos de una sola ejecución; cada consulta arranca,
  responde y termina. Motivo: es lo más simple de automatizar, de encadenar con otras herramientas y de
  probar. El modo interactivo queda para un cambio posterior.

## Referencias

- Notas de diseño previas: `conversacion_rust_csv_foundationdb_sstable.md` (raíz del repositorio).
- Crimes — 2001 to Present: https://data.cityofchicago.org/Public-Safety/Crimes-2001-to-Present/ijzp-q8t2
- FoundationDB, modelado de datos y capa de tuplas: https://apple.github.io/foundationdb/data-modeling.html
