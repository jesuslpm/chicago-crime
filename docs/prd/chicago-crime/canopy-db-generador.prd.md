# Generador CanopyDB — PRD

**Versión**: 1.0
**Fecha**: 2026-10-21
**Documento padre**: [Almacén de delitos de Chicago sobre CanopyDB — PRD](canopy-db.prd.md) (v1.0)

## Objetivo y contexto

Entregar el primer hijo del módulo CanopyDB (orden 1 de [Reparto en PRD hijos](canopy-db.prd.md#reparto-en-prd-hijos)):

- el **generador CanopyDB**, una aplicación nueva que hace lo mismo que el generador original pero guarda el
  conjunto en una base CanopyDB en lugar de en SSTables;
- un **verificador CanopyDB**, independiente del generador, que lee la base y la compara con el CSV y con otro
  conjunto de cualquiera de los dos motores;
- su **verificación de aceptación** con las cifras de referencia;
- el **registro de hallazgos** del módulo, dado de alta con H1–H4 y sus reproducciones.

El generador CanopyDB hereda el contrato del [PRD del generador](generador.prd.md) (decisión K1 del padre): las
mismas opciones, mensajes, salidas y códigos de salida, y las reglas del documento raíz, del [PRD de soporte para
Linux](soporte-linux.prd.md) y del [PRD de aceptación con cualquier CSV](aceptacion-cualquier-csv.prd.md). Este
documento no las repite. Solo fija lo que el padre le asigna: las excepciones que dependen del formato (nombre,
forma de la carpeta publicada y del manifiesto, lista de archivos del resumen y opción sin compresión) y las
decisiones K2, K4, K9 y K10 aplicadas al generador.

**Por qué ahora**: es la base del módulo. El consultor CanopyDB, el banco de comparación y la búsqueda de bugs
necesitan un conjunto CanopyDB publicado con el mismo contenido lógico que el original.

### Estado de partida

Verificado el 2026-10-21 sobre el repositorio y sobre el código fuente de CanopyDB 0.2.5:

| Aspecto | Situación |
|---|---|
| Generador original | Implementado y aceptado. Opciones: CSV de entrada, carpeta de salida, carpeta temporal, log, sobrescritura y sin compresión, más tres opciones ocultas de ajuste: tamaño de los segmentos de ordenación de registros y de índices, y umbral del aviso de memoria (512 MiB por defecto). La ayuda dice que genera "SSTables y manifiesto". |
| Carpeta publicada por el original | En la carpeta de salida, un puntero a la versión activa y una carpeta por versión con nombre `dataset-<AAAAMMDD>-<hhmmss>-<proceso>` (con un sufijo `-<n>` si ya existe). La carpeta de versión contiene la tabla principal, los cinco índices y `manifest.json`. Publica cambiando el puntero en un solo paso y después borra la versión anterior; si no puede, lo anota en el log y la limpia una generación posterior. Mientras trabaja, bloquea la carpeta de salida; una segunda generación sobre la misma carpeta aborta con `Otra generación está usando la carpeta de salida <carpeta>.` |
| Clasificación de la carpeta de salida en el original | Si existe el puntero, la carpeta "ya contiene un conjunto", aunque el puntero no lleve a una versión válida. Una carpeta de versión que el puntero no nombra y el puntero temporal son restos. Cualquier otra entrada es contenido ajeno y se rechaza siempre, con o sin sobrescritura. Solo mira el primer nivel de la carpeta de salida, no el contenido de las carpetas de versión. |
| Manifiesto del original | JSON con sangría y un salto de línea final, con `schema_version` (hoy 2), `generated_at`, `source` (nombre, tamaño y SHA-256 del CSV), `records` (válidos y rechazados), `compression` (`snappy` o `none`), `primary` e `indexes` (archivo, entradas y tamaño de cada tabla). Todos los campos son obligatorios al leerlo; los desconocidos se toleran. No indica el motor. |
| Consultor original ante un manifiesto ajeno | Localiza la versión activa por el puntero y lee del manifiesto **solo `schema_version`** primero, como un entero sin signo. Si es un entero distinto de 2, rechaza con `El conjunto usa el esquema v<N> y este consultor admite v2. Regenera el conjunto.` En cualquier otro caso de manifiesto inválido (`schema_version` que falta o que no es un entero sin signo, como un texto; JSON mal formado; o un resto del manifiesto sin la forma del original, por ejemplo sin `primary` o `indexes`), rechaza con `No se encontró un conjunto de datos válido en <carpeta>.` Los dos con el código 1. Solo después abre las tablas. |
| Resumen final del original | Líneas en este orden: registros válidos y rechazados, duración, pico de memoria contra el límite, carpeta publicada, una línea `Archivo <nombre>: <bytes> bytes (<MiB>).` por cada tabla y por el manifiesto, y ubicación del log. El log se muestra también en la salida de errores. |
| Errores de escritura en el original | Falta de espacio: `Espacio insuficiente en <carpeta>.`; cualquier otro error al escribir en la carpeta de salida: `No se puede escribir en la carpeta de salida <carpeta>.`; en la carpeta temporal, `No se puede usar la carpeta temporal <carpeta>.`. Después, `Generación cancelada. No se ha modificado el conjunto existente.` y el código 1. El detalle técnico del error no se muestra. |
| Restos de generaciones interrumpidas en el original | Descarta las carpetas de versión huérfanas y el puntero temporal (`Se han descartado restos de una generación interrumpida anterior.`, una vez aunque haya varios) y, en la carpeta temporal, sus carpetas de ordenación de un proceso que ya no está en marcha (`Se han descartado carpetas temporales de una generación interrumpida anterior.`). |
| Verificador original | Lee las SSTables con un lector de tablas LevelDB independiente y compara con el CSV. Escribe una fila por comprobación (`PASS`, `FAIL` o `SKIP`, su identificador y un resumen) y una línea final `RESULT:`; termina con 0, 1 si alguna falla, o 2 si la línea de comandos no es válida. Un `SKIP` no cuenta como fallo para él; las verificaciones de aceptación sí lo cuentan (decisión A3). Exige exactamente los siete archivos del original y `compression` `snappy` o `none`, así que no sirve para una base CanopyDB. |
| Verificación de aceptación del original | Script de Windows que comprueba AC-G1 a AC-G10: memoria medida desde fuera y en el resumen, dos generaciones idénticas byte a byte, interrupción, cancelación, sobrescritura y CSV de prueba. Exige unos 6 GB libres en la carpeta de trabajo. |
| CanopyDB en el proyecto | No es una dependencia todavía. No existe el registro de hallazgos ni ninguna reproducción de H1–H4. |
| Archivos de una base CanopyDB 0.2.5 | Una base es una carpeta con un archivo de bloqueo, los archivos del registro de escritura anticipada y una subcarpeta por cada base de datos interna, con su archivo de datos y sus opciones. El nombre de esa subcarpeta lleva un identificador aleatorio, así que cambia en cada generación. Al abrir la base, CanopyDB trata cada subcarpeta como una base de datos interna; abrirla crea el archivo de bloqueo si falta y puede cambiar el tamaño de los archivos internos. |
| Compresión en CanopyDB 0.2.5 | Solo comprime con LZ4 los valores mayores que un umbral (12 KiB por defecto); los valores de hasta 4 KiB no se comprimen nunca, y las páginas del árbol tampoco. Los registros de este conjunto ocupan unos 138 bytes, así que ninguno se comprime, con la opción activada o no. |
| Memoria de CanopyDB | La caché de páginas crece bajo demanda hasta 1 GiB por defecto (padre, H4); es configurable. |
| Hallazgos del padre | H1 (README de CanopyDB, tercer ejemplo) y H4 (el tamaño por defecto de la caché solo aparece en la documentación de esa opción, no en la portada) son de documentación. H2 y H3 se comprobaron en el equipo de referencia el 2026-10-20, sin un programa de reproducción guardado. |

**Huecos de producto**: ninguno abierto. Cada punto de la columna "Fija" del padre queda como regla en este
documento o en [Queda para la implementación](#queda-para-la-implementación).

## Actores

- **Operador del generador**: el del padre. Ejecuta el generador CanopyDB igual que el original.
- **Desarrollador**: ejecuta el verificador y la verificación de aceptación, y da de alta los hallazgos.

## Flujo

### Generación

Sigue el flujo A del padre y, por K1, el flujo del PRD del generador. Esta entrega concreta:

1. El operador ejecuta `crime-canopy-generator` (KG1) con las mismas opciones que el original, incluidas las
   ocultas, con los mismos valores por defecto y el mismo significado (K1); el umbral de memoria solo decide el
   aviso, como en el original. La ayuda es la del original con tres cambios: el nombre de la aplicación, la
   descripción, que dice `Genera el conjunto de datos de delitos de Chicago (base CanopyDB y manifiesto) a
   partir del CSV.`, y el texto de la opción sin compresión (KG4).
2. Antes de procesar ninguna fila hace las mismas comprobaciones que el original sobre el CSV, el log, la
   carpeta temporal y la carpeta de salida. Bloquea la carpeta de salida igual que el original, de modo que el
   generador original y el CanopyDB no pueden trabajar a la vez sobre la misma carpeta: el segundo aborta con el
   mensaje del original. La carpeta de salida se clasifica como en el original y un conjunto del otro motor
   cuenta como conjunto existente (KG5). Descarta los restos de generaciones interrumpidas (KG6).
3. Valida, convierte, rechaza, ordena los registros y ordena las entradas de los índices con las mismas reglas,
   mensajes, líneas de progreso y presupuesto de memoria que el original.
4. Escribe una base CanopyDB en una carpeta de versión nueva, con la forma de KG2: la tabla principal y los
   cinco índices, con las mismas claves y los mismos valores que las SSTables (K2), incluidos los índices de
   texto en minúsculas. Con un CSV que solo tiene la cabecera, avisa como el original y publica la base con
   las seis tablas vacías.
5. Deja la base lista para leer: abrirla después para leer no añade archivos a la carpeta de versión, no cambia
   el manifiesto ni el contenido lógico (K6 del padre). Lo que sí puede cambiar es el contenido o el tamaño de
   los archivos internos del motor.
6. Escribe el manifiesto CanopyDB (KG3) y publica de forma atómica, igual que el original: cambia el puntero en
   un solo paso y después borra la versión anterior; si no puede, lo anota en el log con el aviso del original y
   la limpia una generación posterior.
7. Muestra el resumen final del original, con la lista de archivos de la carpeta de versión (ver
   [Salida](#salida)). El pico de memoria se mide y se compara con 512 MiB igual que en el original (K2); si
   CanopyDB impide cumplirlo, se aplica K10 (KG10).
8. Una cancelación con Ctrl+C o una interrupción brusca se comportan como en el original, también mientras
   CanopyDB escribe o vuelca la base: la cancelación borra la carpeta de versión nueva y termina con 130; una
   interrupción brusca deja una carpeta de versión huérfana que la siguiente generación descarta (KG6).
9. Ante un error del motor, usa el mensaje existente más cercano y anota el detalle en el log (KG7).

### Verificador CanopyDB

1. El desarrollador ejecuta el verificador CanopyDB, `crime-canopy-acceptance` (KG12), sobre el CSV y la carpeta
   de salida. El verificador lee la
   base sin código del generador y hace sobre el contenido las mismas comprobaciones que el verificador original
   (AC-G1, AC-G3 y AC-G4, con y sin las cifras de referencia), además de comprobar el manifiesto CanopyDB.
2. En su modo de comparación recibe dos carpetas de salida, cada una de cualquiera de los dos motores, y
   compara tabla por tabla y entrada por entrada las claves y los valores de sus versiones activas (KG8).
3. Escribe el informe con el formato del verificador original y termina con sus mismos códigos (ver
   [Salida](#salida)). Sobre una base CanopyDB se ejecuta de uno en uno (K6).

### Registro de hallazgos

1. El desarrollador crea el registro de hallazgos del módulo en el lugar y con la forma de KG9, con los campos
   que fija el padre, y da de alta H1–H4 con la fecha en que el padre los anotó (2026-10-20), la versión 0.2.5
   y el estado "Detectado". Un hallazgo nuevo lleva la fecha del día en que se da de alta. Los campos sin
   contenido llevan `—`.
2. Cada hallazgo tiene su reproducción mínima:
   - H1: la cita del README de CanopyDB 0.2.5 con el ejemplo afectado;
   - H4: la cita de la documentación de la opción del tamaño de la caché y la constatación de que la portada no
     lo menciona;
   - H2: un programa que abre la base en un proceso y, mientras la tiene abierta, intenta abrirla desde un
     segundo proceso, que falla a los 5 segundos;
   - H3: un programa que muestra que no se puede abrir para leer una base en una carpeta de solo lectura, y que
     abrir y cerrar sin escribir una base cerrada sin volcar su registro de escritura anticipada cambia sus
     archivos.
3. Las reproducciones de H2 y H3 se ejecutan con una orden, sin el resto del proyecto.
4. Un hallazgo nuevo que aparezca durante esta ejecución se da de alta igual, con el siguiente identificador.

## Validaciones y mensajes

Se aplican todas las validaciones y mensajes del documento raíz, del PRD del generador y del PRD de soporte para
Linux, sin cambiar una palabra. Esta entrega solo fija cómo se aplican a los casos propios de CanopyDB:

| Situación | Regla | Mensaje al usuario |
|---|---|---|
| La carpeta de salida tiene un conjunto SSTable, sin sobrescritura | Se rechaza sin tocar nada, como un conjunto propio (KG5) | `La carpeta de salida ya contiene un conjunto de datos. Usa la opción de sobrescritura para reemplazarlo.` |
| La carpeta de salida tiene un conjunto SSTable, con sobrescritura | Se reemplaza como un conjunto propio; la versión SSTable se borra después de publicar (KG5) | (resumen habitual) |
| Carpetas de versión huérfanas, de cualquiera de los dos motores | Se descartan como restos, con un solo mensaje (KG6) | `Se han descartado restos de una generación interrumpida anterior.` |
| Carpetas de ordenación del generador CanopyDB de un proceso que ya no está en marcha | Se descartan (KG6) | `Se han descartado carpetas temporales de una generación interrumpida anterior.` |
| El generador original trabaja sobre la misma carpeta de salida | Aborta sin tocar nada | `Otra generación está usando la carpeta de salida <carpeta>.` |
| Error del motor por falta de espacio | Aborta; el conjunto anterior sigue intacto (KG7) | `Espacio insuficiente en <carpeta de salida>.`, o `Espacio insuficiente en <carpeta temporal>.` si el error viene de los archivos temporales del motor |
| Cualquier otro error del motor al crear, escribir, volcar o cerrar la base, incluido un fallo interno o un pánico del motor | Aborta con el código 1, nunca con el de un pánico; el conjunto anterior sigue intacto (KG7) | `No se puede escribir en la carpeta de salida <carpeta>.` |
| Cualquier otro error del motor en sus archivos temporales | Aborta como un error de la carpeta temporal (KG7) | `No se puede usar la carpeta temporal <carpeta>.` |
| Detalle de un error del motor | Una sola línea en el log, que también sale por la salida de errores, justo antes del mensaje de la causa; los saltos de línea del texto del motor se sustituyen por espacios (KG7) | `Detalle del error de CanopyDB: <texto del error tal como lo da CanopyDB>` |
| El pico de memoria supera el límite (512 MiB por defecto) | Igual que en el original: aviso en el log, el resumen lo indica y la generación termina con 0 (KG10) | `Aviso: el pico de memoria privada comprometida (<cifra> MiB) supera el límite de <límite> MiB.` |
| El consultor original recibe un conjunto CanopyDB | Lo rechaza con el código 1, sin ningún cambio en el consultor original (KG3) | `No se encontró un conjunto de datos válido en <carpeta>.` |

Como en el original, cualquier aborto termina con `Generación cancelada. No se ha modificado el conjunto
existente.`, salvo la cancelación con Ctrl+C, que solo muestra `Generación cancelada por el operador. No se ha
modificado el conjunto existente.` Códigos de salida: los del original (0, 1, 2 y 130).

## Salida

### Carpeta publicada

Misma forma que el original (KG2): en la carpeta de salida, el puntero a la versión activa y la carpeta de
versión `dataset-<AAAAMMDD>-<hhmmss>-<proceso>[-<n>]`. La carpeta de versión contiene solo:

- `manifest.json`, el manifiesto CanopyDB;
- `canopydb/`, la base CanopyDB con sus archivos internos.

### Manifiesto

Se escribe como el del original, con sangría y un salto de línea final, con estos campos y en este orden (KG3).
Ejemplo ilustrativo y abreviado (`tables` tiene siempre seis entradas); los nombres de `files` dependen de
CanopyDB y de su configuración:

```json
{
  "schema_version": "canopydb-2",
  "generated_at": "2026-10-21T10:15:00+02:00",
  "engine": {
    "name": "canopydb",
    "version": "0.2.5",
    "fork_commit": null
  },
  "source": {
    "file_name": "chicago-crime-data.csv",
    "size_bytes": 2406401982,
    "sha256": "<huella en hexadecimal, minúsculas>"
  },
  "records": {
    "valid": 8656904,
    "rejected": 0
  },
  "compression": "lz4",
  "tables": [
    {
      "name": "primary",
      "entries": 8656904
    },
    {
      "name": "date",
      "entries": 8656904
    }
  ],
  "files": [
    {
      "path": "canopydb/LOCK",
      "size_bytes": 0
    },
    {
      "path": "canopydb/WAL-0000000001",
      "size_bytes": 4096
    },
    {
      "path": "canopydb/crime_1F3A9C0B2D4E6F70_l/DATA",
      "size_bytes": 1534066688
    }
  ]
}
```

- `schema_version` es el texto `canopydb-2`: el motor y la versión del esquema lógico, que es la misma que la
  de las SSTables (K2). Como no es un número, el consultor original rechaza el conjunto en su primera lectura
  del manifiesto, con el mensaje de la tabla (KG3). Tampoco tiene `primary` ni `indexes`.
- `engine.version` es la versión de CanopyDB con que se compiló el generador, tal como la declara la biblioteca
  usada; `engine.fork_commit` es el commit completo del fork si se usa uno (flujo E del padre) y `null` si no.
  En esta entrega siempre es `null`.
- `compression` es `lz4` por defecto y `none` con la opción sin compresión (KG4).
- `tables` tiene siempre las seis tablas, en este orden: `primary`, `date`, `primary_type`, `district`,
  `community_area` y `case_number`.
- `files` lista cada archivo de `canopydb/`, con su ruta relativa a la carpeta de versión, separada con `/` en
  todos los sistemas, en orden de bytes de la ruta, y su tamaño en bytes en el momento de publicar. Después de
  abrir la base los nombres no cambian, pero los tamaños pueden cambiar (paso 5 del flujo).
- Con un CSV que solo tiene la cabecera, `records.valid` y cada `entries` valen 0, y `files` lista igualmente
  los archivos de la base.

### Resumen final

Las mismas líneas y en el mismo orden que el original. Las líneas `Archivo` son una por cada entrada de `files`,
en el mismo orden y con la misma ruta, seguidas de la de `manifest.json`:

```text
Archivo canopydb/LOCK: 0 bytes (0,0 MiB).
Archivo canopydb/WAL-0000000001: 4.096 bytes (0,1 MiB).
Archivo canopydb/crime_1F3A9C0B2D4E6F70_l/DATA: 1.534.066.688 bytes (1.463,0 MiB).
Archivo manifest.json: 1.234 bytes (0,1 MiB).
```

### Informe del verificador

`crime-canopy-acceptance` (KG12) admite las opciones del verificador original (`--csv <archivo>`,
`--dataset <carpeta de salida>` y `--reference`) y el modo de comparación `--compare <carpeta A> <carpeta B>`,
que no se combina con las anteriores. Una combinación inválida termina con 2, como en el original.

Mismo formato y mismo idioma (inglés) que el verificador original: una fila por comprobación con `PASS`,
`FAIL` o `SKIP`, su identificador y un resumen, y la línea final `RESULT:`. Los códigos de salida y el
tratamiento de `SKIP` son los del original.

- **Con el CSV** (`--csv` y `--dataset`): solo admite un conjunto CanopyDB. Las comprobaciones de contenido
  del original conservan sus identificadores (`CSV`, `AC-G1`, `AC-G3`, `AC-G3-SAMPLE`, `AC-G3-DST`,
  `AC-G4-<índice>`, `AC-G4-THEFT`, `AC-G4-COMMUNITY-NIL`, `AC-G4-SHARED-CASES` y `AC-C18-LOWERCASE`). Las de
  forma y manifiesto conservan también el suyo, pero comprueban la forma CanopyDB:
  - `LAYOUT`: la carpeta de versión activa tiene solo `manifest.json` y `canopydb/`;
  - `MANIFEST-SCHEMA`: `schema_version` `canopydb-2`, los campos y su orden, `engine.name` `canopydb`,
    `compression` `lz4` o `none`, y las seis tablas de `tables` con sus nombres y su orden;
  - `MANIFEST-SOURCE`: nombre, tamaño y SHA-256 del CSV, como en el original;
  - `MANIFEST-TABLES`: cada `entries` contra las entradas leídas de su tabla;
  - `MANIFEST-FILES`: las rutas de `files` contra los archivos de `canopydb/`, sin comparar tamaños, que pueden
    cambiar al abrir la base (paso 5 del flujo).

  Si la carpeta no tiene un conjunto CanopyDB válido, `LAYOUT` es la única fila, da `FAIL` y no se ejecuta
  ninguna otra comprobación: con un conjunto SSTable, su resumen es `not a CanopyDB dataset`.
- **En el modo de comparación** (`--compare`): cada carpeta es un conjunto CanopyDB si el manifiesto de su
  versión activa tiene `schema_version` `canopydb-2`, y un conjunto SSTable si tiene `schema_version` 2 y la
  forma del original. Si las dos se abren, da una fila `COMPARE-<tabla>` por cada una de las seis tablas. Si
  coinciden, el resumen da el número de entradas. Si no, da el número de entradas de cada lado y la primera
  diferencia: su posición, contando desde 1, y la clave de cada lado como tupla decodificada, con el formato
  con que el verificador original muestra los valores (`none` si un lado se acabó antes); o la clave y
  `values differ` si las claves coinciden y los valores no.
- Si una de las carpetas no tiene un conjunto válido de ninguno de los dos motores, o su base no se puede abrir
  (por ejemplo, porque otro proceso la tiene abierta), el informe tiene una sola fila `FAIL` con el
  identificador `COMPARE` y el motivo, y termina con 1.
- El verificador CanopyDB no tiene el subcomando `select` del original: el filtrado directo del CSV sigue
  siendo el del verificador original.

### Informe de la verificación de aceptación

`run-canopy-acceptance.ps1` (KG12) admite los parámetros de la verificación del original (`-WorkDir`, `-Csv`,
`-KillTimeoutSeconds`, `-CtrlCAfterSeconds` y `-SkipBuild`) y, además, `-NotMetByCanopyDb`, una lista de
PowerShell de asociaciones `<criterio>=<hallazgo>` (KG10), con el criterio escrito exactamente como la
etiqueta de su fila. Su informe es un archivo Markdown como el del original, en inglés, titulado
`Acceptance of the CanopyDB crime generator`, con estos cambios:

- Las filas son las del original, con sus etiquetas, salvo `AC-G2`, que pasa a `AC-M4`, y `AC-G10`, que pasa a
  `AC-M3`. `AC-M3` compara dos generaciones CanopyDB del mismo CSV, la primera con compresión y la segunda sin
  ella; se añaden `AC-M3 (SSTables)`, la comparación de la primera con el conjunto del generador original, y
  `AC-KG4`.
- Termina con 0 si el resultado global es `PASS` y con 1 si es `FAIL`.
- Solo se puede asociar `AC-M4`. Su fila da `NOT MET`, con la evidencia de siempre seguida de
  `(not met by CanopyDB, <hallazgo>)`, solo cuando la generación terminó con 0, el resumen trae la línea del
  pico y lo único que falla es que el pico, medido desde fuera o en el resumen, supera 512 MiB. Cualquier otro
  fallo de esa fila da `FAIL`, con la asociación o sin ella; si se cumple, da `PASS`, como siempre.
- El resultado global no cuenta las filas `NOT MET` como fallos. Sin ninguna `FAIL`, dice
  `PASS: all <n> checks passed` si no hay `NOT MET`, y `PASS: <p> of <n> checks passed, 1 not met by
  CanopyDB (<hallazgo>)` si `AC-M4` da `NOT MET`; con alguna `FAIL`, `FAIL: <f> of <n> checks failed`, como el original.
- Antes de empezar, la verificación comprueba las asociaciones. Una sin la forma `<criterio>=<hallazgo>`, con
  un criterio distinto de `AC-M4`, con un hallazgo sin su sección en el registro, o una segunda asociación de
  `AC-M4`, la hacen terminar con 1 sin ejecutar nada, con el error
  `Invalid -NotMetByCanopyDb association '<asociación>': <motivo>.`, donde el motivo es
  `expected <criterion>=<finding>`, `only AC-M4 can be associated`, `<hallazgo> is not in the findings
  register` o `AC-M4 is already associated`.

## Documentación

- **Ayuda** del generador CanopyDB y del verificador CanopyDB.
- **README**: un apartado del generador CanopyDB (para qué sirve, cómo se ejecuta y en qué se diferencia del
  original: carpeta publicada, manifiesto y opción sin compresión), del verificador y de su verificación de
  aceptación, con enlace al registro de hallazgos.
- **Guías de Linux**: cómo compilar y ejecutar el generador CanopyDB y comprobar AC-M4 a mano.

## Reutilización

- **Contrato compartido del conjunto**: esquema del registro, codificación de clave y valor, los cinco índices,
  el puntero a la versión activa y la forma de la carpeta de salida.
- **Del generador original**: lectura y validación del CSV, ordenación externa, publicación atómica, bloqueo y
  clasificación de la carpeta de salida, descarte de restos, log, cancelación, medición de memoria, mensajes y
  formato del resumen. Solo cambia dónde se escriben las tablas y el manifiesto. Su código se puede
  reorganizar para compartirlo, sin cambiar su comportamiento (KG11).
- **Verificador original**: sus comprobaciones de contenido contra el CSV, su formato de informe y su lector
  independiente de tablas LevelDB, que el modo de comparación usa para leer un conjunto SSTable.
- **Verificación de aceptación del original**: sus casos, con los cambios de los criterios de este documento.

## Fuera de alcance

- El consultor CanopyDB, el lote de consultas, el banco de comparación y la búsqueda de bugs: son los hijos 2 a
  5 del padre.
- Cambiar el comportamiento del generador, el consultor o el verificador originales.
- Del flujo E, todo salvo el alta en el registro: preparar o publicar issues y PR, comentar, crear el fork o
  depender de él.
- Leer o convertir conjuntos entre motores, salvo la comparación de contenido del verificador.
- Garantizar que una base sea legible con otra versión de CanopyDB, y el rechazo de esos conjuntos: lo fija
  `canopy-db-consultor`.
- Comprobar qué pasa al borrar una versión que un consultor CanopyDB tiene abierta: no hay consultor CanopyDB
  hasta `canopy-db-consultor`, y los escenarios de concurrencia son de `canopy-db-robustez`.
- Que el generador original descarte las carpetas de ordenación del generador CanopyDB, o al revés: cada uno
  descarta solo las suyas.
- Un objetivo de tiempo de generación y ajustar CanopyDB para que genere más rápido: la comparación de
  rendimiento es de `canopy-db-comparacion`.
- Ejecutar la verificación de aceptación en Linux: AC-M4 se comprueba allí a mano.
- Durabilidad frente a cortes de alimentación, como en el padre.
- Comprimir los valores en la aplicación antes de guardarlos (KG4).

## Queda para la implementación

- La organización interna de la base (una base de datos interna con seis árboles o varias) y la configuración
  de CanopyDB, incluido cómo se limita su memoria, siempre que se cumplan el contrato y los límites. Determina
  los nombres de `files`; el README y el informe de cierre muestran los que resulten.
- La versión de CanopyDB: la última publicada al empezar, hoy la 0.2.5.
- El espacio libre que exige la verificación de aceptación, calculado a partir de una generación medida.
- El lenguaje de los programas de reproducción de H2 y H3 y dónde viven en el repositorio, siempre que se
  ejecuten con una orden y sin el resto del proyecto.
- Cómo provocan las pruebas los errores del motor de AC-KG7, siempre que no añada ninguna opción visible ni
  cambie el comportamiento de una ejecución normal.

## Criterios de aceptación

Las cifras corresponden al CSV de referencia, con la misma salvedad que en el padre. Un criterio heredado que
CanopyDB no puede cumplir se trata según K10 (KG10).

1. **AC-KG1** (AC-M1, parte del generador) — Dado el CSV de referencia, cuando se ejecutan el generador CanopyDB
   y su verificación de aceptación, entonces se cumplen AC-G1 y AC-G3 a AC-G9 con las mismas cifras,
   comprobados con el verificador CanopyDB; AC-M3 sustituye a AC-G10, y AC-M4 a AC-G2 y AC-L8. Con el extracto
   de referencia se cumple AC-A1; con una descarga posterior del portal, hecha durante la verificación como
   fija la decisión A5, AC-A2 y AC-A3.
2. **AC-M3** (decisión K2, KG8) — Dado el mismo CSV, cuando se genera dos veces con el generador CanopyDB y una
   con el original, entonces el modo de comparación del verificador confirma que los dos conjuntos CanopyDB
   tienen exactamente las mismas claves y valores en cada tabla, y que son los mismos que los de las SSTables;
   lo mismo entre un conjunto CanopyDB generado con compresión y otro sin ella.
3. **AC-M4** (decisión K2, KG10) — Dado el CSV de referencia completo, cuando se ejecuta el generador CanopyDB,
   entonces el pico de memoria medido no supera 512 MiB y el resumen final lo muestra. En Windows lo comprueba
   la verificación de aceptación, desde fuera y en el resumen; en Linux se comprueba a mano y se anota en el
   informe de cierre. Si CanopyDB lo impide, se trata según KG10.
4. **AC-KG2** (KG2, KG3) — Dado un conjunto CanopyDB recién publicado y sin abrir, cuando se mira la carpeta de
   salida, entonces tiene el puntero y una carpeta de versión con solo `manifest.json` y `canopydb/`; el
   manifiesto tiene los campos y el orden de [Manifiesto](#manifiesto); `files` coincide con los archivos de
   `canopydb/` y sus tamaños; y el resumen final lista esos mismos archivos en el mismo orden, seguidos de
   `manifest.json`. Después, el verificador confirma que `tables` coincide con las entradas que lee. Con un CSV
   que solo tiene la cabecera, se muestra el aviso del original, cada `entries` vale 0 y `files` lista los
   archivos de la base.
5. **AC-KG3** (KG3) — Dado un conjunto CanopyDB publicado, cuando se le hace una consulta con el consultor
   original, entonces se rechaza con `No se encontró un conjunto de datos válido en <carpeta>.` y el código 1.
6. **AC-KG4** (K6, paso 5 del flujo) — Dado un conjunto CanopyDB recién publicado y una copia de su carpeta de
   salida entera hecha antes de abrirlo, cuando el verificador lee la base entera dos veces, entonces la
   carpeta de versión tiene los mismos nombres de archivo que la de la copia, sin ninguno nuevo, el manifiesto
   es idéntico y el modo de comparación entre la carpeta de salida y la copia da las seis tablas iguales.
7. **AC-KG5** (KG5) — Dada una carpeta de salida con un conjunto SSTable, cuando se ejecuta el generador CanopyDB
   sin sobrescritura, entonces se rechaza con el mensaje de conjunto existente y la carpeta no cambia; con
   sobrescritura, publica el conjunto CanopyDB y la carpeta de versión SSTable desaparece. Dada una carpeta con
   un conjunto CanopyDB, el generador original se comporta igual en los dos casos.
8. **AC-KG6** (KG6) — Dada una carpeta de salida con un conjunto publicado y una carpeta de versión huérfana de
   cada motor, cuando se ejecuta el generador CanopyDB, entonces descarta las dos con un solo mensaje de restos
   y el resultado es el de una generación normal. Dadas en la carpeta temporal unas carpetas de ordenación de un
   generador CanopyDB que ya no está en marcha, otras de uno que sigue en marcha y otras del generador original
   que ya no está en marcha, cuando arranca el generador CanopyDB, entonces descarta solo las primeras, con su
   mensaje, en Windows y en Linux.
9. **AC-KG7** (KG7) — Dado un error del motor provocado en una prueba (falta de espacio, un fallo de escritura,
   un fallo interno, un pánico del motor y un error en sus archivos temporales), cuando aborta la generación,
   entonces el mensaje es el de la tabla, el log contiene una sola línea `Detalle del error de CanopyDB:`
   seguida del texto del error, el código de salida es 1 y el conjunto anterior sigue intacto y el verificador
   lo lee sin fallos.
10. **AC-KG8** (KG4) — Dado un CSV, cuando se genera con y sin la opción sin compresión, entonces el manifiesto
    dice `lz4` y `none` respectivamente, y la ayuda describe la opción con el texto de KG4.
11. **AC-KG9** (KG8) — Dados dos conjuntos con una diferencia conocida en una tabla (una entrada de más, una
    clave distinta y un valor distinto), cuando se comparan con el verificador, entonces esa tabla da `FAIL`
    con la diferencia descrita en [Informe del verificador](#informe-del-verificador), las demás dan `PASS` y
    el código de salida es 1; con dos conjuntos iguales, todas dan `PASS` y el código es 0; con una base que
    otro proceso tiene abierta o una carpeta sin conjunto, da una sola fila `FAIL` con el motivo y el código 1;
    con un conjunto SSTable en `--dataset`, `LAYOUT` da `FAIL` con `not a CanopyDB dataset`; y con una línea de
    comandos inválida, el código es 2.
12. **AC-KG10** (KG9) — Dado el registro de hallazgos al cerrar, cuando se revisa, entonces está en el lugar y con
    la forma de KG9, con todos los campos del padre para H1–H4 y para cualquier hallazgo nuevo; las citas de H1
    y H4 corresponden a la documentación de CanopyDB 0.2.5; y al ejecutar las reproducciones de H2 y H3 con la
    versión anotada, muestran lo que describe el paso 2 de [Registro de hallazgos](#registro-de-hallazgos).
13. **AC-KG11** (KG11) — Dado el repositorio en Windows y en Linux (Ubuntu 24.04 sobre WSL2), cuando se pasan
    las pruebas, entonces pasan todas, incluidas las del generador y el verificador CanopyDB. Las pruebas de las
    aplicaciones originales que existían antes del cambio siguen existiendo y pasando, ninguna desaparece y sus
    comprobaciones no cambian; solo pueden cambiar las rutas o los nombres con que llegan al código que
    prueban. En Linux, las pruebas del generador CanopyDB cubren AC-L3 a AC-L6 y AC-L9. Y en Windows, la
    verificación de aceptación del generador original sigue pasando.
14. **AC-KG12** — Dados la ayuda y el README, cuando se leen, entonces la ayuda del generador CanopyDB es la del
    original con los tres cambios del paso 1 del flujo, y el README tiene el apartado de
    [Documentación](#documentación) con el enlace al registro de hallazgos.
15. **AC-KG13** (KG10) — Dada la verificación de aceptación con `-NotMetByCanopyDb AC-M4=<hallazgo>` y una
    generación que termina con 0 con un pico de más de 512 MiB, cuando termina, entonces la fila `AC-M4` dice
    `NOT MET` con su hallazgo y el resultado global es `PASS` si no hay otra fila `FAIL`; con la misma
    asociación y una generación que falla de otro modo (sin la línea del pico o con un código distinto de 0),
    la fila dice `FAIL`; y dada una asociación con un criterio distinto de `AC-M4` o un hallazgo sin su sección
    en el registro, cuando arranca, entonces termina con 1 sin ejecutar nada, con el error de
    [Informe de la verificación de aceptación](#informe-de-la-verificación-de-aceptación).
16. **AC-KG14** (paso 2 del flujo) — Dada una generación en curso del generador original sobre una carpeta de
    salida, cuando arranca el generador CanopyDB sobre la misma carpeta, entonces aborta con `Otra generación
    está usando la carpeta de salida <carpeta>.` sin tocar nada, y la primera termina con normalidad; y lo mismo
    al revés, en Windows y en Linux.

## Decisiones

Todas se tomaron con el usuario el 2026-10-21.

- **KG1 — Nombre**: `crime-canopy-generator`; sus carpetas de ordenación se llaman como las del original con
  ese nombre. El consultor CanopyDB fija su propio nombre en su PRD.
- **KG2 — Forma de la carpeta**: la misma que el original (puntero y carpetas de versión con el mismo patrón de
  nombre); dentro de la carpeta de versión, el manifiesto y la base CanopyDB en su propia subcarpeta
  `canopydb/`. Motivo: reutiliza tal cual la publicación atómica, el bloqueo y la clasificación de la carpeta de
  salida, y el consultor original llega al manifiesto, donde se fija el rechazo (KG3). La subcarpeta es
  necesaria porque CanopyDB trata cada subcarpeta de la base como una base de datos interna, y separa los
  archivos del motor del manifiesto.
- **KG3 — Manifiesto**: `schema_version` es el texto `canopydb-2`, con `engine`, `tables` y `files` en lugar de
  `primary` e `indexes`. El consultor original lo rechaza en su primera lectura del manifiesto con `No se
  encontró un conjunto de datos válido en <carpeta>.`, el mismo mensaje con que el consultor CanopyDB rechazará
  un conjunto SSTable (padre, Validaciones y mensajes). Motivo: ninguna herramienta que lea solo
  `schema_version` puede confundir un conjunto CanopyDB con uno SSTable, y el texto conserva el número del
  esquema lógico. Se descartaron `schema_version` 2 sin `primary` ni `indexes` (una herramienta que leyera solo
  la versión lo tomaría por un conjunto SSTable) y un número distinto, que daría `El conjunto usa el esquema
  v<N> y este consultor admite v2. Regenera el conjunto.`, un mensaje engañoso: el analista no necesita
  regenerar, sino usar el consultor CanopyDB.
- **KG4 — Opción sin compresión**: se admite con el mismo nombre; desactiva la compresión LZ4 que CanopyDB
  aplica a los valores grandes, activa por defecto. Ayuda: `Escribe la base sin compresión. Por defecto,
  CanopyDB comprime con LZ4 los valores grandes.` `compression` describe la configuración, no que se haya
  comprimido algún valor. Consecuencia aceptada: con este conjunto ningún valor es lo bastante grande, así que
  la base ocupa lo mismo con y sin la opción; el README lo explica. Se descartó comprimir los valores con
  Snappy en la aplicación, porque cambiaría los valores guardados y rompería K2.
- **KG5 — Conjunto del otro motor**: el generador CanopyDB trata un conjunto SSTable como un conjunto existente,
  igual que el original trata ya un conjunto CanopyDB con la forma de KG2. Motivo: un mismo contrato y ningún
  cambio en el original.
- **KG6 — Restos**: como en el original. Las carpetas de versión huérfanas se descartan sea cual sea su motor;
  en la carpeta temporal, cada generador descarta solo sus propias carpetas de ordenación. Los archivos
  temporales que use CanopyDB van dentro de la carpeta de ordenación de la ejecución y se descartan con ella.
- **KG7 — Errores del motor** (K9): un error de CanopyDB, incluido un fallo interno o un pánico, usa los
  mensajes de escritura del original: los de la carpeta temporal si viene de los archivos temporales del motor,
  que se configuran dentro de la carpeta de ordenación de la ejecución, y los de la carpeta de salida en
  cualquier otro caso. El detalle va en una línea propia del log. Motivo: el
  original no muestra el detalle técnico y K9 pide conservarlo para investigar hallazgos.
- **KG8 — Mismo contenido lógico** (K2): el verificador CanopyDB compara entrada por entrada dos conjuntos de
  cualquier motor. Motivo: una huella de los archivos no sirve entre motores, y la comparación directa dice
  dónde está la primera diferencia.
- **KG9 — Registro de hallazgos**: `docs/canopy-db/hallazgos.md`, en español, común a todo el módulo. Empieza
  con una tabla resumen (identificador, tipo, estado y título) y sigue con una sección por hallazgo, con todos
  los campos del padre en forma de lista. Motivo: las reproducciones, las citas y las aprobaciones no caben
  bien en una celda; la carpeta del módulo puede alojar también las reproducciones.
- **KG10 — Criterios que CanopyDB no puede cumplir** (K2 y K10): el generador configura CanopyDB para cumplir
  512 MiB. Si CanopyDB lo impide, la generación termina igual, con el aviso del original. La verificación de
  aceptación tiene un estado propio para K10, solo para AC-M4: cuando el desarrollador la ejecuta asociando
  AC-M4 con su hallazgo del registro (`-NotMetByCanopyDb AC-M4=H5`, por ejemplo) y lo único que falla es que
  el pico supera 512 MiB, la fila dice `NOT MET` en lugar de `FAIL` y no cuenta como fallo en el resultado
  global (ver [Salida](#salida)). El log de la ejecución y el informe de cierre lo anotan como "no cumplido por
  CanopyDB". Motivo: el informe de aceptación separa lo que falla en el proyecto de lo que impide CanopyDB, la
  asociación con un hallazgo existente obliga a justificar la excepción con su reproducción, y limitarla al
  pico impide que esconda un fallo del proyecto, que K10 obliga a corregir. Se descartaron marcarlo `FAIL` y
  clasificarlo solo en el informe de cierre, y admitir cualquier criterio o cualquier fallo.
- **KG11 — Aplicaciones originales**: se puede extraer o reorganizar su código para compartirlo con las
  aplicaciones CanopyDB, sin cambiar su comportamiento, sus mensajes ni sus salidas; sus pruebas siguen pasando
  sin cambiar lo que comprueban, y su verificación de aceptación sigue pasando. Motivo: casi todo el generador
  original sirve tal cual, y duplicarlo costaría más que el riesgo, que cubren sus pruebas y su aceptación.
  Se descartó no tocar ni una línea de las originales.
- **KG12 — Herramientas de verificación**: el verificador CanopyDB se llama `crime-canopy-acceptance` y la
  verificación de aceptación `run-canopy-acceptance.ps1`, con el patrón de nombres de KG1 (ver
  [Salida](#salida)).

## Anotaciones al cerrar

Al cerrar este hijo se anota el [PRD del generador](generador.prd.md) y su traducción, sin reescribir su
historia, como pide [Criterios y anotaciones por hijo](canopy-db.prd.md#criterios-y-anotaciones-por-hijo):

- En G5: *Anotación del <fecha>: el generador CanopyDB no produce archivos idénticos byte a byte; dos
  generaciones del mismo CSV guardan las mismas claves y valores (decisión K2 de [canopy-db.prd.md](canopy-db.prd.md),
  criterio AC-M3 de [canopy-db-generador.prd.md](canopy-db-generador.prd.md)). El generador original conserva G5.*
- En AC-G2: *Anotación del <fecha>: el generador CanopyDB debe cumplir los mismos 512 MiB (K2); si CanopyDB lo
  impide, se aplica K10 (criterio AC-M4 de [canopy-db-generador.prd.md](canopy-db-generador.prd.md)).*

## Referencias

- Documento padre: [canopy-db.prd.md](canopy-db.prd.md)
- Documento padre general: [generador.prd.md](generador.prd.md), cuyo contrato hereda la aplicación nueva (K1)
- Documento raíz: [chicago-crime.prd.md](chicago-crime.prd.md)
- [PRD de soporte para Linux](soporte-linux.prd.md) y [PRD de aceptación con cualquier
  CSV](aceptacion-cualquier-csv.prd.md)
- CanopyDB: https://github.com/arthurprs/canopydb y https://docs.rs/canopydb

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 982e14bf | 2026-10-21 07:29 UTC+01:00 | 43 min | 0 min | 43 min | 31 min | 12 min | 9 |

<!-- prd-time: [{"session": "982e14bf-027d-4852-9540-64cd2aeea746", "start": "2026-10-21T06:29:14.532000+00:00", "wall": 2607, "discarded": 0, "agent": 1869, "human": 738, "interventions": 9, "idle_minutes": 10}] -->
