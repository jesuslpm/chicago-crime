# Lote de consultas en el consultor — PRD

**Versión**: 1.0
**Fecha**: 2026-10-21
**Documento padre**: [Almacén de delitos de Chicago sobre CanopyDB — PRD](canopy-db.prd.md) (v1.0)

## Objetivo y contexto

Que el analista pueda ejecutar **varias consultas en una sola ejecución** del consultor original: escribe un
archivo con una consulta por línea, el consultor abre el conjunto una vez, las ejecuta una tras otra en el
orden del archivo y termina. Es la decisión K7 del padre y el segundo hijo de su
[reparto](canopy-db.prd.md#reparto-en-prd-hijos).

**Por qué ahora**: el banco de comparación del módulo CanopyDB (K3) mide las consultas también en lote, para
no medir sobre todo el arranque y la apertura en frío. El consultor CanopyDB nacerá con el lote
(`canopy-db-consultor`) y el original lo necesita antes, para que los dos respondan igual a lo mismo (K1).
Este hijo no depende de `canopy-db-generador` y no toca nada del consultor CanopyDB.

Este documento no repite las reglas del consultor (C1–C11), de "empieza por" (P1–P6) ni de la ascendencia,
que siguen vigentes para cada consulta del lote. Solo añade cómo se pide el lote, cómo se escribe el archivo,
qué sale y cómo terminan los errores. Sustituye o precisa estas reglas de la ascendencia:

- **D6** del [documento padre raíz](chicago-crime.prd.md) (y el paso 1 de su flujo B) y el paso 1 del flujo
  del [PRD del consultor](consultor.prd.md): una ejecución responde una consulta **o un lote de consultas**.
  Sigue siendo una ejecución que arranca, responde y termina; no hay modo interactivo.
- **C6** del consultor: el código 2 ("la línea de comandos no es válida") vale también para una línea del lote
  mal formada, que es la línea de comandos de su consulta (decisión B4).
- **K8** del padre: en el lote, lo que no crece con el número de consultas es la memoria de cada consulta; las
  consultas ya comprobadas del archivo se pueden guardar (decisión B7).

Las anotaciones que deja en esos documentos están en [Anotaciones al cerrar](#anotaciones-al-cerrar).

### Estado de partida

Verificado el 2026-10-21 sobre el repositorio:

| Aspecto | Situación |
|---|---|
| Consultor original | Implementado y aceptado (consultor v1.1, AC-C1 a AC-C18; "empieza por", AC-S1 a AC-S12). Responde una sola consulta por ejecución (D6). No existe ninguna opción de lote. |
| Comprobaciones | La forma de la línea de comandos se comprueba primero (código 2); después, las condiciones (código 1), sin abrir el conjunto; después se abre el conjunto (código 1). Un archivo de salida que no se puede preparar se rechaza justo antes de ejecutar la consulta (código 1). |
| Salida | Resultados en la salida estándar o en el archivo de `--output`; avisos, errores y una línea de estadísticas en la salida de errores (C5), con la forma `1.234 resultados en 0,041 s. Índices usados: Primary Type, Date. Filtros sin índice: ninguno.` Con `--count`, solo el número en la salida estándar. Si la salida estándar se cierra (por ejemplo, `head`), termina sin mensaje ni estadísticas y con el código 0. |
| Verificación de aceptación | Un script de Windows ejecuta las consultas de los criterios del consultor y de "empieza por" contra el filtrado directo del CSV, comprueba los mensajes y la cancelación con Ctrl+C y mide el pico de memoria (AC-C13, AC-S8). No conoce el lote. |

## Actores

- **Analista**: el del consultor. Es el único actor de esta entrega.

## Flujo

Sigue el flujo del consultor; esta entrega concreta el flujo C del padre:

1. El analista escribe un archivo de lote con una consulta por línea (ver [Archivo de lote](#archivo-de-lote)).
2. Ejecuta el consultor con `--dataset <CARPETA> --batch <ARCHIVO>` (decisión B1).
3. El consultor comprueba la línea de comandos, lee el archivo y comprueba cada línea, en orden, con las mismas
   reglas que una consulta suelta, y después las reglas propias del lote. Todo antes de abrir el conjunto: si
   algo está mal, rechaza el lote con el primer error y no ejecuta ninguna consulta (decisión B4).
4. Abre el conjunto una vez y ejecuta las consultas una tras otra, en el orden del archivo, sobre la versión
   que abrió (flujo C del padre).
5. Antes de cada consulta escribe su cabecera en la salida de errores. Después, la consulta hace exactamente
   lo que haría suelta: prepara su archivo de salida si lo tiene, escribe sus avisos, sus resultados y su línea
   de estadísticas (decisión B3).
6. Si una consulta falla o el analista cancela, el lote para ahí; lo que ya escribieron las consultas
   anteriores se conserva (decisión B6).
7. Al terminar todas, escribe la línea final del lote con el número de consultas y el tiempo total.

### Archivo de lote

Decisión B2:

- **Codificación**: UTF-8, con o sin marca de orden de bytes al principio.
- **Finales de línea**: LF o CRLF, mezclados o no; la última línea puede no terminar en salto de línea. Las
  líneas se numeran desde 1 tal como están en el archivo, contando las vacías y los comentarios.
- **Líneas ignoradas**: las vacías o con solo espacios y tabuladores, y las que empiezan por `#` tras espacios
  opcionales. Un `#` en cualquier otro sitio es un carácter más. Una línea con algo más, aunque sean solo unas
  comillas vacías (`""`), es una consulta.
- **Palabras**: solo los espacios y tabuladores separan las palabras, como en la línea de comandos. Unas
  comillas dobles o simples agrupan un texto con espacios y pueden abrirse a mitad de palabra
  (`--where="Primary Type=THEFT"`); dentro de unas comillas, las del otro tipo son un carácter más
  (`"Location Description=O'HARE"`). No hay secuencias de escape: `\` es un carácter más. Cualquier otro
  carácter (un retorno de carro suelto, un carácter nulo, un espacio de no separación, una marca de orden de
  bytes fuera del principio) es un carácter más de su palabra.
- **Opciones**: cada línea admite las opciones de una consulta suelta salvo `--dataset`, `--batch`, `--help` y
  `-h`, es decir `--where`, `--startswith`, `--range`, `--limit`, `--count`, `--format` y `--output`, también
  con la forma `--opción=valor` (decisión B1).
- **Rutas**: una ruta relativa de `--output` se resuelve desde la carpeta en que se ejecuta el consultor, como
  en una consulta suelta. Para comparar dos rutas (decisión B5) se resuelven de forma léxica: ruta absoluta
  desde la carpeta actual, sin `.` ni `..`, sin consultar el disco y, en Windows, sin distinguir mayúsculas.
  Así `crim.csv`, `./crim.csv` y `sub\..\CRIM.csv` son la misma ruta, aunque el archivo aún no exista.

Ejemplo:

```text
# Robos y daños de enero de 2020
--where "Primary Type=THEFT" --range Date=2020-01-01..2020-01-31 --count
--startswith "Primary Type=CRIM" --range Date=2020-01-01..2020-01-31 --format csv --output crim.csv

--where ID=13209277 --format json
```

## Validaciones y mensajes

Cada línea del lote tiene las validaciones, los mensajes y los códigos de una consulta suelta (consultor,
"empieza por" y padre raíz), con el prefijo `Línea <n>: ` delante del mensaje, sin cambiarlo (incluida la
pista final ` Usa --help para ver las opciones.`). `<archivo>` es la ruta de `--batch` y `<ruta>` la de
`--output`, tal como se escribieron. Esta entrega añade:

| Situación | Regla | Mensaje al usuario | Código |
|---|---|---|---|
| `--batch` junto a `--where`, `--startswith`, `--range`, `--limit`, `--count`, `--format` u `--output` en la línea de comandos | Rechaza; esas opciones van en cada línea del lote. Con varias, se nombra la primera escrita | `Con --batch, la opción <opción> va en cada línea del lote, no en la línea de comandos.` | 2 |
| El archivo de lote no existe, es una carpeta o no se puede leer (también `--batch ""` y `--batch -`) | Rechaza | `No se puede leer el lote <archivo>.` | 1 |
| El archivo no es UTF-8 válido | Rechaza | `El lote <archivo> no es un texto UTF-8 válido.` | 1 |
| El archivo no tiene ninguna consulta (vacío, o solo líneas ignoradas) | Rechaza | `El lote <archivo> no contiene ninguna consulta.` | 1 |
| Una línea con unas comillas sin cerrar | Rechaza | `Línea <n>: falta cerrar unas comillas.` | 2 |
| Una línea con `--dataset`, `--batch`, `--help` o `-h` | Rechaza | `Línea <n>: la opción <opción> no se admite dentro del lote.` | 2 |
| Una línea que, suelta, se rechazaría por su forma o por sus condiciones | Rechaza | `Línea <n>: ` y el mensaje de la consulta suelta | El de la consulta suelta (2 o 1) |
| Una línea con el mismo archivo de salida que una línea anterior, comparando las rutas como dice [Archivo de lote](#archivo-de-lote) | Rechaza | `Línea <n>: el archivo de salida <ruta> ya lo usa la línea <m>.` | 1 |
| Una línea cuyo archivo de salida es el propio archivo de lote, con la misma comparación | Rechaza | `Línea <n>: el archivo de salida <ruta> es el propio lote.` | 1 |
| Una consulta falla durante el lote: el conjunto deja de poderse leer, su archivo de salida no se puede preparar o escribir, o la salida estándar falla | Para el lote (B6) | El de la consulta suelta, debajo de la cabecera de la consulta | 1 |
| Ctrl+C durante el lote, también antes de la primera consulta o entre dos consultas | Para el lote (B6). Entre dos consultas, la siguiente no empieza y no se escribe su cabecera | `Consulta cancelada.` | 130 |
| La salida estándar se cierra | Para el lote sin mensaje ni línea final, como una consulta suelta; lo ya escrito en la salida de errores (cabeceras y estadísticas) se queda | — | 0 |
| Todas las consultas se ejecutan, con o sin resultados ni avisos | — | La línea final de [Salida](#salida) | 0 |

**Orden de las comprobaciones** (decisión B4); se informa solo del primer error y su código es el del
resultado:

1. La línea de comandos, con los errores de forma que ya existen, que ganan: por ejemplo, `--batch` sin
   `--dataset` da el mensaje de opciones obligatorias, `--batch` repetido el de argumentos no válidos y
   `--limit abc` el del límite, todos con el código 2. Después, las opciones que no van con `--batch`. Con
   `--batch`, la línea de comandos no necesita ninguna condición.
2. La lectura del archivo y su codificación.
3. Cada línea, en orden: comillas, opciones no admitidas en el lote, forma y condiciones de la consulta suelta,
   y archivo de salida repetido o igual al lote.
4. Que haya al menos una consulta.
5. La apertura del conjunto, con los mensajes de la consulta suelta y sin prefijo.

Ninguna de estas comprobaciones crea ni modifica un archivo de salida.

## Salida

Decisión B3. Una consulta suelta, sin `--batch`, no cambia su salida.

- **Salida estándar**: lo que escribe cada consulta, una tras otra, exactamente como lo escribiría suelta
  (tabla, CSV, JSON o el número del conteo), sin separadores ni líneas en blanco añadidos. Una consulta con
  `--output` no escribe nada en ella. Así, la salida de un lote es la concatenación de las salidas de sus
  consultas sueltas.
- **Salida de errores**: para cada consulta, la cabecera `--- Consulta <i> de <total> (línea <n>) ---`, y
  debajo sus avisos, su error si falla y su línea de estadísticas, exactamente como suelta. En la cabecera y en
  el prefijo `Línea <n>: ` los números van sin separador de miles (`Consulta 1234 de 2000`). Al terminar todas,
  `Lote completado: <total> consultas en <t> s.` (`1 consulta` en singular), con los números y el tiempo en el
  formato de la línea de estadísticas. El tiempo cubre la ejecución entera: leer y comprobar el archivo, abrir
  el conjunto y todas las consultas. La línea final no sale si el lote se rechaza, falla, se cancela o se
  cierra la salida estándar.
- **Archivos de salida**: cada uno tiene exactamente el contenido que tendría con la consulta suelta sobre la
  misma versión; en el JSON (C9), la versión del conjunto es la misma en todas las consultas del lote.

Ejemplo con el archivo de [Archivo de lote](#archivo-de-lote). Salida estándar:

```text
1234
{"query": …, "dataset": …, "results": [
…
]}
```

Salida de errores (los tiempos varían):

```text
--- Consulta 1 de 3 (línea 2) ---
1.234 resultados en 0,041 s. Índices usados: Primary Type, Date. Filtros sin índice: ninguno.
--- Consulta 2 de 3 (línea 3) ---
2.345 resultados en 0,087 s. Índices usados: Primary Type, Date. Filtros sin índice: ninguno.
--- Consulta 3 de 3 (línea 5) ---
1 resultado en 0,002 s. Índices usados: ID. Filtros sin índice: ninguno.
Lote completado: 3 consultas en 0,152 s.
```

Si la segunda consulta no pudiera escribir `crim.csv`, la salida de errores terminaría así, con el código 1:

```text
--- Consulta 1 de 3 (línea 2) ---
1.234 resultados en 0,041 s. Índices usados: Primary Type, Date. Filtros sin índice: ninguno.
--- Consulta 2 de 3 (línea 3) ---
No se puede escribir el archivo de salida crim.csv.
```

## Reutilización

- **Consultor y "empieza por"**: opciones, validaciones y mensajes, resolución de consultas (C10), formatos y
  archivo de salida (C4, C8, C9), canales y códigos de salida (C5, C6), cancelación y memoria (C7). Cada
  consulta del lote se comprueba y se ejecuta con ellos, sin reglas paralelas.
- **Línea de estadísticas** de la consulta suelta, con su formato de número y de tiempo, también para la línea
  final del lote.
- **Verificación de aceptación del consultor**: se amplía con las comprobaciones del lote y sigue pasando
  entera (AC-M9).

## Fuera de alcance

- Lo que el padre deja fuera del lote (consultas en paralelo, lote desde la entrada estándar, modo
  interactivo). `-` como archivo de lote no tiene un significado especial.
- El consultor CanopyDB y su lote: los entrega `canopy-db-consultor`.
- Medir el pico de memoria de un lote (AC-M8): lo hace `canopy-db-consultor` con los dos consultores (K8).
- Una salida de lote que sea un único documento CSV o JSON válido: para separar resultados limpios, cada línea
  usa su `--output`.
- Consultas sobre varios conjuntos en un mismo lote.
- Continuar el lote después de una consulta que falla, reanudarlo desde una línea o ejecutar solo algunas
  líneas.
- Deshacer los archivos de salida de las consultas ya terminadas cuando el lote falla o se cancela.
- Comprobar antes de empezar que se podrán escribir los archivos de salida: se comprueba cada uno al llegar a
  su consulta, como en la consulta suelta.
- Detectar como iguales dos rutas que llevan al mismo archivo por enlaces simbólicos o duros.
- Impedir un `--output` dentro de la carpeta del conjunto: el lote lo trata como la consulta suelta, que hoy
  no lo impide.
- Opciones comunes a todas las líneas, variables, expansión de `~`, `%VAR%` o comodines, continuación de
  línea, inclusión de otros archivos y comentarios al final de una línea.
- Secuencias de escape en las comillas, y por tanto un valor que contenga a la vez comillas dobles y simples.
- Otras codificaciones del archivo, como UTF-16.
- Un límite de líneas o de tamaño del archivo de lote, y un objetivo de tiempo para el lote.
- Cambios en el generador, en el formato del conjunto o en el verificador independiente.

## Queda a criterio de la implementación

- El texto de la ayuda, incluida la línea de uso, siempre que describa `--batch`, la forma del archivo, las
  opciones que admite cada línea y los códigos de salida del lote.
- Cómo se lee y se guarda el archivo, dentro de B7.
- Un Ctrl+C que llega cuando la última consulta ya ha terminado puede acabar con la línea final y el código 0
  o con `Consulta cancelada.` y el código 130.

## Criterios de aceptación

Las cifras corresponden al CSV de referencia. Este hijo verifica además AC-M7 con el consultor original y
AC-M9 del padre; la memoria del lote (flujo C, K8) la verifica `canopy-db-consultor` en AC-M8. Los tiempos
de la salida se comprueban por su formato, no por su valor. Los momentos que no se pueden provocar de forma
determinista desde fuera del proceso (un lote detenido entre dos consultas o durante una, un conjunto que deja
de poderse leer, una salida estándar que falla) se pueden comprobar con pruebas que controlan ese momento sin
un proceso real; la cancelación con un Ctrl+C real se comprueba al menos una vez en la verificación de
aceptación.

1. **AC-B1** (AC-M7) — Dado un lote con todas las consultas que la verificación de aceptación del consultor y
   de "empieza por" ejecuta con éxito sueltas, cuando se ejecuta, entonces cada consulta devuelve el mismo
   conteo y el mismo conjunto de registros que suelta; las que tienen un límite devuelven tantos registros como
   el límite, todos del resultado sin límite.
2. **AC-B2** (AC-M7) — Dado un lote que ya ha abierto el conjunto y ha ejecutado su primera consulta, cuando el
   generador publica una versión nueva antes de que empiece la segunda, entonces todas sus consultas devuelven
   resultados de la versión que abrió y el JSON de cada una indica esa versión.
3. **AC-B3** (B2) — Dado un lote con líneas vacías, comentarios, finales LF y CRLF mezclados, marca de orden de
   bytes, última línea sin salto, comillas dobles y simples, comillas a mitad de palabra, `--opción=valor` y un
   `#` dentro de un valor, cuando se ejecuta, entonces cada consulta da lo mismo que suelta.
4. **AC-B4** (B4) — Dados un lote cuya tercera consulta, en la línea 5 tras una línea vacía y un comentario, se
   rechazaría suelta por su forma, y otro igual que se rechazaría por sus condiciones, cuando se ejecutan,
   entonces no se ejecuta ninguna consulta, no se crea ningún archivo de salida, el mensaje es `Línea 5: ` y el
   de la consulta suelta, y el código es 2 y 1. Dado un lote con dos líneas mal escritas, entonces solo se
   informa de la primera.
5. **AC-B5** — Dada cada fila de [Validaciones y mensajes](#validaciones-y-mensajes), cuando se provoca,
   entonces se muestra el mensaje indicado con su código.
6. **AC-B6** (B3) — Dado un lote con una consulta en tabla, una en CSV con `--output` y un conteo, cuando se
   ejecuta, entonces la salida estándar es exactamente la concatenación de las salidas estándar de las tres
   consultas sueltas, el archivo es idéntico al de la consulta suelta, y la salida de errores tiene la forma de
   [Salida](#salida): una cabecera por consulta, su línea de estadísticas y la línea final. Con un lote de una
   sola consulta, la línea final dice `1 consulta`.
7. **AC-B7** (B6) — Dado un lote de tres consultas cuya segunda escribe en un archivo que no se puede crear
   (en una carpeta que no existe, y en otra prueba, una ruta que es una carpeta), cuando se ejecuta, entonces la primera conserva sus resultados, la salida de errores termina con la cabecera
   de la segunda y su mensaje, la tercera no se ejecuta, no hay línea final y el código es 1.
8. **AC-B8** (B6) — Dado un lote de tres consultas con archivo de salida, detenido durante la segunda, cuando el
   analista pulsa Ctrl+C, entonces termina con `Consulta cancelada.` y el código 130, el archivo de la primera
   existe completo, el de la segunda no existe ni queda su archivo temporal, el de la tercera no existe y no
   hay línea final.
9. **AC-B9** — Dado `--help`, cuando se muestra, entonces describe `--batch`, la forma del archivo, las opciones
   que admite cada línea y los códigos de salida del lote; y el README y la guía del consultor explican el lote
   con un ejemplo.
10. **AC-B10** (AC-M9) — Dadas las pruebas y la verificación de aceptación de las aplicaciones originales,
    cuando se ejecutan, entonces pasan todas sin cambiar lo que comprueban, salvo el texto de la ayuda, más las
    nuevas del lote.

## Decisiones

Todas se tomaron con el usuario el 2026-10-21.

- **B1 — Cómo se pide el lote**: `--batch <ARCHIVO>` junto a `--dataset`. Cada línea admite todas las
  opciones de una consulta suelta salvo `--dataset`, `--batch`, `--help` y `-h`; con `--batch`, esas opciones
  no se admiten en la línea de comandos. Motivo: K7 dice que cada línea se escribe con las mismas opciones que
  una consulta suelta, y el estilo sigue al de `--where` y `--startswith`. Se descartaron un argumento
  posicional, un subcomando, y `--format` y `--output` globales o como valor por defecto de las líneas, que
  añaden reglas de combinación y obligarían a juntar resultados en un archivo.
- **B2 — Forma del archivo**: la de [Archivo de lote](#archivo-de-lote). Palabras como en un shell sin
  escapes, para copiar consultas de una consola tal cual y escribir rutas de Windows sin duplicar `\`;
  comentarios solo en líneas propias, para no romper un valor con `#`; UTF-8 con o sin marca de orden de bytes
  y LF o CRLF, para aceptar lo que guardan el Bloc de notas y PowerShell 5.1. Se descartaron comillas solo al
  principio de palabra, escapes con `\`, comentarios al final de línea, UTF-8 estricto sin marca y UTF-16.
- **B3 — Salida y separación**: la salida estándar recibe solo lo que escribiría cada consulta suelta; la
  identificación de cada consulta, como cabecera, y la línea final con el tiempo total van a la salida de
  errores. Motivo: respeta C5, que existe para guardar o encadenar CSV y JSON tal cual, y reutiliza los avisos
  y la línea de estadísticas sin tocarlos. Se descartaron el separador en la salida estándar (rompe C5 y
  AC-C8), el separador solo en tabla, un prefijo en cada línea de la salida de errores, el tiempo como suma de
  las consultas y no tener línea final.
- **B4 — Comprobación previa y códigos**: todo el archivo se comprueba antes de abrir el conjunto, en el orden
  de [Validaciones y mensajes](#validaciones-y-mensajes), y cualquier error rechaza el lote sin ejecutar nada.
  Se informa solo del primer error y su código es el del resultado: una línea mal formada da 2, como la línea
  de comandos de su consulta (amplía C6); el archivo ilegible, sin consultas o con un archivo de salida repetido
  da 1, como un dato rechazado. Los mensajes de la consulta suelta se reutilizan tal cual tras `Línea <n>: `.
  Motivo: un error que se puede ver antes no debe dejar resultados a medias. Se descartaron informar de todos
  los errores, fallar al llegar a la línea, y dar a todo el archivo un único código.
- **B5 — Archivo de salida repetido**: dos líneas con el mismo `--output`, o un `--output` que es el propio
  archivo de lote, rechazan el lote antes de empezar, comparando las rutas de forma léxica (ver
  [Archivo de lote](#archivo-de-lote)), sin consultar el disco. Un `--output` dentro de la carpeta del conjunto
  se trata como en la consulta suelta. Que un archivo de salida se pueda escribir se comprueba al llegar a su consulta, como en la
  consulta suelta. Motivo: evitar perder resultados o el propio lote sin aviso, sin crear nada antes de
  tiempo. Se descartaron sobrescribir sin regla nueva, comparar solo el texto de la ruta, resolver la ruta
  real en el disco (exige que la carpeta exista), preparar todos los archivos antes de empezar y rechazar un
  `--output` dentro de la carpeta del conjunto solo en el lote.
- **B6 — Fallos y cancelación durante el lote**: el lote para en la consulta que falla (código 1) o en la que
  está en curso al pulsar Ctrl+C (`Consulta cancelada.`, código 130); esa consulta no deja archivo de salida a
  medias, las anteriores conservan sus resultados y sus archivos, las siguientes no se ejecutan y no sale la
  línea final. Cada consulta cumple así C8 por separado. Motivo: lo terminado es válido y un conjunto ilegible
  haría fallar todas las siguientes. Se descartaron seguir con las demás, deshacer los archivos anteriores y un
  mensaje de cancelación propio del lote.
- **B7 — Memoria**: precisa K8 para el lote. Lo que no crece con el número de consultas es la memoria que usa
  cada una: ninguna conserva para la siguiente sus resultados ni nada derivado de su consulta; lo único común
  es el conjunto abierto. Las consultas ya comprobadas del
  archivo se pueden guardar, y eso crece con el tamaño del archivo, despreciable frente al pico de una consulta.
  Motivo: la comprobación previa de B4 necesita el archivo entero. Se descartó un pico independiente del
  tamaño del archivo, que obligaría a leerlo dos veces y a rechazarlo si cambia entre las dos lecturas.

## Anotaciones al cerrar

Al cerrar este hijo se anotan, sin reescribir su historia:

- **Documento padre raíz**, en la decisión D6 y en el paso 1 del flujo B: *Anotación del <fecha>: sustituida
  en parte por el [PRD del lote de consultas](canopy-db-lote.prd.md) (decisión K7 del
  [PRD de CanopyDB](canopy-db.prd.md)); una ejecución del consultor responde una consulta o un lote de
  consultas, y sigue arrancando, respondiendo y terminando.*
- **PRD del consultor**, en el paso 1 del flujo ("Cada ejecución responde una consulta y termina (D6)") y en
  C6: la misma anotación, y en C6 además *el código 2 vale también para una línea del lote mal formada
  (decisión B4 del [PRD del lote de consultas](canopy-db-lote.prd.md)).*
- **PRD de CanopyDB**, en K8: *Anotación del <fecha>: en el lote, las consultas ya comprobadas del archivo se
  pueden guardar; lo que no crece es la memoria de cada consulta (decisión B7 del
  [PRD del lote de consultas](canopy-db-lote.prd.md)).*

Las dos primeras son las que pide la tabla
[Criterios y anotaciones por hijo](canopy-db.prd.md#criterios-y-anotaciones-por-hijo) del padre; la de C6 y la
de K8 se añaden por las decisiones B4 y B7.

## Referencias

- Documento padre: [canopy-db.prd.md](canopy-db.prd.md)
- Documento padre general: [consultor.prd.md](consultor.prd.md)
- Documento padre raíz: [chicago-crime.prd.md](chicago-crime.prd.md)
- [PRD de "empieza por"](startswith.prd.md)

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| ea0ef110 | 2026-10-21 07:24 UTC+01:00 | 24 min | 0 min | 24 min | 15 min | 8 min | 18 |

<!-- prd-time: [{"session": "ea0ef110-7116-4e15-bcfe-d1c9b40cd59a", "start": "2026-10-21T06:24:22.769000+00:00", "wall": 1428, "discarded": 0, "agent": 921, "human": 507, "interventions": 18, "idle_minutes": 10}] -->
