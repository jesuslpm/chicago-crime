# Consultor CanopyDB — PRD

**Versión**: 1.0
**Fecha**: 2026-10-22
**Documento padre**: [Almacén de delitos de Chicago sobre CanopyDB — PRD](canopy-db.prd.md) (v1.0)

## Objetivo y contexto

Entregar el tercer hijo del módulo CanopyDB (orden 3 de [Reparto en PRD hijos](canopy-db.prd.md#reparto-en-prd-hijos)):

- el **consultor CanopyDB**, una aplicación nueva que hace lo mismo que el consultor original, lote de consultas
  incluido, pero lee el conjunto que publica el generador CanopyDB;
- el **conjunto fijo de consultas** del módulo, que reutilizará el banco de comparación (K3);
- su **verificación de aceptación**, que comprueba también los criterios del módulo que el padre asigna a este
  hijo con los dos consultores (AC-M2 y AC-M8).

El consultor CanopyDB hereda el contrato del [PRD del consultor](consultor.prd.md), del
[PRD de "empieza por"](startswith.prd.md) y del [PRD del lote de consultas](canopy-db-lote.prd.md) (decisión K1
del padre): las mismas opciones, validaciones, mensajes, salidas y códigos de salida, y las reglas del documento
raíz, del [PRD de soporte para Linux](soporte-linux.prd.md) y del
[PRD de aceptación con cualquier CSV](aceptacion-cualquier-csv.prd.md). Este documento no las repite. Solo fija
lo que el padre le asigna (columna "Fija" del reparto): el nombre, el conjunto fijo de consultas, la espera ante
un conjunto abierto por otro proceso (K6), los mensajes de los errores del motor (K9), el rechazo de un conjunto
del otro motor y de uno escrito con otra versión de CanopyDB, y cómo se limita la memoria (K8).

Además, **amplía la lista de excepciones de K1** con dos cambios de la salida pública que solo valen para el
consultor CanopyDB: la ayuda (KC1) y el `schema_version` del JSON (KC6). El consultor original no cambia. Las
anotaciones que deja están en [Anotaciones al cerrar](#anotaciones-al-cerrar).

**Por qué ahora**: el banco de comparación (`canopy-db-comparacion`) necesita los dos consultores y el conjunto
fijo de consultas, y la búsqueda de bugs (`canopy-db-robustez`) necesita un consultor CanopyDB que bloquear,
cancelar y matar.

### Estado de partida

Verificado el 2026-10-22 sobre el repositorio y el código fuente de CanopyDB 0.2.5:

| Aspecto | Situación |
|---|---|
| Generador CanopyDB | Implementado y aceptado (`canopy-db-generador`, 17 de 17 comprobaciones). Publica en la carpeta de versión `manifest.json` y `canopydb/`. AC-M4 se cumple: pico de 219,5 MiB en Windows y 212,2 MiB en Linux, con la memoria de CanopyDB limitada muy por debajo de su valor por defecto de 1 GiB. |
| Manifiesto CanopyDB | `schema_version` es el texto `canopydb-2`; `engine` anota `canopydb`, la versión `0.2.5` y `fork_commit` `null`; `files` lista los archivos de `canopydb/`. No tiene `primary` ni `indexes`. |
| Consultor original | Implementado y aceptado (consultor, "empieza por" y lote). Opciones: `--dataset`, `--where`, `--startswith`, `--range`, `--limit`, `--count`, `--format`, `--output`, `--batch` y `-h`/`--help`. Ante un conjunto CanopyDB responde `No se encontró un conjunto de datos válido en <carpeta>.` con el código 1 (AC-KG3). |
| Errores de apertura del original | Sin puntero, versión que falta, manifiesto ilegible o sin la forma esperada, o una tabla que falta: `No se encontró un conjunto de datos válido en <carpeta>.` Un archivo que existe pero no se puede leer: `No se pudo leer el conjunto de datos en <carpeta>.` El detalle técnico no se muestra. |
| Tiempos del original | El tiempo de la línea de estadísticas de una consulta empieza con el conjunto ya abierto. El de la línea final del lote cubre la ejecución entera, apertura incluida. |
| Salida JSON del original | El miembro `dataset` tiene `version` (nombre de la carpeta de versión), `generated_at` y `schema_version`, este como número (`2`). |
| Ayuda del original | Termina con los códigos de salida y un párrafo sobre los códigos del lote. |
| Consultor CanopyDB | No existe. Ningún documento le da nombre. |
| Apertura en CanopyDB 0.2.5 | Solo un proceso a la vez puede tener abierta una base. El tiempo que espera el segundo es configurable (5 s por defecto); al agotarse, CanopyDB da un error. No hay apertura de solo lectura (H2 y H3). |
| Versión del formato en CanopyDB | Los archivos de la base no indican con qué versión de CanopyDB se escribieron. La única constancia es `engine` en el manifiesto. |
| Lectura sin cambios | El verificador CanopyDB ya lee una base sin añadir archivos ni cambiar el manifiesto o el contenido lógico (AC-KG4). |
| Registro de hallazgos | `docs/canopy-db/hallazgos.md`, con H1–H5 en estado "Detectado". H5 (rendimiento): CanopyDB no tiene carga masiva de entradas ordenadas. |
| Verificación de aceptación del consultor original | Script de Windows que comprueba AC-C1 a AC-C18, AC-S1 a AC-S12 y los criterios del lote contra el filtrado directo del CSV, y mide el pico de memoria desde fuera. Cifras anotadas: contar `THEFT`, 14,5 MiB; contar un tipo raro, 7,5 MiB; archivo CSV con todos los robos, 16,6 MiB. **Nunca se ha medido el pico de un lote.** |
| Conjunto fijo de consultas | Solo existe el esbozo de K3. No hay una lista concreta. |

**Huecos de producto**: ninguno abierto. Cada punto de la columna "Fija" del padre queda como regla en este
documento o en [Queda a criterio de la implementación](#queda-a-criterio-de-la-implementación).

## Actores

- **Analista**: el del consultor. Usa el consultor CanopyDB igual que el original.
- **Desarrollador**: ejecuta la verificación de aceptación y da de alta los hallazgos que aparezcan.

## Flujo

### Consulta

Sigue el flujo B del padre y, por K1, los flujos del consultor, de "empieza por" y del lote. Esta entrega
concreta:

1. El analista ejecuta `crime-canopy-query` con las mismas opciones que el original, sueltas o con `--batch`.
   La ayuda es la del original con tres cambios (KC1): el nombre de la aplicación; la descripción, que dice
   `Consulta el conjunto de datos de delitos de Chicago (base CanopyDB) publicado por el generador CanopyDB.`;
   y un párrafo más al final de la ayuda, después del de los códigos del lote:
   `Si otro proceso tiene abierto el conjunto de datos, la consulta espera hasta 30 s antes de rechazarse con 1.`
2. Comprueba la línea de comandos, las condiciones y, con `--batch`, el archivo de lote, igual que el original y
   en el mismo orden, sin abrir el conjunto.
3. Localiza la versión activa por el puntero, como el original, y comprueba, en este orden y sin abrir la base:
   el `schema_version` del manifiesto (KC3), la forma del manifiesto (KC3), la versión de CanopyDB (KC4) y que
   existan todos los archivos que lista `files`. El primer fallo rechaza con su mensaje. *Anotación del
   2026-10-22: si la versión que iba a abrir deja de ser la activa (el generador publicó otra y la está
   borrando), vuelve a leer el puntero y abre la nueva, sin un segundo aviso ni más espera que la de KC2; una
   versión que ya abrió responde hasta el final aunque se borre (decisión KR7 del
   [PRD de robustez](canopy-db-robustez.prd.md)).*
4. Abre la base. Si otro proceso cualquiera la tiene abierta (otro consultor CanopyDB, el verificador CanopyDB u
   otro programa), avisa en cuanto lo detecta y espera hasta 30 s (KC2): si se libera, sigue con normalidad; si
   no, rechaza sin resultados con el código 1.
5. Comprueba que la base tenga las seis tablas del manifiesto; nunca crea una base, una tabla ni un archivo.
6. Ejecuta la consulta, o las del lote una tras otra, con las mismas reglas, salida y estadísticas que el
   original. Como en el original, el tiempo de la línea de estadísticas de cada consulta no incluye la apertura
   (ni, por tanto, la espera), y el de la línea final del lote sí. Un lote mantiene la base abierta hasta que
   termina.
7. Ctrl+C en cualquier momento, también durante la espera o la apertura de la base, termina como cualquier
   cancelación del original: `Consulta cancelada.` y el código 130.
8. Ante cualquier otro error del motor, muestra el mensaje existente más cercano y una línea con el detalle en
   la salida de errores (KC5).
9. No cambia el contenido consultable ni el manifiesto, y no crea archivos en la carpeta del conjunto; sí puede
   cambiar el contenido o el tamaño de los archivos internos del motor (K6).
10. Su pico de memoria no crece con el número de resultados ni con el número de consultas del lote (K8).

### Verificación de aceptación

1. El desarrollador ejecuta `run-canopy-query-acceptance.ps1` (KC8) sobre un CSV y una carpeta de trabajo.
2. La verificación genera el conjunto con los dos generadores y ejecuta con el consultor CanopyDB las
   comprobaciones de la verificación del consultor original, con las sustituciones de AC-M1, más las de este
   documento. Donde la del original usa el verificador independiente, esta usa el verificador CanopyDB.
3. Ejecuta el conjunto fijo de consultas (KC7) con los dos consultores, sueltas y en lote, compara sus
   resultados (AC-M2) y mide el pico de memoria de cada consultor (AC-M8).
4. Escribe un informe con el formato del original y termina con 0 o 1.

## Conjunto fijo de consultas

Decisión KC7. Once consultas, escritas como líneas de un archivo de lote del
[PRD del lote de consultas](canopy-db-lote.prd.md), en este orden:

| Id | Qué mide | Línea |
|---|---|---|
| F1 | Por ID | `--where ID=634 --format json` |
| F2 | Por número de caso compartido | `--where "Case Number=<número>" --format json` |
| F3 | Conteo de un tipo principal grande | `--where "Primary Type=THEFT" --count` |
| F4 | Conteo de un tipo principal raro | `--where "Primary Type=RITUALISM" --count` |
| F5 | Rango de fechas de un mes | `--range Date=2019-08-01..2019-08-31 --count` |
| F6 | Rango de once años con un tipo principal | `--where "Primary Type=THEFT" --range Date=2005-01-01..2015-12-31 --count` |
| F7 | Dos condiciones indexadas | `--where "Primary Type=THEFT" --where District=008 --count` |
| F8 | Dos indexadas y un filtro sin índice | `--where "Primary Type=THEFT" --range Date=2019-08-01..2019-08-31 --where Arrest=true --count` |
| F9 | Prefijo | `--startswith "Case Number=JA" --count` |
| F10 | Listado con límite | `--where "Primary Type=THEFT" --limit 1000 --format csv` |
| F11 | Listado completo | `--where "Primary Type=THEFT" --format csv` |

`<número>` es el primer número de caso compartido del CSV, el mismo que usa la verificación del consultor
original para AC-C6. Los demás valores son los que ya usa esa verificación (el ID 634 es el primero del extracto
de referencia; agosto de 2019, `RITUALISM`, `008` y `Arrest=true`, sus consultas de AC-C4, AC-C5, AC-C9 y
AC-C13) y, como dice el estado de partida del PRD de aceptación con cualquier CSV, una descarga posterior los
conserva casi con seguridad.

## Validaciones y mensajes

Se aplican todas las validaciones y mensajes del documento raíz, del consultor, de "empieza por", del lote y del
PRD de soporte para Linux, sin cambiar una palabra. Esta entrega solo fija los casos propios de CanopyDB, en el
orden en que se comprueban (paso 3 en adelante del flujo), después de la línea de comandos, las condiciones y el
archivo de lote:

| Situación | Regla | Mensaje al usuario | Código |
|---|---|---|---|
| El manifiesto existe pero no se puede leer | Rechaza sin abrir la base, como el original, sin línea de detalle | `No se pudo leer el conjunto de datos en <carpeta>.` | 1 |
| Sin puntero, versión que falta, manifiesto que no es JSON válido, o un `schema_version` que no es un texto de la forma `canopydb-` seguido solo de cifras; incluye cualquier conjunto SSTable, de cualquier esquema (KC3) | Rechaza sin abrir la base | `No se encontró un conjunto de datos válido en <carpeta>.` | 1 |
| `schema_version` `canopydb-<cifras>` distinto de `canopydb-2` (KC3) | Rechaza sin abrir la base | `El conjunto usa el esquema v<cifras> y este consultor admite v2. Regenera el conjunto.` | 1 |
| Manifiesto `canopydb-2` sin la forma del generador CanopyDB (KC3) | Rechaza sin abrir la base | `No se encontró un conjunto de datos válido en <carpeta>.` | 1 |
| `engine.version` o `engine.fork_commit` distintos de los del consultor (KC4) | Rechaza sin abrir la base | `El conjunto se escribió con CanopyDB <versión del conjunto> y este consultor usa CanopyDB <versión propia>. Regenera el conjunto.` | 1 |
| Falta un archivo que lista `files` (o es una carpeta), o falta `canopydb/` | Rechaza sin abrir la base ni crear nada | `No se encontró un conjunto de datos válido en <carpeta>.` | 1 |
| Otro proceso tiene abierta la base (KC2) | Avisa una vez en la salida de errores, en cuanto el primer intento la encuentra abierta, aunque se libere enseguida, y espera; si se libera antes de 30 s, sigue sin más mensajes | `Aviso: otro proceso tiene abierto el conjunto de datos en <carpeta>; se esperará hasta 30 s.` | — |
| La base sigue abierta por otro proceso 30 s después del aviso (KC2) | Rechaza sin resultados, entre 30 y 35 s después del aviso | `El conjunto de datos en <carpeta> sigue abierto por otro proceso. Inténtalo de nuevo cuando termine.` | 1 |
| Ctrl+C durante la espera o la apertura | Termina como cualquier cancelación, en menos de 1 s | `Consulta cancelada.` | 130 |
| La base abierta no tiene alguna de las seis tablas | Rechaza; nunca crea una tabla | `No se encontró un conjunto de datos válido en <carpeta>.` | 1 |
| Cualquier otro error del motor al abrir o leer la base, incluidos un fallo inesperado del motor y que CanopyDB no pueda escribir en sus archivos internos (KC5) | Rechaza o aborta como el original ante un conjunto que deja de poderse leer: sin salida parcial engañosa ni archivo de salida a medias, con el código 1 y nunca con un código de terminación anormal | `No se pudo leer el conjunto de datos en <carpeta>.` | 1 |
| Detalle de ese error del motor (KC5) | Una sola línea en la salida de errores, justo antes del mensaje; cada retorno de carro y cada salto de línea del texto del motor se sustituye por un espacio, y el resto queda tal cual, aunque esté vacío. Solo acompaña a ese `No se pudo leer…` | `Detalle del error de CanopyDB: <texto del error tal como lo da CanopyDB>` | — |

Precisiones:

- *Anotación del 2026-10-22, al implementar: la línea de detalle sigue la regla de KG7, como dice KC5, y no la
  de la fila: cada grupo seguido de retornos de carro y saltos de línea del texto del motor se sustituye por un
  solo espacio, y los del principio y del final se quitan: dos líneas del motor separadas por un retorno de
  carro y un salto de línea quedan separadas por un solo espacio.*

- **Forma del manifiesto** (KC3): como en el original, todos los campos del manifiesto CanopyDB del
  [PRD del generador CanopyDB](canopy-db-generador.prd.md#manifiesto) son obligatorios y con su tipo, y los
  desconocidos se toleran. En particular, `engine.name` es `canopydb`; `engine.version` es un texto no vacío;
  `engine.fork_commit` existe y es `null` o un texto no vacío; y `tables` tiene exactamente las seis tablas, con
  sus nombres y en su orden; cada entrada de `files` es una ruta relativa a la carpeta de versión que empieza
  por `canopydb/`, separada con `/`, sin partes `.` ni `..`. `canopydb/` puede tener archivos que `files` no
  lista: se toleran.
- **`<cifras>`** se muestra tal como está escrito: `canopydb-02` da `v02`. La comparación con `canopydb-2` es
  exacta, con las mayúsculas y minúsculas tal cual, así que `CANOPYDB-2` y `canopydb-` son conjuntos inválidos.
  El mensaje es el de esquema incompatible del documento raíz, con la versión que el consultor admite hoy (v2,
  decisión C1 del consultor).
- **Versiones**: `<versión del conjunto>` y `<versión propia>` son `engine.version` tal como está escrita,
  seguida de ` (fork <commit>)` con el commit completo cuando `engine.fork_commit` no es `null`. La comparación
  es exacta.
- **`<carpeta>`** es la de `--dataset`, como en los mensajes del original.
- **Archivo de salida**: todos los rechazos de la tabla, salvo un error del motor a mitad de una consulta, ocurren
  antes de preparar el archivo de `--output`, así que un archivo que ya existía queda como estaba, como en el
  original.
- **Con `--batch`**, todos estos casos ocurren al abrir el conjunto, antes de la cabecera de la primera consulta,
  así que salen sin el prefijo `Línea <n>: ` y sin cabecera (paso 5 del orden de las comprobaciones del lote), y
  Ctrl+C durante la espera solo deja `Consulta cancelada.`. Un error del motor durante una consulta sale debajo
  de su cabecera, como cualquier fallo de una consulta del lote.

## Salida

- **Consola, salida de errores, CSV, archivo de salida y lote**: los del original, sin cambios, con los mensajes
  de [Validaciones y mensajes](#validaciones-y-mensajes).
- **JSON**: el del original, con una sola diferencia (KC6): en el miembro `dataset`, `schema_version` es el
  texto del manifiesto, `"canopydb-2"`, en lugar de un número. `version` y `generated_at` no cambian. Ejemplo
  abreviado:

  ```json
  {"query":{…},"dataset":{"version":"dataset-20261022-101500-4242","generated_at":"2026-10-22T10:15:00+02:00","schema_version":"canopydb-2"},"results":[…]}
  ```

### Informe de la verificación de aceptación

`run-canopy-query-acceptance.ps1` (KC8) admite los parámetros de la verificación del consultor original y,
además, `-NotMetByCanopyDb`, con la forma, las comprobaciones previas y el tratamiento de `NOT MET` de la
verificación del generador CanopyDB (KG10), con estas diferencias:

- se pueden asociar `AC-C7`, `AC-S8` y `AC-M8`, cada uno una sola vez; asociar otro criterio da el motivo
  `only AC-C7, AC-S8 and AC-M8 can be associated`, y repetir uno, `<criterio> is already associated`;
- una fila asociada da `NOT MET` solo cuando todas sus ejecuciones terminaron con 0 y con el resultado esperado,
  y lo único que falla es la cifra que mide: el tiempo de AC-C7 o el cociente de memoria de AC-S8 y de AC-M8,
  todos del consultor CanopyDB; cualquier otro fallo da `FAIL`. La fila
  `AC-M8 (SSTables)` no se puede asociar: si falla, da `FAIL` (K8: es un fallo del proyecto).

Su informe es un archivo Markdown como el del original, en inglés, titulado
`Acceptance of the CanopyDB crime query app`, y termina con 0 si el resultado global es `PASS` y con 1 si es
`FAIL`. Sus filas:

- las del original, con sus etiquetas, salvo `AC-C13`, que pasa a `AC-M8`, y la segunda mitad de `AC-C18`
  (conjunto con esquema 1), que pasa a comprobar el rechazo de un conjunto SSTable de esquema 2;
- `AC-M2-F1` a `AC-M2-F11`, una por consulta del conjunto fijo, con el número de resultados de cada consultor;
- `AC-M5`, `AC-M6` y `AC-M7`, este con el consultor CanopyDB (el original lo cubre el lote);
- `AC-M8` para el consultor CanopyDB y `AC-M8 (SSTables)` para el original, cada una con sus tres picos y su
  cociente;
- `AC-KC2` a `AC-KC7`.

El resultado global sigue la regla de la verificación del generador CanopyDB, generalizada a varias filas
`NOT MET`: sin ninguna `FAIL`, `PASS: all <n> checks passed` si no hay `NOT MET`, y
`PASS: <p> of <n> checks passed, <k> not met by CanopyDB (<hallazgos>)` si las hay, con los hallazgos de las
filas `NOT MET` en el orden de las filas, separados por `, ` y sin repetir; con alguna `FAIL`,
`FAIL: <f> of <n> checks failed`.

## Documentación

- **Ayuda** del consultor CanopyDB, con los cambios del paso 1 del flujo.
- **README**: un apartado del consultor CanopyDB (para qué sirve, cómo se ejecuta, en qué se diferencia del
  original: la espera ante un conjunto abierto, el rechazo de conjuntos de otra versión de CanopyDB y el
  `schema_version` del JSON), de su verificación de aceptación y del conjunto fijo de consultas, con la
  configuración de memoria de CanopyDB que se use.
- **Guías de Linux**: cómo compilar y ejecutar el consultor CanopyDB.

## Reutilización

- **Consultor original**: opciones, validaciones, mensajes, resolución de consultas (C10), formatos y archivo de
  salida, lote, canales y códigos de salida, cancelación. Solo cambia de dónde se leen las tablas. Su código se
  puede reorganizar para compartirlo, sin cambiar su comportamiento (KC9).
- **Contrato compartido del conjunto**: codificación de clave y valor, índices y puntero a la versión activa.
- **Verificador CanopyDB**: su forma de leer la base sin cambiarla, su lectura del manifiesto CanopyDB y su modo
  de comparación (AC-M6, AC-KC4).
- **Verificación de aceptación del consultor original**: sus casos y su medición de memoria desde fuera, con los
  cambios de este documento.
- **Verificación de aceptación del generador CanopyDB**: su comprobación de que leer no añade archivos
  (AC-KG4) y su tratamiento de `-NotMetByCanopyDb`.

## Fuera de alcance

- El banco de comparación y su informe (`canopy-db-comparacion`) y la búsqueda dirigida de bugs
  (`canopy-db-robustez`): entre otros, matar un consultor o un lote a mitad, varios consultores esperando a la
  vez y el orden en que entran, consultas durante la publicación más allá de AC-M7, y borrar una versión que un
  consultor CanopyDB tiene abierta. *Anotación del 2026-10-22: los fija y los ejecuta el
  [PRD de robustez](canopy-db-robustez.prd.md), escenarios R3 a R12.*
- Una opción para cambiar el tiempo de espera: el contrato no tiene opciones nuevas (K1).
- Leer un conjunto escrito con otra versión de CanopyDB o convertirlo: se regenera (KC4).
- Un conjunto llevado a un equipo con otro orden de bytes, y conjuntos en carpetas de red o en discos de Windows
  montados en Linux: no se garantiza ni la espera de KC2 ni un mensaje concreto.
- Detectar una base dañada que CanopyDB lee sin dar error.
- Un modo de solo lectura en CanopyDB y la lectura de varios procesos a la vez (H2 y H3).
- Consultar conjuntos en carpetas o soportes de solo lectura: CanopyDB necesita escribir en sus archivos
  internos, así que se rechazan como un error del motor (KC5).
- Cambiar el generador CanopyDB, su manifiesto o el verificador CanopyDB.
- Cambiar el comportamiento del consultor original, salvo corregirlo si no cumple AC-M8 (K8).
- Del flujo E del padre, todo salvo el alta en el registro de hallazgos.
- Un objetivo de tiempo para las consultas, salvo AC-C7 heredado.
- Asociar con `-NotMetByCanopyDb` criterios distintos de AC-C7, AC-S8 y AC-M8.
- Ejecutar la verificación de aceptación y medir AC-M8 en Linux.

## Queda a criterio de la implementación

- La configuración de CanopyDB en el consultor, incluido cómo se limita su memoria (K8), siempre que se cumplan
  el contrato, K6 y AC-M8. El README, el informe de cierre y, después, el informe de comparación la indican.
- Cómo se espera y se reintenta la apertura, siempre que se cumplan los tiempos de
  [Validaciones y mensajes](#validaciones-y-mensajes).
- El texto de la evidencia de cada fila del informe, siempre que `AC-M2-F<n>` muestre el número de resultados de
  cada consultor y las filas `AC-M8` sus tres picos en MiB y el cociente.
- Cómo provocan las pruebas los errores del motor de AC-KC5, siempre que no añada ninguna opción visible ni
  cambie una ejecución normal.
- Dónde vive el archivo del conjunto fijo de consultas, siempre que el banco de comparación lo pueda
  reutilizar.
- Adónde va la salida estándar de las consultas mientras la verificación mide su memoria.

## Criterios de aceptación

Las cifras corresponden al CSV de referencia, con la misma salvedad que en el padre. Un criterio heredado que
CanopyDB no puede cumplir se trata según K10, con la asociación de KC8. AC-KC1 se comprueba con las filas del
original; AC-KC2 a AC-KC7 y los criterios del módulo son filas propias del informe de aceptación; AC-KC8 y
AC-KC9 se comprueban al cerrar y se anotan en el informe de cierre. Una consulta con `--count` solo da un número,
así que en ella "el mismo resultado" es el mismo conteo; el conjunto de registros se compara en las que listan.

1. **AC-KC1** (AC-M1, parte del consultor) — Dado el CSV de referencia generado con el generador CanopyDB,
   cuando se ejecuta la verificación de aceptación, entonces el consultor CanopyDB cumple AC-C1 a AC-C18, AC-S1
   a AC-S12 y los criterios del lote que comprueba la verificación del original, con las mismas cifras; AC-M8
   sustituye a AC-C13, y el rechazo de un conjunto SSTable de esquema 2 a la segunda mitad de AC-C18. Con el
   extracto de referencia se cumple AC-A1; con una descarga posterior, AC-A2.
2. **AC-M2** (KC7) — Dado el mismo CSV generado con los dos generadores, cuando se ejecuta con los dos
   consultores cada consulta del conjunto fijo, entonces el número de resultados es igual en las once; en F1,
   F2 y F11 el conjunto de registros devueltos es el mismo, sin tener en cuenta el orden; y F10 devuelve con
   cada consultor 1.000 registros, todos del resultado de F11.
3. **AC-M5** (K6, KC2) — Dado un lote del consultor CanopyDB en curso que dura más de 45 s (repitiendo F11 las
   veces necesarias), cuando un segundo consultor CanopyDB abre el mismo conjunto, entonces muestra el aviso de
   espera en la salida de errores, se rechaza con su mensaje y el código 1 entre 30 y 35 s después del aviso, y
   el lote termina con normalidad. Con un lote que termina antes de los 30 s, el segundo muestra el aviso y
   responde con normalidad. Si el analista pulsa Ctrl+C durante la espera, el segundo termina con
   `Consulta cancelada.` y el código 130 en menos de 1 s.
4. **AC-M6** (K6) — Dado un conjunto CanopyDB recién publicado y una copia de su carpeta de salida hecha antes de
   abrirlo, cuando se ejecutan todas las consultas del conjunto fijo, sueltas y en lote, entonces la carpeta de
   versión tiene los mismos nombres de archivo que la copia, sin ninguno nuevo, el manifiesto es idéntico y el
   modo de comparación del verificador CanopyDB entre la carpeta y la copia da las seis tablas iguales.
5. **AC-M7** (K7) — Dado el conjunto fijo de consultas, cuando se ejecuta en lote con el consultor CanopyDB,
   entonces cada consulta devuelve el mismo número de resultados que suelta, F1, F2 y F11 el mismo conjunto de
   registros, y F10 1.000 registros del resultado de F11. Dado un lote del consultor CanopyDB con el conjunto
   fijo que ya ha ejecutado F1, cuando el generador CanopyDB publica una versión nueva, entonces el JSON de F2
   indica la misma versión que el de F1, la que abrió el lote, y el lote termina con normalidad.
6. **AC-M8** (K8) — Dados, con cada consultor por separado, el conteo F3, el conteo F4 y un lote con todo el
   conjunto fijo, cuando se mide el pico de memoria de cada ejecución, entonces el mayor de los tres picos no
   supera 3 veces el menor. No se comparan los picos de un consultor con los del otro. El informe anota los seis
   picos.
7. **AC-KC2** (KC3) — Dados un conjunto SSTable de esquema 2 y otro de esquema 1, y conjuntos CanopyDB con el
   manifiesto cambiado a `schema_version` `canopydb-3`, `canopydb-02`, `canopydb-x`, `canopydb-`, `CANOPYDB-2`,
   el número 2, el texto `"2"` y sin `schema_version`;
   sin `tables`; con cinco tablas; con las tablas en otro orden; con `engine.name` distinto; sin
   `engine.fork_commit`; y con un campo desconocido añadido, cuando se consultan con el consultor CanopyDB,
   entonces cada uno da el mensaje de su fila en [Validaciones y mensajes](#validaciones-y-mensajes) y el código
   1, salvo el del campo desconocido, que responde con normalidad; y ninguna carpeta de versión cambia.
8. **AC-KC3** (KC4) — Dado un conjunto CanopyDB con `engine.version` cambiada a `0.2.4` en el manifiesto, y otro
   con un `fork_commit` de 40 caracteres, cuando se consultan, entonces se rechazan con el mensaje de otra
   versión, con las versiones escritas como dice [Validaciones y mensajes](#validaciones-y-mensajes), y el
   código 1, sin abrir la base: `canopydb/` no cambia.
9. **AC-KC4** — Dado un conjunto CanopyDB sin `canopydb/`, otro sin uno de los archivos que lista `files`, otro
   en el que una de esas rutas es una carpeta, y otro cuya base no tiene una de las seis tablas, cuando se
   consultan, entonces se rechazan con `No se encontró un conjunto de datos válido en <carpeta>.`, sin línea de
   detalle, y el código 1; la carpeta de versión no tiene ningún archivo nuevo, y en el último la base sigue sin
   esa tabla. Con un archivo de más en `canopydb/` que `files` no lista, la consulta responde con normalidad.
10. **AC-KC5** (KC5) — Dado un error del motor provocado en una prueba (al abrir, a mitad de un listado en CSV
    con `--output` y sin él, un fallo inesperado del motor y un texto de error con saltos de línea), cuando se
    consulta, suelta y en lote, entonces el mensaje es `No se pudo leer el conjunto de datos en <carpeta>.`, la
    salida de errores tiene justo antes una sola línea `Detalle del error de CanopyDB:` con el texto del error
    en una línea, el código es 1, no queda ningún archivo de salida a medias y la salida estándar es la del
    original ante un conjunto que deja de poderse leer. Dado en Windows un conjunto con los archivos de
    `canopydb/` marcados de solo lectura, cuando se consulta, entonces se rechaza con ese mismo mensaje, su
    detalle y el código 1, y los archivos no cambian.
11. **AC-KC6** (KC6) — Dada una consulta en JSON, cuando se ejecuta con el consultor CanopyDB, entonces la
    salida tiene la forma del original y `dataset.schema_version` es `"canopydb-2"`.
12. **AC-KC7** (KC8) — Dada la verificación con `-NotMetByCanopyDb AC-M8=<hallazgo>` y un consultor CanopyDB cuyo
    cociente supera 3, cuando termina, entonces la fila `AC-M8` dice `NOT MET` con su hallazgo, el resultado
    global es `PASS: <p> of <n> checks passed, 1 not met by CanopyDB (<hallazgo>)` si no hay otra `FAIL` y el
    código es 0; con dos filas `NOT MET` el resultado dice `2 not met` y sus dos hallazgos; si el que supera el
    cociente es el consultor original, la fila `AC-M8 (SSTables)` dice `FAIL` y el código es 1; y una
    asociación con otro criterio termina con 1 sin ejecutar nada, con el error de
    [Informe de la verificación de aceptación](#informe-de-la-verificación-de-aceptación).
13. **AC-KC8** (KC9) — Dado el repositorio en Windows y en Linux (Ubuntu 24.04 sobre WSL2), cuando se pasan las
    pruebas, entonces pasan todas, incluidas las del consultor CanopyDB; las pruebas de las aplicaciones
    originales que existían antes siguen existiendo y pasando sin cambiar lo que comprueban; y en Windows la
    verificación de aceptación del consultor original sigue pasando.
14. **AC-KC9** (KC1) — Dados la ayuda y el README, cuando se leen, entonces la ayuda del consultor CanopyDB es la
    del original con los tres cambios del paso 1 del flujo, y el README tiene el apartado de
    [Documentación](#documentación), con el conjunto fijo de consultas.

## Decisiones

KC2, KC4, KC6, KC7 y KC8 se tomaron con el usuario el 2026-10-22; las demás se derivan de la ascendencia y de
las decisiones del generador CanopyDB.

- **KC1 — Nombre y ayuda**: `crime-canopy-query`, con el patrón de KG1. La ayuda cambia la descripción y añade
  el párrafo de la espera; es una excepción a K1, como la ayuda del generador CanopyDB.
- **KC2 — Espera ante un conjunto abierto** (K6): fija, de 30 s, sin opción para cambiarla; un aviso en cuanto
  se detecta y un mensaje propio al agotarse, con el código 1. Motivo: una consulta suelta suele tardar unos
  segundos, así que 30 s cubren casi siempre la consulta ajena sin dejar al analista esperando a ciegas; el
  aviso explica la pausa. Se descartaron los 5 s de CanopyDB (fallan con cualquier lote o listado grande ajeno)
  y 60 s.
- **KC3 — Conjunto de otro motor o de otro esquema**: solo se acepta `schema_version` `canopydb-2` con la forma
  del manifiesto del generador CanopyDB. Un `canopydb-<cifras>` con otro número usa el mensaje de esquema
  incompatible del original con esas cifras; cualquier otro valor, incluido un número (un conjunto SSTable), es
  un conjunto inválido, como fija el padre.
- **KC4 — Otra versión de CanopyDB**: se rechaza antes de abrir la base, con un mensaje nuevo, si
  `engine.version` o `engine.fork_commit` no son exactamente los del consultor. Motivo: los archivos de CanopyDB
  no indican su versión, así que abrir una base de otra versión podría dar resultados falsos sin ningún error, y
  el mensaje dice al analista que basta con regenerar. El padre deja a este hijo cómo se rechaza. Se
  descartaron el mensaje existente de conjunto inválido, que no dice qué hacer, e intentar abrir la base.
- **KC5 — Errores del motor** (K9): cualquier error de CanopyDB al abrir o leer, salvo la espera de KC2, usa
  `No se pudo leer el conjunto de datos en <carpeta>.`, el mensaje del original para un archivo que existe y no
  se puede leer, y lleva una línea de detalle con el texto de KG7. Lo que el consultor comprueba por sí mismo
  (manifiesto, archivos de `files` y tablas) usa `No se encontró…` sin detalle, como el original. Motivo: el
  consultor no tiene log, así que el detalle va por el canal de avisos (C5).
- **KC6 — `schema_version` en el JSON**: el texto del manifiesto, `"canopydb-2"`. Es una excepción a K1 que solo
  vale para el consultor CanopyDB. Motivo: C9 pide que el resultado documente sobre qué datos se consultó, y el
  texto dice además el motor. Se descartó mostrar `2`, que mantenía la salida idéntica sin excepción.
- **KC7 — Conjunto fijo de consultas**: las once de [Conjunto fijo de consultas](#conjunto-fijo-de-consultas),
  que cubren las clases de K3, con un listado grande (F11), y los casos de memoria de AC-M8, con valores que ya
  usa la verificación del consultor original. Se descartó dejar fuera F11.
- **KC8 — Verificación de aceptación**: `run-canopy-query-acceptance.ps1`, con el patrón de KG12, y el estado
  `NOT MET` de KG10 para los tres criterios heredados que K10 prevé que CanopyDB pueda impedir: el tiempo por ID
  (AC-C7) y la memoria (AC-S8 y AC-M8), solo del consultor CanopyDB. Amplía la lista de KG10, que solo admitía
  AC-M4. Se descartó admitir solo AC-M8.
- **KC9 — Aplicaciones originales**: como KG11, se puede reorganizar el código del consultor original para
  compartirlo, sin cambiar su comportamiento, sus mensajes ni sus salidas. Si el original no cumple AC-M8, se
  corrige (K8).

## Anotaciones al cerrar

Al cerrar este hijo se anotan, sin reescribir su historia, el [PRD del consultor](consultor.prd.md) (que no tiene
traducción) y el [PRD de CanopyDB](canopy-db.prd.md) con su traducción. Las dos primeras son las que pide
[Criterios y anotaciones por hijo](canopy-db.prd.md#criterios-y-anotaciones-por-hijo); las otras dos se añaden
por KC6:

- **Consultor**, en "Cachés o índices auxiliares creados por el consultor: el consultor nunca escribe en la
  carpeta del conjunto" (Fuera de alcance): *Anotación del <fecha>: el consultor CanopyDB puede escribir en los
  archivos internos del motor, sin cambiar el contenido consultable ni el manifiesto ni crear archivos (decisión
  K6 de [canopy-db.prd.md](canopy-db.prd.md)); el consultor original no cambia.*
- **Consultor**, en C7: *Anotación del <fecha>: vale también para el consultor CanopyDB y para el lote, y se
  comprueba con el cociente máximo de 3 de AC-M8 (decisión K8 de [canopy-db.prd.md](canopy-db.prd.md),
  [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)).*
- **Consultor**, en C9: *Anotación del <fecha>: en el consultor CanopyDB, `schema_version` es el texto del
  manifiesto, `"canopydb-2"` (decisión KC6 de [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)); el
  consultor original no cambia.*
- **CanopyDB**, en el contrato común (la lista de excepciones de K1): *Anotación del <fecha>: el consultor
  CanopyDB añade dos excepciones, su ayuda y el `schema_version` de su salida JSON (decisiones KC1 y KC6 de
  [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)).*

## Referencias

- Documento padre: [canopy-db.prd.md](canopy-db.prd.md)
- Documentos cuyo contrato hereda la aplicación nueva (K1): [consultor.prd.md](consultor.prd.md),
  [startswith.prd.md](startswith.prd.md) y [canopy-db-lote.prd.md](canopy-db-lote.prd.md)
- Hermano: [canopy-db-generador.prd.md](canopy-db-generador.prd.md)
- Documento raíz: [chicago-crime.prd.md](chicago-crime.prd.md)
- [PRD de soporte para Linux](soporte-linux.prd.md) y [PRD de aceptación con cualquier
  CSV](aceptacion-cualquier-csv.prd.md)
- CanopyDB: https://github.com/arthurprs/canopydb y https://docs.rs/canopydb

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 71fbd401 | 2026-10-22 06:10 UTC+01:00 | 23 min | 0 min | 23 min | 19 min | 4 min | 5 |

<!-- prd-time: [{"session": "71fbd401-ac30-467d-8555-d070e19606b9", "start": "2026-10-22T05:10:44.656000+00:00", "wall": 1399, "discarded": 0, "agent": 1156, "human": 242, "interventions": 5, "idle_minutes": 10}] -->
