# Robustez de las aplicaciones CanopyDB y cierre del módulo — PRD

**Versión**: 1.0
**Fecha**: 2026-10-22
**Documento padre**: [Almacén de delitos de Chicago sobre CanopyDB — PRD](canopy-db.prd.md) (v1.0)

## Objetivo y contexto

Buscar bugs de CanopyDB, y de las aplicaciones del proyecto que lo usan, en las situaciones que una ejecución
normal no toca: procesos interrumpidos o matados a mitad, varios procesos sobre el mismo conjunto, consultas
mientras se publica una versión nueva, un volumen mayor que el de referencia y Linux. Es el quinto y último hijo
del módulo CanopyDB (decisión K5 del padre). El módulo lo cierra este hijo o `canopy-db-comparacion`, el que
cierre último (decisión KR5).

**Qué entrega**:

1. Un catálogo de escenarios, cada uno con lo que debe pasar (ver [Escenarios](#escenarios)).
2. Una forma repetible de ejecutarlos en Windows y en Linux, para volver a pasarlos con cada versión nueva de
   CanopyDB (decisión KR2), y una herramienta que fabrica el CSV grande de R13 (decisión KR3).
3. El registro de escenarios, con el resultado de cada uno en cada sistema (ver
   [Registro de escenarios](#registro-de-escenarios)), y una entrada en el registro de hallazgos por cada bug de
   CanopyDB que aparezca.
4. El borrador del issue de cada hallazgo, de H1 en adelante, listo para publicar (decisión KR8).
5. Su parte del cierre del módulo y, si cierra último, el cierre entero (ver
   [Cierre del módulo](#cierre-del-módulo)).

**Por qué**: el objetivo 2 del padre es detectar bugs de CanopyDB con las exigencias del contrato. Los hijos
anteriores comprueban el camino normal y unas pocas interrupciones; queda sin probar justo donde un motor joven
suele fallar: el bloqueo de un proceso que muere, el borrado de archivos que otro proceso tiene abiertos y las
carreras entre publicar y leer.

Este hijo no cambia las opciones, los mensajes ni las salidas de ninguna aplicación. Añade una regla al consultor
CanopyDB para una versión que se borra (decisión KR7) y, donde el contrato heredado no dice qué debe pasar en un
escenario, lo fija la fila del escenario (decisión KR1).

### Estado de partida

Verificado el 2026-10-22 sobre el repositorio y los informes de cierre de los hijos 1 a 3:

| Aspecto | Situación |
|---|---|
| Hijos del módulo | `canopy-db-generador`, `canopy-db-lote` y `canopy-db-consultor` cerrados. `canopy-db-comparacion` no tiene PRD todavía. |
| Versión de CanopyDB | 0.2.5 publicada, sin fork. |
| Cancelación del generador CanopyDB | Atiende Ctrl+C mientras lee el CSV, mientras carga la tabla principal y los índices, y justo antes del cambio de versión. Un Ctrl+C durante el volcado final de la base se atiende al terminar el volcado. Un Ctrl+C después del cambio de versión no cambia nada: la generación ya ha terminado bien. |
| Interrupción brusca del generador CanopyDB | La siguiente generación descarta los restos (KG6). La verificación de aceptación mata el generador en un solo momento, durante la carga en CanopyDB; ninguna prueba lo mata en las demás fases. |
| Generadores a la vez | Dos generadores sobre la misma carpeta, de cualquier motor y en cualquier orden, ya se comprueban en Windows y en Linux (AC-KG14). |
| Consultor CanopyDB matado o cancelado a mitad | Ninguna prueba mata o cancela un consultor o un lote a mitad de una consulta y comprueba que la siguiente apertura no espera. |
| Espera ante un conjunto abierto (KC2) | Probada con un solo consultor esperando; nunca con varios a la vez ni con el que tiene el conjunto matado mientras otro espera. |
| Versión que desaparece durante la espera | El consultor CanopyDB vuelve a leer el puntero y abre la versión nueva, pero ningún PRD lo dice y no se ha probado: en Windows no se puede provocar, porque el generador no puede borrar una versión abierta. |
| Borrar una versión abierta | En Windows el generador no puede y lo trata como fija el padre (flujo A, paso 5). En Linux el borrado funciona aunque un consultor la tenga abierta; qué le pasa a ese consultor no se ha probado ni lo dice ningún PRD. *Anotación del 2026-10-22: con un conjunto CanopyDB, el generador de Windows también borra una versión que un consultor tiene abierta, sin aviso (prueba de la ejecución, registro L12 de `odd/tasks/canopy-db-robustez.md`).* |
| Publicación durante un lote | AC-M7 se comprueba en Windows con un lote retenido: el JSON de cada consulta indica la versión que abrió el lote. No hay pruebas con consultores en paralelo durante varias publicaciones seguidas. |
| Datos grandes | Ninguna prueba supera los registros del CSV de referencia, y no hay herramienta para fabricar un CSV mayor. |
| Linux | Las pruebas pasan en Ubuntu 24.04 sobre WSL2. Las verificaciones de aceptación de los hijos anteriores solo se ejecutan en Windows (fuera de alcance en el padre). Las guías de Linux piden el código y los datos en el sistema de archivos de Linux. |
| Registro de hallazgos | H1 a H5, todos "Detectado", sin ninguna acción pública. |
| PRD raíz | No menciona todavía el módulo CanopyDB. |

**Huecos de producto**: ninguno abierto.

## Actores

- **Desarrollador**: ejecuta los escenarios, anota sus resultados, corrige los bugs del proyecto, registra los
  de CanopyDB y redacta sus issues.
- **Responsable del proyecto** (el usuario): el del padre; además, da el módulo por cerrado.
- **Analista**: solo nota KR7: un consultor CanopyDB cuya versión se borra responde en lugar de fallar. El
  operador no ve ningún cambio.

## Flujo

1. El desarrollador ejecuta los escenarios en Windows 11 y en Ubuntu 24.04 sobre WSL2. En Linux, el código y los
   datos están en el sistema de archivos de Linux, como piden sus guías, no en un disco de Windows montado.
2. La ejecución informa, por cada escenario y variante, de su estado y de lo que observó (ver [Salida](#salida)).
3. El desarrollador anota cada resultado en el registro de escenarios.
4. Un incumplimiento se trata según K10 del padre: si es del proyecto, se corrige y el escenario se repite hasta
   que pasa; si es de CanopyDB, entra en el registro de hallazgos con su reproducción mínima y el borrador de su
   issue.
5. Si una corrección cambia el código de una aplicación, se vuelve a pasar la verificación de aceptación de esa
   aplicación antes de cerrar.
6. Cuando todos los escenarios están anotados, el desarrollador hace su parte del cierre del módulo.

## Escenarios

### Reglas comunes

- Los escenarios R1 a R12 usan el conjunto generado del CSV de referencia; R13, el CSV grande.
- **Matar** es terminar el proceso de la aplicación sin darle ocasión de limpiar: `TerminateProcess` en Windows,
  `SIGKILL` en Linux. **Ctrl+C** es la señal de cancelación que la aplicación atiende.
- Una señal cuenta solo si llega en la fase que nombra la variante; si la aplicación ya había pasado esa fase o
  había terminado, esa ejecución no cuenta y se repite, hasta cinco intentos. Si ninguno lo consigue, la variante
  queda como no ejecutada.
- "El siguiente" proceso se lanza cuando el anterior ya ha terminado, y debe abrir el conjunto sin el aviso de
  espera de KC2.
- "Una ejecución limpia" es la misma consulta ejecutada sola, sin interrupciones ni otros procesos; "el mismo
  resultado" es el mismo número de resultados y, en los listados, el mismo conjunto de registros.
- "La versión" de una consulta es la que indica su salida JSON, como en AC-M7.
- Los procesos que no se matan terminan con el código que fija su fila. Al final de R1 a R12, el contenido lógico
  del conjunto publicado es el de una generación limpia (comprobado como en AC-M3).
- Los escenarios con carreras (R8, R9, R11 y R12) se repiten al menos cinco veces cada variante. Una variante
  falla si falla una sola repetición, y el registro anota cuántas fallaron.

### Generación

| Id | Escenario y variantes | Lo esperado |
|---|---|---|
| R1 | Matar el generador CanopyDB en cada fase: lectura y ordenación del CSV, carga de la tabla principal, escritura de los índices, volcado final de la base, escritura del manifiesto, entre la escritura del puntero temporal y el cambio de versión, y después del cambio de versión, antes de borrar la versión anterior. Cada fase con un conjunto anterior publicado y sin él. | Antes del cambio de versión: el conjunto anterior sigue publicado y una consulta da la versión anterior; sin conjunto anterior, el consultor CanopyDB rechaza la carpeta con `No se encontró un conjunto de datos válido en <carpeta>.` y el código 1. Después del cambio de versión: una consulta da la versión nueva. En todos los casos, la siguiente generación (con `--overwrite` si hay conjunto) no informa de otra generación en curso, muestra los mensajes de KG6 de lo que quedó (restos de versión, carpetas temporales o los dos, y ninguno de lo que no quedó), termina con 0 y publica el contenido de una generación limpia. |
| R2 | Ctrl+C al generador CanopyDB en cada fase: lectura y ordenación del CSV, carga de la tabla principal, escritura de los índices, volcado final de la base, y después del cambio de versión. Cada fase con un conjunto anterior publicado. | Hasta el volcado incluido: termina con 130 y el mensaje de cancelación, sin restos, y una consulta da la versión anterior; en el volcado, termina al acabar el volcado. Después del cambio de versión: termina con 0, con su resumen final y sin mensaje de cancelación, y una consulta da la versión nueva. El registro anota cuánto tardó en terminar desde el Ctrl+C. |

### Consultor CanopyDB

| Id | Escenario y variantes | Lo esperado |
|---|---|---|
| R3 | Matar un consultor CanopyDB: durante la apertura de la base; durante el conteo F3; durante el listado F11 a la salida estándar; durante el listado F11 con archivo de salida, con y sin un archivo previo con ese nombre. | El siguiente consultor da el mismo resultado que una ejecución limpia. La carpeta del conjunto cumple AC-M6. Con archivo de salida, el archivo con el nombre pedido es el previo, intacto, o no existe si no había ninguno: nunca uno a medias (C8). Un temporal que quede junto a él se anota y no es un fallo, porque un proceso matado no puede borrarlo. |
| R4 | Matar un lote con el conjunto fijo en el que F10 y F11 escriben en archivos de salida: después de terminar F10 y durante F11. | Lo mismo que R3; además, el archivo de F10 está completo y es el de una ejecución limpia. |
| R5 | Ctrl+C a mitad de F11, fuera de la espera: a un consultor suelto y a un lote. | Termina con `Consulta cancelada.` y 130 en menos de 1 s, sin archivo de salida a medias ni temporales; el siguiente consultor abre sin el aviso de espera. |
| R6 | Un lote tiene el conjunto abierto y termina entre 10 y 20 s después de que espere un segundo consultor; al segundo se le cancela con Ctrl+C o se le mata. Después llega un tercero. | El cancelado termina como fija KC2 (`Consulta cancelada.`, 130, menos de 1 s). El lote termina con 0 y el mismo resultado que una ejecución limpia. El tercero muestra un solo aviso y, cuando el lote termina, responde con 0 y el resultado de una ejecución limpia. |
| R7 | Un lote tiene el conjunto abierto y un segundo consultor espera; se mata el lote. | El segundo abre en menos de 1 s desde la muerte del lote, sin un segundo aviso, y responde con 0 y el resultado de una ejecución limpia. |

### Procesos concurrentes y publicación

| Id | Escenario y variantes | Lo esperado |
|---|---|---|
| R8 | Cuatro consultores con el conteo F3, lanzados a la vez, esperan a que un lote libere el conjunto. Variante A: el lote termina entre 5 y 15 s después del último aviso. Variante B: el lote dura al menos 45 s más. | A: cada uno muestra un solo aviso y responde con 0 y el resultado de una ejecución limpia; un rechazo es un fallo, porque las cuatro consultas caben en la espera. B: cada uno muestra un solo aviso y se rechaza con el mensaje de KC2 y el código 1, entre 30 y 35 s después de su aviso. En las dos, el registro anota el orden en que terminaron; no hay orden garantizado (KR4). |
| R9 | Cuatro consultores en paralelo, cada uno repitiendo F3 y F2 sin pausa, mientras el generador CanopyDB publica diez versiones seguidas del CSV de referencia con `--overwrite`. Después, una generación más sin consultores. | Cada generación termina con 0 y publica; en Windows, la que no puede borrar una versión anterior abierta lo trata como fija el padre (flujo A, paso 5). Cada consulta termina con 0 y el resultado de una ejecución limpia, con la versión que abrió, también si la versión que iba a abrir se borra antes (KR7); o, solo si otro consultor tuvo el conjunto abierto todo ese tiempo, se rechaza con el mensaje de KC2 y el código 1 entre 30 y 35 s después de su aviso (KR4). Tras la generación final no queda ninguna versión anterior. |
| R10 | Matar el generador CanopyDB durante la carga de la tabla principal mientras un lote con el conjunto fijo lee el conjunto anterior. | El lote termina con 0, con el resultado de una ejecución limpia y la versión anterior; después, lo esperado de R1 para esa fase con conjunto anterior. |
| R11 | Solo en Linux: el generador publica y borra la versión anterior mientras la tiene abierta un consultor suelto con F11, y en otra variante un lote con el conjunto fijo. | El consultor o el lote termina con 0, con el resultado de una ejecución limpia, y todas sus consultas dan la versión que abrió (KR7). Cuando termina, la versión borrada no deja nada en la carpeta del conjunto. |
| R12 | Solo en Linux: un lote tiene abierta una versión y un consultor con F2 espera; el generador publica otra y borra la que esperaba antes de que el lote termine. | El lote, como R11. El que espera muestra un solo aviso y, en cuanto la versión que esperaba desaparece, abre la nueva sin esperar al lote y responde con 0, el resultado de una ejecución limpia y la versión nueva, dentro de los 30 s desde su aviso (KR7). |

### Datos grandes

| Id | Escenario y variantes | Lo esperado |
|---|---|---|
| R13 | Solo en Windows: generar el CSV grande con el generador CanopyDB y ejecutar con el consultor CanopyDB el conjunto fijo y las tres ejecuciones de AC-M8. | El generador termina con 0, con `<registros del CSV grande> registros válidos, 0 rechazados`, y su pico de memoria, medido como en su verificación de aceptación, no supera 512 MiB (o K10). Cada consulta del conjunto fijo devuelve el mismo número de resultados que el filtrado directo del CSV grande; en F10, 1.000. Las tres ejecuciones de AC-M8 cumplen su cociente. |

**CSV grande** (KR3): la herramienta lo fabrica a partir del CSV de referencia, con su cabecera una sola vez, el
original y después dos copias de todos sus registros. En cada copia, cada registro recibe un ID nuevo, numérico y
mayor que cualquier ID del original, y un número de caso nuevo con las dos mismas letras iniciales y el mismo
formato, que no aparecen en el original ni en la otra copia; el resto de campos no cambia. Así F1 y F2 devuelven
lo mismo que en el CSV de referencia, F9 el triple, y las demás consultas el triple. Antes de empezar, el
escenario calcula el espacio libre que necesita en el disco de la carpeta de trabajo; si no lo hay, no empieza,
dice cuánto necesita y queda como no ejecutado.

### Sistemas

Todos los escenarios se ejecutan en los dos sistemas, salvo R11 y R12, que solo se pueden provocar en Linux, y
R13, que solo se ejecuta en Windows (KR3). En el otro sistema, esos escenarios salen como no aplicables.
*Anotación del 2026-10-22: R11 y R12 también se provocan en Windows (ver la anotación de "Borrar una versión
abierta" en el estado de partida), así que se ejecutan en los dos sistemas; solo R13 queda limitado a Windows.*

## Registro de escenarios

Un documento del repositorio, en español, junto al registro de hallazgos. Cada vez que se pasan los escenarios se
añaden filas nuevas; las anteriores no se cambian, para comparar versiones de CanopyDB. Una fila por escenario,
variante y sistema, con las variantes que nombra la fila del escenario.

| Campo | Contenido |
|---|---|
| Escenario | `R1`… `R13` y su variante. |
| Sistema | `Windows` o `Linux`. |
| Fecha | Fecha de la ejecución, `AAAA-MM-DD`. |
| Versión de CanopyDB | La versión y, si se usa, el commit del fork. |
| Commit del proyecto | El commit con que se ejecutó. |
| Resultado | `Sin bug` (estado cumplido); `Bug del proyecto` con el commit que lo corrige (estado no cumplido por el proyecto; la fila de la repetición posterior dice `Sin bug`); `Hallazgo` con los identificadores `Hn` (estado `NOT MET`, o no cumplido por un hallazgo nuevo); o `No aplica en este sistema`. Una variante no ejecutada no se anota: se repite hasta ejecutarse. |
| Observado | Códigos de salida, mensajes, tiempos, orden y repeticiones; si falló, en qué se apartó de lo esperado; si un escenario asociado a un hallazgo se cumplió, que el hallazgo puede estar resuelto. |

## Cierre del módulo

El módulo lo cierra el último de sus hijos en cerrar, este o `canopy-db-comparacion` (KR5). Este hijo hace
siempre los pasos 1, 2 y 5; los pasos 3 y 4 los hace el hijo que cierre último, que además vuelve a verificar
AC-M12 y AC-M13 en su cierre.

1. Se verifican AC-M11, AC-M12 y AC-M13 del padre.
2. Anotaciones de este hijo, en cada PRD en español y en su traducción:
   - Padre, en "Reparto en PRD hijos" (el orden de ejecución y el cierre del módulo) y en su fila de
     "Criterios y anotaciones por hijo": KR5.
   - Padre, en "Registro de hallazgos": el campo nuevo `Borrador del issue` (KR8).
   - PRD de `canopy-db-consultor`: en su flujo, la regla de KR7; en "Fuera de alcance", que los escenarios que
     dejaba para este hijo están en este documento.
3. El informe de cierre del hijo que cierra el módulo incluye una tabla con una fila por criterio, de AC-M1 a
   AC-M13: los hijos que lo verifican, según la tabla "Criterios y anotaciones por hijo" del padre, y el
   resultado de cada uno (`Cumplido`, o `No cumplido por CanopyDB` con su hallazgo). El módulo no se cierra si a
   un criterio le falta el resultado de alguno de sus hijos.
4. Anotaciones del cierre del módulo, en el PRD en español y en su traducción:
   - PRD raíz: *Anotación del <fecha>: el [módulo CanopyDB](canopy-db.prd.md) añade un generador y un consultor
     que guardan el conjunto en CanopyDB, con el mismo contrato.*
   - Padre: el módulo queda cerrado, con la fecha.
5. El README y las guías de Linux dicen cómo ejecutar los escenarios, cómo fabricar el CSV grande y dónde está
   el registro de escenarios.

## Validaciones y mensajes

Este hijo no añade validaciones ni mensajes a ninguna aplicación; todas conservan los de su ascendencia. KR7 no
añade mensajes: el consultor responde en lugar de rechazar.

## Salida

- **Registro de escenarios**: ver [Registro de escenarios](#registro-de-escenarios).
- **Registro de hallazgos**: una entrada nueva por cada bug, limitación o problema de documentación de CanopyDB
  que aparezca, con la forma del padre y el campo nuevo `Borrador del issue`, que enlaza el borrador (KR8).
- **Ejecución de los escenarios**: para cada escenario y variante, su identificador, su estado y lo observado
  (KR6). Los estados son:
  - cumplido;
  - no cumplido;
  - `NOT MET` con su hallazgo: la variante está asociada a un hallazgo conocido y lo único que no se cumple es
    lo que ese hallazgo describe; cualquier otra diferencia la deja como no cumplida;
  - no ejecutado, con el motivo (la señal no llegó en su fase tras cinco intentos, o falta espacio en disco);
  - no aplicable en este sistema.

  Una variante asociada que se cumple sale como cumplida, con un aviso de que su hallazgo puede estar resuelto.
  Una asociación que nombra un escenario, una variante o un hallazgo que no existen hace que la ejecución no
  empiece y termine con el código 1. El código de salida es 1 si alguna variante queda no cumplida o no
  ejecutada, y 0 en otro caso. El formato exacto queda a criterio de la implementación.
- **Herramienta del CSV grande**: escribe el CSV grande y dice cuántos registros tiene.
- Las aplicaciones no cambian su salida.

## Reutilización

- El contrato de la ascendencia y de los hijos 1 a 3: mensajes, códigos, cancelación, restos (KG6), dos
  generadores a la vez (AC-KG14), espera (KC2), errores del motor (KG7 y KC5) y archivo de salida (C8).
- El conjunto fijo de consultas (KC7) en R3 a R13.
- La comprobación del mismo contenido lógico del verificador CanopyDB (KG8).
- El filtrado directo del CSV y la medición del pico de memoria de las verificaciones de aceptación, para R13.
- La asociación de criterios a hallazgos de KG10 y KC8 (`-NotMetByCanopyDb`), para KR6.
- Las reproducciones mínimas del registro de hallazgos como modelo para las nuevas.

## Fuera de alcance

- Cambiar las opciones, mensajes o salidas de cualquier aplicación, incluido el tiempo de espera de KC2.
- Escenarios sobre las aplicaciones originales, incluido matar o cancelar el lote del consultor original.
- Generadores a la vez sobre la misma carpeta o con la misma carpeta temporal: ya los cubren AC-KG14 y las
  pruebas de los hijos anteriores.
- El verificador CanopyDB en cualquier escenario.
- Otras señales y formas de terminar: `SIGTERM`, `SIGHUP`, Ctrl+Break y cerrar la consola.
- Salida estándar cerrada a mitad de un listado, falta de memoria o de descriptores de archivo, rutas con espacios
  o caracteres no ASCII, y programas ajenos (antivirus, indexadores) que abren los archivos del conjunto.
- Cortes de alimentación y durabilidad tras un apagado del equipo, como en el padre.
- Disco lleno real, carpetas de red, discos de Windows montados en Linux, soportes de solo lectura y equipos con
  otro orden de bytes.
- Bases dañadas a propósito o escritas con otra versión de CanopyDB (KC4).
- Desarrollar en CanopyDB un modo de solo lectura o el acceso de varios procesos (H2 y H3).
- Medir o comparar el rendimiento: es de `canopy-db-comparacion`.
- R13 en Linux y un CSV grande de otro tamaño (KR3).
- Ejecutar en Linux las verificaciones de aceptación de los hijos anteriores.
- macOS y otros sistemas.
- Del flujo E del padre, todo salvo el alta en el registro y los borradores de los issues (KR8): publicar issues,
  preparar o publicar PR, comentar, crear el fork o depender de él.

## Queda a criterio de la implementación

- Cómo se ejecutan los escenarios (pruebas automáticas, scripts o un programa propio), si alguno forma parte de
  `cargo test`, cómo se le indican las asociaciones de KR6 y el formato de su salida, siempre que cumplan KR2 y
  [Salida](#salida).
- Cómo se consigue que una señal llegue en cada fase, siempre que no añada ninguna opción visible a las
  aplicaciones ni cambie una ejecución normal.
- Los nombres de las variantes, del registro de escenarios y de la herramienta del CSV grande, y dónde viven los
  borradores de los issues.
- Cómo calcula R13 el espacio que necesita.

## Criterios de aceptación

Las cifras corresponden al CSV de referencia, con la misma salvedad que en el padre. Un criterio heredado que
CanopyDB no puede cumplir se trata según K10.

1. **AC-KR1** — Dado cada escenario de R1 a R13 y cada variante, cuando se ejecuta en cada sistema que le
   corresponde, entonces se cumple lo esperado de su fila, o queda como `NOT MET` por un hallazgo de CanopyDB.
2. **AC-KR2** (KR2) — Dado el repositorio en cada sistema, cuando se ejecutan los escenarios, entonces cada
   variante informa de su identificador, su estado y lo observado, sin pasos a mano; R11 y R12 salen como no
   aplicables en Windows, y R13 en Linux.
3. **AC-KR3** (KR6) — Dada una asociación de una variante con un hallazgo, cuando la variante no cumple solo lo
   que el hallazgo describe, entonces sale como `NOT MET` y el código es 0 si todo lo demás se cumple; cuando
   además no cumple otra cosa, sale como no cumplida y el código es 1; cuando se cumple, sale como cumplida con
   el aviso. Dada una asociación con un escenario, variante o hallazgo inexistente, la ejecución no empieza y el
   código es 1. Dada una variante no ejecutada, el código es 1.
4. **AC-KR4** (KR7) — Dado un consultor CanopyDB suelto o en lote que ya abrió una versión, cuando el generador
   la borra, entonces todas sus consultas dan esa versión y el resultado de una ejecución limpia (R11). Dado uno
   que aún no la había abierto, cuando la versión desaparece, entonces abre la nueva con un solo aviso y dentro
   de los 30 s de KC2 (R9 y R12).
5. **AC-KR5** (KR3) — Dado el CSV de referencia, cuando la herramienta fabrica el CSV grande, entonces tiene una
   cabecera, el triple de registros, ningún ID ni número de caso repetido salvo los que ya se repetían en el
   original, y el generador CanopyDB lo acepta sin rechazos.
6. **AC-KR6** — Dado el registro de escenarios al cerrar, cuando se revisa, entonces tiene una fila con todos sus
   campos por cada escenario, variante y sistema, con un resultado distinto de no ejecutado; y cada hallazgo que
   nombra existe en el registro de hallazgos con su reproducción mínima.
7. **AC-KR7** (KR8) — Dado el registro de hallazgos al cerrar, cuando se revisa, entonces cada hallazgo enlaza en
   `Borrador del issue` un borrador en inglés con título, descripción, reproducción mínima y versión; y no hay
   ninguna acción pública nueva en el repositorio de CanopyDB ni en un fork.
8. **AC-KR8** (KR5) — Dado el cierre de este hijo, cuando se revisa, entonces se cumplen los pasos 1, 2 y 5 de
   [Cierre del módulo](#cierre-del-módulo); y, si `canopy-db-comparacion` ya está cerrado, también los pasos 3 y
   4.

AC-M11, AC-M12 y AC-M13 son los del padre.

## Decisiones

KR1 y KR2 se derivan de la ascendencia; KR3 a KR8 se tomaron con el usuario, que también confirmó las reglas
comunes de los escenarios y R7. Todas son del 2026-10-22.

- **KR1 — Lo esperado de cada escenario**: sale del contrato heredado. Donde el contrato no dice qué pasa, la
  fila del escenario lo fija: el código y el resumen tras un Ctrl+C después del cambio de versión (R2), un
  temporal que deja un consultor matado (R3), la cancelación en menos de 1 s también a mitad de una consulta
  (R5) y la apertura en menos de 1 s tras matar al que tenía el conjunto (R7). Un incumplimiento de lo esperado
  es un bug del proyecto o un hallazgo de CanopyDB.
- **KR2 — Escenarios repetibles**: se ejecutan en cada sistema sin pasos a mano, para volver a pasarlos con una
  versión nueva de CanopyDB o tras corregir un hallazgo. Motivo: el padre pide ayudar a que CanopyDB madure, y un
  escenario que solo se ejecutó una vez no dice nada de la versión siguiente.
- **KR3 — Datos grandes**: el triple del CSV de referencia (unos 26 millones de registros), fabricado por una
  herramienta de este hijo, y solo en Windows. Motivo: basta para ver si la memoria del generador y del
  consultor crece con el volumen, con unos 50 GB libres; en Linux, el pico del generador ya se midió a mano con
  el CSV de referencia (AC-M4). Se descartaron el doble (menos margen para ver crecer la memoria), el quíntuple
  (unos 80 GB y mucho más tiempo) y repetirlo en Linux.
- **KR4 — Varios consultores en espera**: no hay orden de entrada ni reparto justo. Un consultor que se rechaza a
  los 30 s cumple KC2 si en ese tiempo otro proceso tuvo el conjunto abierto, aunque quedara libre por momentos.
  Motivo: KC2 promete una espera, no un turno, y un turno sería un mecanismo nuevo. Se descartó tratar como bug
  al consultor que pierde frente a otros.
- **KR5 — Cierre del módulo**: lo cierra el último hijo en cerrar, este o `canopy-db-comparacion`; ese hijo
  vuelve a verificar AC-M12 y AC-M13, hace la tabla de AC-M1 a AC-M13 y las anotaciones del cierre. Sustituye,
  en el padre, el orden de ejecución entre los hijos 4 y 5, la frase "El módulo se cierra con el cierre del 5"
  y la anotación del PRD raíz en la fila de este hijo de "Criterios y anotaciones por hijo"; se anota el padre.
  Motivo: AC-M2 con el banco y AC-M10 solo los verifica `canopy-db-comparacion`, que aún no tiene PRD, y este
  hijo no depende de él. Se descartaron esperar a `canopy-db-comparacion` para cerrar este hijo y cerrar el
  módulo con esos dos criterios pendientes.
- **KR6 — Hallazgos conocidos en la ejecución de los escenarios**: se le indica qué variantes están asociadas a
  qué hallazgo, con el patrón de `-NotMetByCanopyDb` de KG10 y KC8: una variante asociada sale como `NOT MET` solo
  si lo que no se cumple es lo que describe su hallazgo, y no hace fallar la ejecución. Amplía ese patrón con el
  aviso de un hallazgo que puede estar resuelto. Motivo: una ejecución que no puede quedar en verde mientras
  CanopyDB no corrija un hallazgo deja de avisar de los fallos nuevos. Se descartó que cualquier fallo dé el
  código 1.
- **KR7 — Consultor cuya versión se borra**: un consultor CanopyDB que ya abrió una versión termina con los
  resultados de esa versión aunque el generador la borre, como fija AC-M7 para una versión nueva publicada. Uno
  que aún no la ha abierto, esté esperando o no, vuelve a leer el puntero en cuanto la versión desaparece y
  responde con la nueva, sin un segundo aviso y sin pasar de los 30 s de KC2 desde su aviso. Vale en los dos
  sistemas. Si el puntero no ha cambiado y la versión está incompleta, sigue valiendo el rechazo de AC-KC4. Es
  una regla nueva del consultor CanopyDB que no cambia sus opciones, mensajes ni salidas; se anota su PRD.
  Motivo: en Linux el borrado de una versión abierta funciona, y sin regla un rechazo cualquiera pasaría por
  correcto. Se descartó aceptar un rechazo con un mensaje existente y el código 1.
- **KR8 — Contribución a CanopyDB**: este hijo deja redactado en inglés el issue de cada hallazgo (título,
  descripción, reproducción mínima y versión), pero no publica nada ni prepara PR. Cada entrada del registro de
  hallazgos enlaza su borrador en un campo nuevo, `Borrador del issue`, que amplía la forma del padre. Motivo:
  publicar es una acción externa que el responsable aprueba una por una (K4), y con los borradores listos esa
  aprobación es inmediata. Se descartaron publicar con aprobación dentro de este hijo y limitarse al alta en el
  registro.

## Referencias

- Documento padre: [canopy-db.prd.md](canopy-db.prd.md); raíz: [chicago-crime.prd.md](chicago-crime.prd.md)
- Hermanos: [canopy-db-generador.prd.md](canopy-db-generador.prd.md),
  [canopy-db-lote.prd.md](canopy-db-lote.prd.md), [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)
- [PRD del generador](generador.prd.md), [PRD del consultor](consultor.prd.md),
  [PRD de soporte para Linux](soporte-linux.prd.md),
  [PRD de aceptación con cualquier CSV](aceptacion-cualquier-csv.prd.md)
- [Registro de hallazgos](../../canopy-db/hallazgos.md)

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 6174e156 | 2026-10-22 10:32 UTC+01:00 | 1 h 3 min | 14 min | 49 min | 25 min | 24 min | 7 |

<!-- prd-time: [{"session": "6174e156-e93a-4ad3-88e8-cda74c462cb4", "start": "2026-10-22T09:32:36.433000+00:00", "wall": 3774, "discarded": 858, "agent": 1489, "human": 1425, "interventions": 7, "idle_minutes": 10}] -->
