# Almacén de delitos de Chicago sobre CanopyDB — PRD

**Versión**: 1.0
**Fecha**: 2026-10-20
**Documento padre**: [Almacén consultable de delitos de Chicago sobre SSTable — PRD](chicago-crime.prd.md) (v1.2)

## Objetivo y contexto

Construir una segunda pareja de aplicaciones, un **generador** y un **consultor**, que hacen lo mismo que las
actuales pero guardan el conjunto de datos en **CanopyDB**, un motor de almacenamiento clave-valor
transaccional escrito en Rust, en lugar de en SSTables. Este documento es el paraguas del módulo: fija sus
objetivos, el contrato que comparten las dos parejas y las reglas que valen para todo el módulo, y lo reparte en
PRD hijos más pequeños, cada uno con su propia ejecución y su propio cierre (ver
[Reparto en PRD hijos](#reparto-en-prd-hijos)).

**Objetivos**:

1. **Comparar el rendimiento** de CanopyDB con el enfoque original de SSTables sobre el mismo CSV, el mismo
   equipo y las mismas consultas, con un banco de pruebas que se pueda repetir cuando salga una versión nueva
   de CanopyDB.
2. **Detectar bugs de CanopyDB** usándolo con un volumen real (8,66 millones de registros) y con las
   exigencias del contrato actual: memoria limitada, publicación atómica, interrupciones y Linux.
3. **Contribuir al repositorio de CanopyDB** con lo que se encuentre (issues con reproducción mínima y, cuando
   se encuentre la causa, PR con la corrección), para ayudar a que madure.

**Por qué**: el enfoque original ya está implementado y aceptado, así que sirve de referencia medida. CanopyDB
es un proyecto joven: su autor lo declara software en fase temprana y no recomienda usarlo con datos de
producción. Un caso real y exigente como este es justo lo que necesita para madurar, y la comparación indica si
un motor transaccional compensa frente a tablas inmutables para este tipo de consultas.

**Contrato común** (decisión K1): las aplicaciones CanopyDB tienen las mismas opciones, mensajes, salidas y
códigos de salida que las originales, y cumplen las reglas del padre, del [PRD del generador](generador.prd.md),
del [PRD del consultor](consultor.prd.md), del [PRD de soporte para Linux](soporte-linux.prd.md) y del
[PRD de aceptación con cualquier CSV](aceptacion-cualquier-csv.prd.md). Las excepciones son de dos clases y
solo valen para las aplicaciones CanopyDB; las originales conservan todas sus reglas:

- las que fija este documento: decisiones K2, K6, K8 y K9;
- las que dependen del formato y fija `canopy-db-generador`: los nombres de las aplicaciones, la forma de la
  carpeta publicada y del manifiesto, la lista de archivos del resumen final y lo que hace la opción sin
  compresión.

*Anotación del 2026-10-22: el consultor CanopyDB añade dos excepciones, su ayuda y el `schema_version` de su
salida JSON (decisiones KC1 y KC6 de [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)).*

El lote de consultas (K7) es la única novedad que llega también al consultor original. Así, ante la misma
consulta, las dos parejas devuelven los mismos registros y la comparación es justa.

### Estado de partida

Verificado el 2026-10-20 sobre el repositorio, el repositorio público de CanopyDB y una prueba en el equipo de
referencia:

| Aspecto | Situación |
|---|---|
| Aplicaciones originales | Generador y consultor sobre SSTables implementados y aceptados (padre v1.2, generador v1.0, consultor v1.1), con una biblioteca compartida que describe el contrato del conjunto, un verificador independiente y scripts de aceptación para Windows. Compilan y pasan las pruebas también en Linux. |
| Consultor original | Responde una sola consulta por ejecución (decisión D6 del padre). No puede ejecutar varias consultas en un mismo proceso. |
| Conjunto ajeno en el consultor original | Lee primero la versión del esquema del manifiesto: si no es la suya, rechaza con el mensaje de esquema incompatible; si el manifiesto no tiene la forma esperada, rechaza con `No se encontró un conjunto de datos válido en <carpeta>.` |
| Versión anterior al publicar | El generador original borra la versión anterior si puede; si no puede (en Windows, por ejemplo, porque una consulta la tiene abierta), lo anota en el log y una generación posterior la limpia. |
| Memoria del consultor original | Su verificación de aceptación exige para AC-C13 que el pico mayor no supere 3 veces el menor. Lo medido: contar `THEFT`, 14,5 MiB; contar un tipo raro, 7,5 MiB; archivo CSV con todos los robos, 16,6 MiB. |
| CanopyDB | Última versión publicada: 0.2.5, del 2025-11-22, licencia MIT. Sin cambios en el repositorio desde entonces. 3 issues abiertos: gestión de checkpoints, funciones de observabilidad e informe de optimización guiada por perfiles. Su integración continua pasa las pruebas en Windows, Linux y macOS. |
| CanopyDB en el equipo de referencia | Compila en Windows 11 sin herramientas de C++. Una prueba de humo escribe y lee 100.000 entradas sin errores. En Linux no se ha probado en este equipo; solo consta su integración continua. |
| Lectores en varios procesos | CanopyDB admite muchos lectores a la vez **dentro de un mismo proceso**. Entre procesos, abrir la base toma un bloqueo exclusivo: con un proceso leyendo, un segundo proceso que abre la misma base espera 5 segundos y falla. Comprobado en el equipo de referencia. |
| Lectura que escribe | Abrir una base para solo leer también abre sus archivos internos para escritura y puede recortar o reorganizar su registro de escritura anticipada. Si la base se cerró sin volcar ese registro al archivo de datos, cada apertura lo vuelve a aplicar entero en memoria y, a los 30 segundos, escribe en disco. CanopyDB no tiene un modo de apertura de solo lectura. |
| Memoria de CanopyDB | La caché de páginas crece bajo demanda hasta 1 GiB por defecto, el doble del límite de 512 MiB del generador. Su tamaño se puede configurar. |
| Compresión | CanopyDB solo comprime los valores grandes. Los registros de este conjunto ocupan unos 138 bytes de media (derivado de la decisión D4 del padre), así que con la configuración por defecto no se comprimirían. El generador original comprime con Snappy y tiene una opción para no hacerlo (decisión G2). |
| Documentación de CanopyDB | El tercer ejemplo de su README configura el tamaño de la caché pero abre el entorno sin esas opciones, así que el ajuste no tiene efecto (hallazgo H1). |

**Huecos de producto**: ninguno abierto en este documento. Los detalles de cada pieza están asignados a su PRD
hijo en la columna "Fija" de [Reparto en PRD hijos](#reparto-en-prd-hijos), y cada hijo los cierra con el
usuario antes de su ejecución.

## Actores

- **Operador del generador**: el del padre. Ejecuta el generador CanopyDB igual que el original.
- **Analista**: el del padre. Usa el consultor CanopyDB igual que el original y, con cualquiera de los dos
  consultores, puede ejecutar un lote de consultas en una sola ejecución (decisión K7).
- **Desarrollador**: ejecuta el banco de comparación, investiga los hallazgos y prepara las contribuciones.
- **Responsable del proyecto** (el usuario): aprueba una por una las acciones públicas fuera de este proyecto
  (decisión K4).
- **Mantenedor de CanopyDB** (externo): recibe los issues y PR. No hay ningún compromiso de que los acepte.

## Flujo

### A. Generación con CanopyDB

1. El operador ejecuta el generador CanopyDB con las mismas opciones que el original.
2. El generador valida, convierte y rechaza registros con las mismas reglas y mensajes que el original, y
   ordena por ID con el mismo presupuesto de memoria.
3. Guarda el conjunto en una base CanopyDB: la tabla principal y los cinco índices secundarios del padre
   (decisión D2), con el mismo contenido lógico que las SSTables. Son las mismas claves y los mismos valores,
   incluidos los valores de los índices de texto en minúsculas de la decisión C1 del consultor.
4. Antes de publicar, deja la base lista para leer: consultarla después no cambia su contenido ni añade
   archivos a la carpeta (decisión K6).
5. Publica de forma atómica, igual que el original: un conjunto incompleto nunca es visible y una generación
   fallida, cancelada o interrumpida no toca el conjunto anterior. La versión anterior se borra igual que en el
   original, también cuando un consultor CanopyDB la tiene abierta: si no se puede, se anota en el log y la
   limpia una generación posterior.
6. Muestra el mismo resumen final que el original, con las mismas líneas y el pico de memoria medido; la lista
   de archivos publicados muestra los que publique la forma de carpeta que fija `canopy-db-generador`.

### B. Consulta con CanopyDB

1. El analista ejecuta el consultor CanopyDB con las mismas opciones que el original y obtiene los mismos
   registros, con la misma salida y los mismos códigos de salida.
2. Mientras un consultor CanopyDB tiene un conjunto abierto, otro proceso que intente abrir el mismo conjunto
   espera. Si se libera dentro del tiempo de espera, continúa con normalidad; si no, se rechaza con un mensaje
   propio y el código de salida 1 (decisiones K6 y C6). Si el analista cancela durante la espera, termina como
   cualquier cancelación, con el código 130.
3. Un error del motor que no tiene equivalente en el original se muestra con el mensaje existente más cercano, y
   su detalle va solo al log o a la salida de errores (decisión K9).

### C. Lote de consultas

1. El analista prepara un archivo con una consulta por línea, escrita con las mismas opciones que una consulta
   suelta.
2. Ejecuta cualquiera de los dos consultores con ese archivo. El consultor abre el conjunto una vez, ejecuta las
   consultas una tras otra, en el orden del archivo, y termina (decisión K7). Todas las consultas del lote leen
   la misma versión del conjunto, la que abrió, aunque el generador publique otra mientras tanto.
3. La salida separa los resultados de cada consulta e informa del tiempo de cada una.
4. El pico de memoria no crece con el número de resultados ni con el número de consultas del lote (decisión K8).

### D. Comparación de rendimiento

1. El desarrollador ejecuta el banco de comparación sobre un CSV y una carpeta de trabajo.
2. El banco genera el conjunto con las dos parejas y ejecuta con los dos consultores el conjunto fijo de
   consultas, sueltas y en lote (decisión K3).
3. Escribe en el repositorio un informe con las cifras de las dos parejas, la versión de CanopyDB, la del
   proyecto y el equipo en que se midió. El informe no tiene umbrales de rendimiento: los únicos límites son los
   heredados del contrato y la tolerancia de repetición de AC-M10. Si una repetición se sale de esa tolerancia,
   el informe lo indica; qué más hace el banco lo fija `canopy-db-comparacion`.

### E. Hallazgos y contribución

1. Cada comportamiento de CanopyDB que se aparte de lo que su documentación promete o de lo que exige el
   contrato, incluido un problema de rendimiento que el desarrollador muestre con cifras, se anota en el
   registro de hallazgos con una reproducción mínima y la versión afectada (ver
   [Registro de hallazgos](#registro-de-hallazgos)).
2. Para cada hallazgo se prepara el issue y, si se encuentra la causa, un PR con la corrección y su prueba.
3. El responsable del proyecto aprueba por separado cada acción pública, es decir, cualquier escritura fuera de
   este proyecto: abrir un issue o un PR, comentar, responder a una revisión, cambiar un PR ya publicado, cerrar
   un issue, crear el fork o subir commits a él. Sin esa aprobación no se hace (decisión K4). Cada aprobación
   queda anotada en el registro.
4. Mientras una corrección propia no esté publicada en una versión de CanopyDB, las aplicaciones pueden usar
   un fork con esa corrección, fijado a un commit concreto que el registro anota.
5. Si CanopyDB no puede cumplir un criterio heredado (por ejemplo, los 512 MiB, la respuesta por ID en menos de
   1 segundo o la memoria de C7), el hijo sigue adelante y cierra: el criterio queda como "no cumplido por
   CanopyDB" en el log de su ejecución y en su informe de cierre, con su hallazgo en el registro (decisión K10).
   Un incumplimiento causado por el código del proyecto se corrige siempre.

## Reparto en PRD hijos

Cada hijo es un cambio con su propio PRD, su ejecución y su informe de cierre, y nombra este documento como
padre. Cada uno actualiza la ayuda, el README y las guías de lo que entrega. Se escriben y ejecutan en este
orden. El 1 y el 2 no dependen entre sí; el 3 depende del 1 y del 2; el 4, de los tres anteriores; el 5, del 1
y del 3. El módulo se cierra con el cierre del 5. *Anotación del 2026-10-22: sustituido por la decisión KR5 del
[PRD de robustez](canopy-db-robustez.prd.md): el 5 no espera al 4, y el módulo lo cierra el último hijo en
cerrar, `canopy-db-robustez` o `canopy-db-comparacion`, que vuelve a verificar AC-M12 y AC-M13.*

| Orden | PRD hijo | Entrega | Fija, entre otras cosas |
|---|---|---|---|
| 1 | `canopy-db-generador` | El generador CanopyDB; un verificador independiente que lee la base CanopyDB sin código del generador; su verificación de aceptación con las cifras de referencia; y el registro de hallazgos con H1–H4 y sus reproducciones. | El nombre de la aplicación; qué hace la opción sin compresión; la forma de la carpeta publicada y del manifiesto, incluidas la versión del esquema y la versión de CanopyDB (o el commit del fork) que anota; una forma del manifiesto con la que el consultor original, sin ningún cambio, rechaza un conjunto CanopyDB con uno de sus mensajes existentes, y cuál; qué pasa con `--overwrite` y sin él sobre una carpeta con un conjunto del otro motor; los restos de una generación interrumpida; qué mensaje existente usa cada error del motor en el generador (K9); cómo se comprueba el mismo contenido lógico de K2, también frente a las SSTables; el nombre y el lugar del registro de hallazgos. |
| 2 | `canopy-db-lote` | El lote de consultas (K7) en el consultor original. | Cómo se pide el lote; la forma del archivo (comillas, líneas vacías, comentarios, finales de línea, codificación); qué opciones se admiten en una línea y cuáles en la línea de comandos, y cómo se combinan (`--dataset`, `--format`, `--limit`, `--count`, `--output`); la salida de cada consulta y su separación; una línea mal escrita (rechazar el archivo entero antes de empezar o fallar en esa línea); un archivo vacío; la cancelación a mitad del lote; los errores y el código de salida. |
| 3 | `canopy-db-consultor` | El consultor CanopyDB, con el lote desde el principio, y su verificación de aceptación. | El nombre de la aplicación; las consultas del conjunto fijo, que reutiliza el banco; el tiempo de espera ante un conjunto bloqueado, su mensaje y si se avisa durante la espera (K6); qué mensaje existente usa cada error del motor en el consultor (K9); el rechazo de un conjunto del otro motor y de un conjunto escrito con otra versión de CanopyDB; cómo se limita la memoria del motor (K8). |
| 4 | `canopy-db-comparacion` | El banco de comparación y el primer informe. | El número de repeticiones; la medición en frío y en caliente; cómo se mide el pico de memoria (en lote, el del lote entero); las opciones de los dos generadores durante la medición, como la compresión o la carpeta temporal; la tolerancia entre dos ejecuciones; qué hace el banco si los conteos de las dos parejas no coinciden; los sistemas en que se ejecuta; la forma del informe. |
| 5 | `canopy-db-robustez` | Una búsqueda dirigida de bugs en CanopyDB y el cierre del módulo. | Los escenarios: interrupciones durante la generación, cancelar o matar un consultor o un lote a mitad (bloqueo liberado, siguiente apertura), procesos concurrentes, consultas durante la publicación, datos grandes y Linux; y cómo se anota el resultado de cada uno. |

Los hallazgos pueden salir de cualquiera de los cinco y siguen siempre el flujo E.

### Criterios y anotaciones por hijo

| Hijo | Verifica | Anota al cerrar |
|---|---|---|
| `canopy-db-generador` | AC-M1 (parte del generador), AC-M3, AC-M4 | PRD del generador, en G5 y AC-G2: el generador CanopyDB sigue K2. |
| `canopy-db-lote` | AC-M7 con el consultor original, AC-M9 | Decisión D6 del padre y PRD del consultor (flujo y D6): existe el lote (K7). |
| `canopy-db-consultor` | AC-M1 (parte del consultor), AC-M2, AC-M5, AC-M6, AC-M7 con el consultor CanopyDB, AC-M8 con los dos consultores | PRD del consultor, en la regla "el consultor nunca escribe en la carpeta del conjunto" y en C7: el consultor CanopyDB sigue K6 y K8. |
| `canopy-db-comparacion` | AC-M2 (con el banco), AC-M10 | — |
| `canopy-db-robustez` | AC-M11, AC-M12, AC-M13 y la comprobación final del módulo | Padre: existe el módulo CanopyDB, con enlace a este documento. *Anotación del 2026-10-22: esta anotación la hace el hijo que cierre el módulo (KR5 del [PRD de robustez](canopy-db-robustez.prd.md)).* |

## Validaciones y mensajes

Las aplicaciones CanopyDB aplican todas las validaciones y mensajes del padre, del generador, del consultor y
del PRD de soporte para Linux sin cambiar ni una palabra. Este documento solo añade reglas para todo el módulo;
los mensajes nuevos los fija el hijo que los introduce:

| Situación | Regla | Mensaje al usuario |
|---|---|---|
| El consultor original recibe un conjunto CanopyDB | Lo rechaza con un mensaje existente y el código 1, sin ningún cambio en el consultor original | El que fija `canopy-db-generador` |
| El consultor CanopyDB recibe un conjunto SSTable, de cualquier versión del esquema | Lo rechaza como un conjunto inválido, con el código 1 | `No se encontró un conjunto de datos válido en <carpeta>.` |
| Otro proceso tiene abierto el conjunto CanopyDB | Espera; si no se libera a tiempo, rechaza sin resultados, con el código 1 | El que fija `canopy-db-consultor` |
| Cancelación durante esa espera | Termina como cualquier cancelación, con el código 130 | `Consulta cancelada.` |
| Error del motor sin equivalente en el original | Se usa el mensaje existente más cercano; el detalle va solo al log o a la salida de errores (K9) | El que fija el hijo de cada aplicación para cada error |
| Acción pública fuera del proyecto sin aprobación | No se hace (K4) | — |

## Salida

- **Aplicaciones**: la misma salida pública que las originales (consola, salida de errores, CSV, JSON,
  archivo de salida, resumen y log del generador), con las excepciones del [contrato común](#objetivo-y-contexto)
  y la salida del lote, que fija `canopy-db-lote`.
- **Informe de comparación**: un documento en el repositorio por cada ejecución del banco que se quiera
  conservar. Como mínimo recoge, para cada pareja:
  - generación: duración, pico de memoria y tamaño en disco del conjunto publicado, con las opciones usadas;
  - cada consulta del conjunto fijo, suelta: tiempo (mediana de las repeticiones), pico de memoria y número de
    resultados;
  - el lote con todas las consultas fijas: tiempo de cada consulta, tiempo total y pico de memoria del lote;
  - la versión de CanopyDB (y el commit del fork, si se usa), su configuración, el commit del proyecto, el CSV
    usado (con su huella) y el equipo.

  Su forma exacta la fija `canopy-db-comparacion`.

### Registro de hallazgos

Un documento del repositorio, en español, común a todo el módulo, con una entrada por hallazgo. Los issues y PR
que se publiquen se redactan en inglés, el idioma del repositorio de CanopyDB.

| Campo | Contenido |
|---|---|
| Identificador | `H1`, `H2`… en orden de alta; nunca se reutiliza. |
| Fecha | Fecha de alta. |
| Versión de CanopyDB | La versión en que se reproduce. |
| Tipo | Bug, limitación, documentación o rendimiento. |
| Descripción y efecto | Qué pasa y cómo afecta al módulo. |
| Reproducción mínima | Pasos o programa que lo reproducen sin el resto del proyecto; en un hallazgo de documentación, la cita del texto afectado. |
| Estado | Detectado, issue publicado, PR publicado, aceptado, publicado en una versión o descartado (con el motivo). |
| Aprobaciones | Cada acción pública aprobada, con su fecha y su enlace. |
| Enlaces | Issue, PR y versión que lo corrige, si existen. |
| Fork | El commit del fork del que dependen las aplicaciones, si la corrección aún no está publicada. |

*Anotación del 2026-10-22: `canopy-db-robustez` añade el campo `Borrador del issue`, el enlace al borrador en
inglés del issue de cada hallazgo (decisión KR8 del [PRD de robustez](canopy-db-robustez.prd.md)).*

El registro empieza con lo comprobado al escribir este documento:

| Id | Tipo | Hallazgo |
|---|---|---|
| H1 | Documentación | El tercer ejemplo del README configura el tamaño de la caché pero abre el entorno sin esas opciones; el ajuste no tiene efecto. |
| H2 | Limitación | Dos procesos no pueden abrir a la vez la misma base: el segundo falla a los 5 segundos. |
| H3 | Limitación | No hay apertura de solo lectura: leer abre los archivos internos para escritura y puede modificarlos. |
| H4 | Documentación | El tamaño por defecto de la caché (1 GiB) y su crecimiento bajo demanda no aparecen en la portada de la documentación, y condicionan cualquier presupuesto de memoria. |

## Reutilización

- **Padre, generador, consultor, soporte para Linux y aceptación con cualquier CSV**: todas sus reglas,
  validaciones, mensajes, salidas y decisiones, salvo las excepciones del contrato común.
- **Contrato compartido del conjunto**: esquema del registro, codificación de clave y valor con tuplas de
  FoundationDB, los cinco índices, el puntero a la versión activa y el manifiesto. Las dos parejas guardan las
  mismas claves y los mismos valores; solo cambia dónde.
- **Lectura y validación del CSV, ordenación externa y publicación atómica** del generador original.
- **Resolución de consultas y formatos de salida** del consultor original (decisiones C1–C11).
- **Verificador independiente, filtrado directo del CSV y scripts de aceptación**: base para comprobar las
  aplicaciones CanopyDB con las mismas cifras. Sobre un conjunto CanopyDB, el verificador y las consultas de la
  verificación se ejecutan de uno en uno (K6).
- **Umbral de memoria de AC-C13**: el cociente máximo de 3 que ya usa la verificación del consultor original.
- **Medición del pico de memoria** en Windows y en Linux (decisión L3 del PRD de soporte para Linux).

## Fuera de alcance

- Sustituir o retirar las aplicaciones originales: siguen siendo la referencia.
- Cambiar las aplicaciones originales, salvo el lote de consultas en el consultor (K7).
- Optimizar las aplicaciones originales para la comparación.
- Un modo de solo lectura en CanopyDB o el acceso de varios procesos a la vez: se registran como hallazgos (H2 y
  H3) y se puede abrir un issue, pero desarrollarlos no forma parte del módulo.
- Conjuntos CanopyDB en carpetas o soportes de solo lectura: el consultor CanopyDB necesita poder escribir sus
  archivos internos (K6), así que los rechaza con el mensaje existente que corresponda (K9).
- Leer o convertir conjuntos entre motores: un consultor que lea los dos formatos o pasar un conjunto SSTable a
  CanopyDB sin el CSV.
- Garantizar que un conjunto sea legible con otra versión de CanopyDB: si no lo es, se regenera; cómo se rechaza
  lo fija `canopy-db-consultor`.
- Durabilidad frente a cortes de alimentación, como en el padre.
- Funciones que CanopyDB permitiría y el contrato no tiene: actualizaciones incrementales, borrados,
  transacciones visibles para el usuario, modo servidor y cualquier otra consulta que el consultor original no
  admita.
- En el lote: ejecutar consultas en paralelo, leer el lote de la entrada estándar y el modo interactivo.
- Comparar con otros motores (RocksDB, LMDB, redb, Fjall…).
- Mantener un fork de CanopyDB más allá de las correcciones propias pendientes de publicar.
- Cualquier compromiso sobre lo que haga el mantenedor de CanopyDB con los issues y PR.
- Publicar los informes de comparación fuera del repositorio.
- Ejecutar los scripts de aceptación en Linux, como en el original.
- macOS y otros sistemas, como en el PRD de soporte para Linux.

## Queda a criterio de la implementación

- La configuración interna de CanopyDB, incluido cómo se limita su memoria (K8), siempre que se cumplan el
  contrato y los límites de memoria. El informe de comparación indica la configuración usada.
- La versión de CanopyDB: la última publicada al empezar cada hijo, hoy la 0.2.5, o un fork con correcciones
  propias pendientes (flujo E).
- Cuándo un problema de rendimiento merece un hallazgo: lo decide el desarrollador y lo justifica con cifras.

## Criterios de aceptación

Son los criterios del módulo; cada hijo tiene además los suyos, y la tabla
[Criterios y anotaciones por hijo](#criterios-y-anotaciones-por-hijo) dice quién verifica cada uno. Las cifras
corresponden al CSV de referencia, con la misma salvedad que en el padre. Un criterio heredado que CanopyDB no
puede cumplir se trata según K10.

1. **AC-M1** — Dado el CSV de referencia, cuando se generan y consultan los conjuntos con las aplicaciones
   CanopyDB, entonces cumplen los criterios de aceptación del padre, del generador, del consultor y de
   aceptación con cualquier CSV con las mismas cifras, con estas sustituciones: AC-M3 sustituye a AC-G10; AC-M4,
   a AC-G2 y AC-L8; AC-M8, a AC-C13; y el rechazo de un conjunto SSTable de
   [Validaciones y mensajes](#validaciones-y-mensajes), a la segunda mitad de AC-C18 (conjunto con esquema 1).
2. **AC-M2** — Dado el mismo CSV generado con las dos parejas, cuando se ejecuta con los dos consultores cada
   consulta del conjunto fijo, entonces el conteo es igual y el conjunto de registros devueltos es el mismo,
   sin tener en cuenta el orden (C11). En las consultas con un límite, cada consultor devuelve tantos registros
   como el límite, todos del resultado sin límite.
3. **AC-M3** (decisión K2) — Dado el mismo CSV, cuando se genera dos veces con el generador CanopyDB, entonces
   los dos conjuntos tienen exactamente las mismas claves y los mismos valores en la tabla principal y en cada
   índice, aunque sus archivos no sean idénticos; y esas claves y valores son los mismos que los de las
   SSTables generadas con el original.
4. **AC-M4** (decisión K2) — Dado el CSV de referencia completo, cuando se ejecuta el generador CanopyDB,
   entonces el pico de memoria medido no supera 512 MiB (salvo K10) y el resumen final lo muestra. En Windows lo comprueba
   la verificación de aceptación; en Linux se comprueba a mano, como AC-L8, y se anota en el informe de cierre
   de `canopy-db-generador`.
5. **AC-M5** (decisión K6) — Dado un conjunto CanopyDB y un lote de consultas en curso sobre él, que dura más que
   el tiempo de espera que fija `canopy-db-consultor` (la prueba puede repetir consultas hasta conseguirlo), cuando un segundo consultor abre el mismo conjunto,
   entonces espera, se rechaza al acabar la espera con su mensaje y el código 1, y el lote termina con
   normalidad. Cuando el lote dura menos que la espera, el segundo consultor espera y responde con normalidad.
   Cuando el analista cancela el segundo durante la espera, termina con `Consulta cancelada.` y el código 130.
6. **AC-M6** (decisión K6) — Dado un conjunto CanopyDB recién publicado, cuando se ejecutan todas las consultas
   del conjunto fijo, sueltas y en lote, entonces la carpeta del conjunto tiene los mismos nombres de archivo que
   antes, sin ninguno nuevo, y el manifiesto es idéntico; y después, el contenido lógico (comprobado como en
   AC-M3) no ha cambiado. El contenido de los archivos internos del motor puede variar (K6).
7. **AC-M7** (decisión K7) — Dado un archivo con varias consultas, cuando se ejecuta en lote con cualquiera de
   los dos consultores, entonces cada consulta devuelve el mismo conteo y el mismo conjunto de registros que
   ejecutada sola; las que tienen un límite devuelven tantos registros como el límite, todos del resultado sin
   límite. Cuando el generador publica una versión nueva a mitad del lote, todas las consultas del lote
   devuelven resultados de la versión que abrió.
8. **AC-M8** (decisión K8) — Dados, con cada consultor por separado, un conteo de `THEFT`, un conteo de un tipo
   principal con pocos registros y un lote con todas las consultas del conjunto fijo, cuando se mide el pico de
   memoria de cada ejecución, entonces el mayor de los tres picos no supera 3 veces el menor. No se comparan los
   picos de un consultor con los del otro.
9. **AC-M9** — Dadas las aplicaciones originales, cuando se pasan sus pruebas y su verificación de aceptación,
   entonces siguen pasando todas, más las nuevas del lote.
10. **AC-M10** (decisión K3) — Dado el banco de comparación, cuando se ejecuta sobre el CSV de referencia,
    entonces produce un informe con todas las cifras de [Salida](#salida) para las dos parejas; y repetirlo
    sobre el mismo commit da cifras dentro de la tolerancia que fija `canopy-db-comparacion`.
11. **AC-M11** — Dado cada escenario que fija `canopy-db-robustez`, cuando se cierra ese hijo, entonces el
    escenario se ha ejecutado y su resultado, con o sin bug, queda anotado; cada bug encontrado tiene su entrada
    en el registro de hallazgos.
12. **AC-M12** (decisión K4) — Dado el registro de hallazgos al cerrar el módulo, cuando se compara con la lista
    de issues, PR y comentarios publicados en el repositorio de CanopyDB y en el fork, entonces cada hallazgo
    tiene su reproducción mínima, su versión y su estado, y cada acción pública de esa lista tiene anotada la
    aprobación del responsable del proyecto.
13. **AC-M13** — Dado el repositorio en Windows y en Linux (Ubuntu 24.04 sobre WSL2), cuando se pasan las
    pruebas, entonces pasan todas, incluidas las de las aplicaciones CanopyDB.

## Decisiones

Todas se tomaron con el usuario el 2026-10-20.

- **K1 — Aplicaciones nuevas, mismo contrato**: el módulo crea un generador y un consultor nuevos sobre
  CanopyDB, con las mismas opciones, mensajes, salidas y códigos de salida que los originales. Motivo: la
  comparación solo es justa si las dos parejas responden igual a lo mismo, y los originales siguen siendo la
  referencia. Se descartaron una opción para elegir el motor en las aplicaciones actuales (cambia su línea de
  comandos) y unas aplicaciones con contrato libre (las salidas dejarían de coincidir).
- **K2 — Garantías exigidas y medidas en el generador CanopyDB**: debe cumplir el pico de 512 MiB y la
  publicación atómica, porque son promesas al operador; si es CanopyDB quien lo impide, se aplica K10. Para él, la salida determinista de G5 pasa a ser "mismo
  contenido lógico": dos generaciones del mismo CSV guardan las mismas claves y valores, aunque los archivos
  difieran. El generador original conserva G5 tal cual. Motivo: un motor de páginas con espacio libre
  difícilmente produce archivos idénticos byte a byte, y CanopyDB no lo promete.
- **K3 — Banco de comparación repetible**: un banco ejecuta las dos parejas sobre el mismo CSV y el mismo equipo
  con un conjunto fijo de consultas (por ID, por número de caso, tipo principal `THEFT`, rango de fechas,
  combinación de condiciones, conteo y listado), sueltas y en lote, y escribe un informe en el repositorio.
  Motivo: se puede repetir con cada versión nueva de CanopyDB o tras corregir un bug. Se descartaron un informe
  único manual y una comparación limitada a la generación y la consulta por ID.
- **K4 — Contribución con aprobación**: cada hallazgo se documenta primero en el repositorio con una
  reproducción mínima. Después se prepara el issue y, si se encuentra la causa, un PR con la corrección y su
  prueba. Ninguna acción pública fuera de este proyecto (issue, PR, comentario, respuesta, cambio de un PR,
  cierre, fork o commits en el fork) se hace sin la aprobación del responsable del proyecto, una por una.
  Mientras una corrección propia no esté en una versión publicada, las aplicaciones pueden usar un fork fijado a
  un commit. Motivo: publicar es una acción externa e irreversible, y el repositorio lleva 11 meses sin cambios,
  así que un issue sin corrección probablemente quede sin resolver.
- **K5 — Reparto en cinco hijos**: generador, lote, consultor, comparación y robustez, en ese orden (ver
  [Reparto en PRD hijos](#reparto-en-prd-hijos)). Motivo: el módulo entero es demasiado grande para un solo
  cambio. El lote se añadió al decidir K7.
- **K6 — El consultor CanopyDB puede bloquear el conjunto**: mientras un consultor CanopyDB tiene un conjunto
  abierto, otro proceso que intente abrirlo espera y, si no se libera a tiempo, se rechaza. El consultor puede
  abrir para escritura los archivos internos del motor, pero el contenido consultable y el manifiesto no cambian
  nunca y no aparecen archivos nuevos; el generador publica la base lista para leer. Motivo: CanopyDB 0.2.5 solo
  admite lectores concurrentes dentro de un mismo proceso y no tiene apertura de solo lectura (H2 y H3).
  Desarrollar ese modo sería un cambio pequeño y aditivo en CanopyDB, pero depende de que el mantenedor lo
  acepte, y el usuario prefirió aceptar la limitación. Solo para el consultor CanopyDB, sustituye la regla del
  consultor original "el consultor nunca escribe en la carpeta del conjunto" y su posibilidad implícita de
  ejecutar varias consultas a la vez sobre el mismo conjunto; el consultor original no cambia.
- **K7 — Lote de consultas en los dos consultores**: los dos consultores pueden leer un archivo con una consulta
  por línea, escrita con las mismas opciones que una consulta suelta; abren el conjunto una vez, las ejecutan una
  tras otra y terminan. Motivo: si cada consulta es un proceso nuevo, la comparación mide sobre todo el arranque
  y la apertura en frío, no el motor; y con K6, el lote es la forma de hacer varias consultas seguidas sobre un
  conjunto CanopyDB. Sigue siendo una ejecución que arranca, responde y termina (D6 del padre), pero cambia el
  consultor original. Se descartaron el modo interactivo (el padre lo dejó para un cambio posterior y cuesta más
  de medir) y un lote solo dentro del banco (el analista no ganaría nada).
- **K8 — Memoria de los consultores**: la regla C7 del consultor original vale para los dos consultores, sueltos
  y en lote: el pico no crece con el número de resultados ni con el número de consultas del lote. Se comprueba
  con cada consultor por separado, con el cociente máximo de 3 entre el pico mayor y el menor que ya usa la
  verificación del consultor original. En el consultor CanopyDB eso exige limitar la memoria que usa el motor;
  cómo se limita queda a criterio de la implementación y figura en el informe de comparación. Motivo: el
  consultor CanopyDB tiene el mismo contrato, y con la configuración por defecto el pico crecería con lo que lee
  cada consulta. Se descartaron un tope fijo de 512 MiB, que sería una exigencia nueva para el consultor
  original, solo medir, que rompería C7, y un cociente menor que 2, que el consultor original no cumple hoy en
  su propia verificación de AC-C13 (2,21 entre el archivo CSV de todos los robos y el conteo de un tipo raro).
  El pico de un lote no se ha medido nunca con el consultor original: lo mide por primera vez
  `canopy-db-consultor` al verificar AC-M8, y si el original no cumple el cociente, es un fallo del proyecto y se
  corrige. *Anotación del 2026-10-21: en el lote, las consultas ya comprobadas del archivo se pueden guardar; lo
  que no crece es la memoria de cada consulta (decisión B7 del [PRD del lote de consultas](canopy-db-lote.prd.md)).*
- **K9 — Errores del motor**: un error de CanopyDB sin equivalente en las aplicaciones originales (por ejemplo,
  una base dañada o que no se puede abrir) se muestra con el mensaje existente más cercano, y su detalle va solo
  al log o a la salida de errores. Motivo: el contrato mantiene los mensajes sin cambiar una palabra, y el detalle
  técnico sirve para investigar hallazgos. La única excepción es el conjunto bloqueado de K6, que tiene mensaje
  propio.
- **K10 — Criterios que CanopyDB no puede cumplir**: si CanopyDB impide cumplir un criterio heredado, el hijo
  sigue adelante y cierra; el criterio queda como "no cumplido por CanopyDB" en el log de su ejecución y en su
  informe de cierre, y el hallazgo entra en el registro. Un incumplimiento causado por el código del proyecto se
  corrige siempre. Motivo: el módulo busca justamente los límites del motor, y bloquearlo por uno de ellos
  impediría terminar la comparación.

## Referencias

- Documento padre: [chicago-crime.prd.md](chicago-crime.prd.md)
- [PRD del generador](generador.prd.md), [PRD del consultor](consultor.prd.md),
  [PRD de soporte para Linux](soporte-linux.prd.md) y
  [PRD de aceptación con cualquier CSV](aceptacion-cualquier-csv.prd.md)
- CanopyDB: https://github.com/arthurprs/canopydb y https://docs.rs/canopydb

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 1b4d192c | 2026-10-20 11:20 UTC+01:00 | 3 h 38 min | 2 h 25 min | 1 h 13 min | 30 min | 43 min | 16 |

<!-- prd-time: [{"session": "1b4d192c-7b21-4a20-a982-50c7db1fafce", "start": "2026-10-20T10:20:49.911000+00:00", "wall": 13098, "discarded": 8708, "agent": 1803, "human": 2586, "interventions": 16, "idle_minutes": 10}] -->
