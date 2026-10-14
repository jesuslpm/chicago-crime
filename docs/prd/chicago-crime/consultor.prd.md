# Consultor del conjunto de datos — PRD

**Versión**: 1.1
**Fecha**: 2026-10-13
**Documento padre**: [Almacén consultable de delitos de Chicago sobre SSTable — PRD](chicago-crime.prd.md) (v1.2)

## Objetivo y contexto

Entregar el **consultor**, la segunda de las dos aplicaciones del documento padre. El analista lo usa para
buscar delitos en un conjunto de datos ya publicado por el generador y para contar resultados, sin cargar el
conjunto entero en memoria y sin recorrer el CSV.

Este documento no repite las reglas del padre, que siguen vigentes: flujo B de consulta, validaciones del
consultor, reglas de los campos y decisiones D1–D6. Tampoco repite lo que ya fijó el
[generador](generador.prd.md) sobre el formato publicado. Solo añade lo que esta entrega necesita y el padre
deja abierto:

- cómo se expresan las condiciones y los valores, incluido "sin valor";
- qué campos admiten condiciones y con qué coste;
- qué muestra cada formato de salida, cómo se guarda en un archivo y dónde aparecen los mensajes;
- los códigos de salida y el rendimiento esperado;
- un cambio en el generador: los índices secundarios de texto se guardan en minúsculas para que las
  búsquedas no distingan mayúsculas (decisión C1);
- cómo se verifican los criterios del padre que quedaron pendientes (AC6, AC7, AC10 y AC11).

### Estado de partida

Verificado el 2026-10-13 sobre el repositorio:

| Aspecto | Situación |
|---|---|
| Producto | Documento padre v1.2 cerrado (D1–D6). PRD del generador v1.0 cerrado (G1–G5). |
| Generador | Implementado y aceptado (AC-G1 a AC-G10). Publica cada conjunto en una carpeta de versión y señala la activa con un puntero que cambia en un solo paso, de modo que un lector siempre encuentra un conjunto completo. |
| Contrato del conjunto | Existe una biblioteca compartida que ya describe el esquema del registro, la codificación de clave y valor, los cinco índices, el puntero a la versión activa y el manifiesto. Se escribió pensando también en el consultor. |
| Índices publicados | Fecha, tipo principal, distrito, área comunitaria y número de caso. Cada entrada es "valor (o sin valor) + ID", ordenada por valor y, dentro de cada valor, por ID creciente. La fecha se guarda como hora civil local de Chicago (D3). |
| Verificador | Existe un verificador independiente que lee las tablas sin código del generador y compara con el CSV. No ejecuta consultas. |
| Consultor | No existe. El README lo marca como planificado. |
| Pendiente del padre | AC6, AC7, AC10 y AC11 quedaron fuera del generador y pasan a esta entrega. |

## Actores

- **Analista**: el definido en el documento padre. Es el único actor de esta entrega.

## Flujo

Sigue el flujo B del documento padre. Esta entrega concreta los puntos siguientes:

1. El analista ejecuta el consultor indicando la carpeta de salida del generador (la que contiene el puntero
   a la versión activa) y una o varias condiciones. Cada ejecución responde una consulta y termina (D6).
2. El consultor localiza la versión activa, lee su manifiesto y comprueba que la versión del esquema es
   compatible. Si el generador publica una versión nueva mientras tanto, la consulta en curso termina sobre
   la versión que abrió.
3. Condiciones disponibles (todas se combinan con Y lógico):
   - **ID**: uno o varios.
   - **Número de caso**: uno o varios; devuelve todos los registros que comparten cada número.
   - **Igualdad** sobre tipo principal, distrito o área comunitaria, con uno o varios valores alternativos
     (O lógico). Se puede pedir explícitamente "sin valor" en los campos opcionales.
   - **Rango de fechas** con desde y hasta inclusivos; se puede indicar solo uno de los dos extremos
     (decisión C2).
   - **Igualdad sobre un campo sin índice** (decisión C3): solo se admite combinada con al menos una
     condición indexada (decisión C10).
4. El consultor resuelve la consulta así (decisión C10):
   - cada condición indexada (ID, número de caso, fecha, tipo principal, distrito o área comunitaria) se
     resuelve con su índice y da un conjunto de ID; con varios valores alternativos, el conjunto es la unión;
   - con varias condiciones indexadas, el resultado es la intersección de sus conjuntos de ID;
   - las condiciones sin índice se aplican después, como filtro sobre los registros de esa intersección.

   El consultor nunca recorre la tabla principal entera.
5. Opciones de salida: límite de resultados, solo conteo, y formato tabla, CSV o JSON (decisión C4). Los
   resultados en CSV o JSON se pueden guardar en un archivo de salida en lugar de mostrarse en consola
   (decisión C8). La salida JSON incluye la consulta que se ha hecho junto a los resultados (decisión C9).
6. Orden de los resultados (decisión C11): el orden no está especificado y puede variar entre ejecuciones;
   solo se garantiza el conjunto de resultados. Con un límite N se devuelven N registros cualesquiera de ese
   conjunto, que pueden no ser los mismos en cada ejecución.
7. Al terminar informa del número de resultados, del tiempo empleado, de los índices usados y de los filtros
   aplicados sin índice. Esta información y los avisos van por un canal distinto de los resultados, para que
   la salida CSV o JSON se pueda guardar o encadenar tal cual (decisión C5).
8. El código de salida es 0 cuando la consulta se ejecuta, aunque no haya resultados; distinto de 0 cuando se
   rechaza (decisión C6).

## Validaciones y mensajes

Se aplican las validaciones del consultor del documento padre, salvo la fila "Condición sobre un campo sin
índice", que la decisión C10 sustituye. Esta entrega añade:

| Situación | Regla | Mensaje al usuario |
|---|---|---|
| Ninguna condición, o solo condiciones sobre campos sin índice | Rechaza; hace falta al menos una condición indexada (C10) | `Indica al menos una condición sobre un campo indexado: ID, número de caso, fecha, tipo principal, distrito o área comunitaria.` |
| Varios ID y alguno no existe | Devuelve los que existen; avisa de cada uno que falta | `No existe ningún delito con ID 123.` |
| Número de caso inexistente | Resultado vacío, no error | `No existe ningún delito con número de caso JA000000.` |
| "Sin valor" sobre un campo obligatorio | Rechaza | `El campo "<nombre>" es obligatorio; no admite "sin valor".` |
| Valor de tipo incorrecto (por ejemplo, área comunitaria no numérica) | Rechaza | `"<valor>" no es válido para "<campo>": se esperaba un número entero.` |
| Rango de fechas sobre un campo que no es fecha | Rechaza | `Solo se admiten rangos sobre "Date".` |
| Límite no positivo | Rechaza | `El límite debe ser un número entero mayor que 0.` |
| Cancelación por el analista (Ctrl+C) | Termina sin dejar un archivo de salida a medias | `Consulta cancelada.` |
| Solo conteo junto con un formato de salida | Rechaza por contradictorio | `"Solo conteo" no se combina con un formato de salida.` |
| La versión activa desaparece o queda ilegible a mitad de la consulta | Aborta sin salida parcial engañosa | `No se pudo leer el conjunto de datos en <carpeta>.` |
| Archivo de salida con formato tabla o con solo conteo | Rechaza; el archivo solo admite CSV o JSON | `El archivo de salida solo admite los formatos CSV o JSON.` |
| El archivo de salida no se puede crear o escribir | Rechaza o aborta sin dejar un archivo a medias | `No se puede escribir el archivo de salida <ruta>.` |
| La consulta falla después de empezar a escribir el archivo | No queda ningún archivo a medias con el nombre pedido | Mensaje de la causa del fallo |

## Reutilización

- **Documento padre**: flujo B, validaciones del consultor, reglas de los campos y D1–D6.
- **Contrato compartido del conjunto**: esquema del registro, codificación, índices, puntero a la versión
  activa y manifiesto. El consultor lee exactamente lo que el generador escribe, sin redefinirlo.
- **Verificador independiente**: base para comprobar los criterios contra el CSV sin usar código del
  consultor.
- **Mediciones del CSV de referencia** del padre: cifras esperadas en la verificación.

## Fuera de alcance

- Todo lo excluido en el documento padre: modo interactivo, servidor o API, texto libre, búsqueda
  aproximada, geoespacial, agregaciones más allá del conteo e índices compuestos.
- Ordenar los resultados, por ID o por cualquier otro campo: queda previsto para un cambio posterior (C11).
- Rangos sobre campos distintos de la fecha (por ejemplo, ID entre A y B o Ward entre 1 y 10).
- Negaciones y O lógico entre condiciones distintas (solo O entre valores de una misma condición).
- Elegir qué campos aparecen en la salida o el orden de las columnas.
- Guardar en archivo la salida en tabla o el solo conteo, y añadir resultados a un archivo existente.
- Incluir la consulta en la salida CSV: solo la lleva el JSON.
- Consultas sin ninguna condición indexada, incluido recorrer el conjunto entero.
- Estadísticas de los índices y elección del plan de consulta por costes: el consultor siempre resuelve la
  consulta como indica C10, sin estimar qué índice es más selectivo. El peor caso conocido es el tipo
  principal `THEFT` (1.838.502 registros, el 21 % del conjunto), cuyos ID se leen completos aunque otra
  condición sea mucho más selectiva. Se podrá añadir más adelante si las mediciones lo justifican.
- Consultar una versión del conjunto distinta de la activa.
- Cachés o índices auxiliares creados por el consultor: el consultor nunca escribe en la carpeta del
  conjunto.
- Ejecutar o verificar en sistemas distintos de Windows. *(Anotación del 2026-10-14: Linux lo cubre el [PRD de
  soporte para Linux](soporte-linux.prd.md).)*

## Criterios de aceptación

Las cifras exactas corresponden al CSV de referencia, con la misma salvedad que en el documento padre. "El
filtrado directo del CSV" significa un recuento independiente sobre el CSV que no usa código del consultor.

1. **AC-C1** (padre AC3 y AC12) — Dada la muestra de ID de AC3 del padre más el registro con hora en el salto
   de primavera, cuando se consultan por ID, entonces cada campo devuelto es equivalente en valor al de la
   fila del CSV y esa fecha es exactamente la publicada.
2. **AC-C2** (padre AC4) — Cuando se cuenta el tipo principal `THEFT`, entonces el resultado es 1.838.502.
3. **AC-C3** (padre AC5) — Cuando se cuentan los registros con área comunitaria "sin valor", entonces el
   resultado es 613.720.
4. **AC-C4** (padre AC6) — Dado un rango de fechas de un mes, cuando se cuenta, entonces el resultado coincide
   con el filtrado directo del CSV, incluidos los registros del primer y del último instante del rango.
5. **AC-C5** (padre AC7) — Dado `THEFT` y distrito `008`, cuando se cuenta, entonces el resultado coincide con
   el filtrado directo del CSV y el consultor informa de que usó índices.
6. **AC-C6** (padre AC8) — Dado un número de caso compartido por varios registros, cuando se consulta,
   entonces se devuelven todos.
7. **AC-C7** (padre AC10) — Dado el conjunto de referencia, cuando se consulta un ID existente, entonces la
   respuesta completa llega en menos de 1 segundo en el equipo de referencia.
8. **AC-C8** (padre AC11) — Dada cualquier consulta de este documento, cuando se pide en CSV o JSON, entonces
   la salida es válida para ese formato, contiene los mismos registros que la salida en tabla y no contiene
   avisos ni estadísticas.
9. **AC-C9** (decisión C10) — Dada una condición sobre un campo sin índice (por ejemplo, Arrest = Sí)
   combinada con dos indexadas (por ejemplo, `THEFT` y un rango de fechas de un mes), cuando se cuenta,
   entonces el resultado coincide con el filtrado directo del CSV y el consultor informa de los dos índices
   usados y del filtro aplicado. Cuando la misma condición sin índice se pide sola, entonces se rechaza con
   el mensaje de la tabla.
10. **AC-C10** — Dada cada fila de la tabla de validaciones (padre y esta entrega), cuando se provoca la
    situación, entonces se muestra el mensaje indicado y el código de salida es el de la decisión C6.
11. **AC-C11** (decisión C11) — Dada una consulta con más de N resultados y un límite N, cuando se ejecuta,
    entonces se devuelven exactamente N registros y todos pertenecen al resultado de la misma consulta sin
    límite, aunque no sean necesariamente los mismos N en cada ejecución.
12. **AC-C12** — Dada una consulta en curso, cuando el generador publica una versión nueva, entonces la
    consulta termina con resultados coherentes de una sola versión.
13. **AC-C13** (decisión C7) — Dadas dos consultas de conteo, una por `THEFT` y otra por un tipo principal
    con pocos registros, y un rango de fechas de varios años combinado con un tipo principal, cuando se
    mide el pico de memoria de cada una, entonces los picos son del mismo orden y no crecen con el número de
    resultados.
14. **AC-C14** (decisión C8) — Dada una consulta con archivo de salida en CSV y en JSON, cuando se ejecuta,
    entonces el archivo contiene los mismos registros que la salida en tabla, la salida estándar no contiene
    resultados, y repetir la consulta sobrescribe el archivo sin preguntar.
15. **AC-C15** (decisión C8) — Dada una consulta con archivo de salida que se rechaza o falla, cuando termina,
    entonces no queda un archivo a medias con el nombre pedido y el código de salida indica el rechazo.
16. **AC-C16** (decisión C9) — Dada una consulta con tipo principal y un rango de fechas sin hora, cuando se
    pide en JSON, entonces la salida contiene la consulta interpretada, con la fecha final resuelta al último
    segundo del día, y la versión del conjunto consultada.
17. **AC-C17** (decisión C1) — Dado un conjunto regenerado con el esquema 2, cuando se cuenta el tipo principal
    escrito como `theft`, `Theft` y `THEFT`, entonces las tres dan 1.838.502, y al listar resultados el tipo
    principal sale como `THEFT`, igual que en el CSV. Un filtro sin índice sobre Location Description escrito
    en minúsculas da el mismo conteo que escrito en mayúsculas.
18. **AC-C18** (decisión C1) — Dado el conjunto regenerado, cuando se ejecuta el verificador independiente,
    entonces confirma que los índices de texto están en minúsculas y que la tabla principal coincide con el
    CSV tal cual. Dado un conjunto con esquema 1, cuando se consulta, entonces se rechaza con el mensaje de
    esquema incompatible del padre.

## Decisiones

No queda ninguna decisión abierta. Todas se confirmaron con el usuario el 2026-10-13 y ya están reflejadas en
el cuerpo del PRD.

- **C1 — Forma de los valores** (confirmada el 2026-10-13, revisada el mismo día): los textos se comparan sin
  distinguir mayúsculas (`theft`, `Theft` y `THEFT` son equivalentes) y sin depender de cómo vengan escritos
  en el CSV:
  - el generador guarda en minúsculas los valores de los índices secundarios de texto (tipo principal,
    distrito y número de caso), y el consultor convierte a minúsculas el valor buscado antes de usar el
    índice;
  - la tabla principal conserva cada campo exactamente como viene en el CSV, y la salida lo muestra así;
  - los filtros sin índice (C3) convierten a minúsculas tanto el valor buscado como el del campo antes de
    compararlos.

  Los códigos conservan los ceros a la izquierda y no se completan (`8` no equivale a `008`). Motivo: el
  primer borrador suponía que los textos publicados eran siempre mayúsculas. Se comprobó el 2026-10-13 que
  en el CSV de referencia es así en todos los campos consultables, pero Block tiene 13.886 filas con
  minúsculas y nada garantiza la regla en extractos futuros. Si se pasara a mayúsculas solo la búsqueda, un
  extracto con minúsculas daría conteos incompletos sin ningún error. Completar ceros sería adivinar.
  Consecuencia aceptada: cambia el contenido de los índices publicados, así que esta entrega incluye un
  cambio del generador y del verificador independiente, y sube la versión del esquema a 2. Los conjuntos
  con esquema 1 se rechazan con el mensaje del padre para un esquema incompatible y hay que regenerarlos.
- **C2 — Fechas en las consultas** (confirmada el 2026-10-13): formato `AAAA-MM-DD [hh:mm]` del padre, en hora civil local
  (D3). Un "desde" sin hora empieza a las 00:00:00; un "hasta" sin hora incluye el día entero, y con hora
  incluye el minuto entero. Se admite solo "desde" o solo "hasta".
- **C3 — Campos sin índice** (confirmada el 2026-10-13): igualdad sobre cualquier campo almacenado salvo los de texto largo
  (Block y Description), con el mismo tratamiento de valores que C1, y siempre como filtro de una consulta
  indexada (C10). Motivo: la búsqueda en texto libre está fuera de alcance.
- **C4 — Contenido de cada formato** (confirmada el 2026-10-13): CSV y JSON incluyen todos los campos almacenados en el
  orden del CSV (en JSON, junto a la consulta según C9), con Location reconstruida a partir de latitud y longitud para que el CSV tenga la misma
  cabecera que el de origen. La tabla legible muestra una fila por registro con todos los campos en
  columnas, aunque resulte ancha para una terminal. Motivo: el usuario prefiere ver siempre el registro
  completo antes que un resumen.
- **C5 — Canales de salida** (confirmada el 2026-10-13): los resultados van a la salida estándar; avisos, errores, número
  de resultados, tiempo e índices usados van a la salida de errores.
- **C6 — Códigos de salida** (confirmada el 2026-10-13): 0 si la consulta se ejecuta, aunque no haya resultados; 1 si se
  rechaza por el conjunto o por los datos de la consulta, o si aborta; 2 si la línea de comandos no es
  válida; 130 si el analista la cancela con Ctrl+C. Motivo: coincide con el generador.
- **C7 — Memoria** (confirmada el 2026-10-13, revisada el mismo día a petición del usuario): sin límite fijo
  como el del generador, pero el consumo no crece con el tamaño del resultado. Contar `THEFT` (1,8 millones)
  usa una memoria comparable a la de contar un tipo raro. Motivo: el padre exige no cargar el conjunto
  entero. La ordenación externa que necesitaría ordenar resultados grandes pasa a un cambio posterior junto
  con C11.

- **C8 — Archivo de salida** (confirmada el 2026-10-13): con formato CSV o JSON se puede indicar un archivo
  de salida. Los resultados se escriben en él en lugar de en la salida estándar; avisos, errores y
  estadísticas siguen yendo a la salida de errores (C5). Si el archivo ya existe, se sobrescribe siempre,
  sin pedir confirmación. Una consulta rechazada o fallida no deja un archivo a medias con el nombre pedido.
  Motivo: el usuario quiere guardar resultados para usarlos después y repetir consultas sin pasos extra.
- **C9 — La consulta en el JSON** (confirmada el 2026-10-13): la salida JSON, tanto en consola como en
  archivo, contiene la consulta interpretada junto a los resultados: cada condición tal como se aplicó (con
  los valores y los rangos de fecha resueltos a instantes exactos según C1 y C2), el límite si lo hubo y la
  versión del conjunto consultada. Motivo: el archivo documenta por sí solo qué se buscó y sobre qué datos,
  sin ambigüedad.
- **C10 — Intersección de índices y filtros** (confirmada el 2026-10-13): toda consulta necesita al menos una
  condición indexada. Las condiciones indexadas se combinan con Y lógico por intersección de los ID que
  devuelve cada índice; las condiciones sin índice filtran los registros de esa intersección. Una consulta
  sin condición indexada se rechaza y el consultor nunca recorre la tabla principal entera. Motivo: una
  condición sin índice sola, como Arrest = Sí, obliga a leer los 8,66 millones de registros y puede devolver
  millones de ellos. Corrige la fila "Condición sobre un campo sin índice" y la última frase de D2 del
  documento padre, que permitían el recorrido completo con aviso; el padre queda anotado. Consecuencia
  aceptada: la condición indexada limita el recorrido, pero no garantiza que sea pequeño (un rango de
  fechas de 25 años abarca casi todo el conjunto).
- **C11 — Orden de los resultados** (confirmada el 2026-10-13, revisada el mismo día a petición del
  usuario): el orden de los resultados no está especificado y puede variar entre ejecuciones; solo se
  garantiza el conjunto de resultados. En esta entrega el analista no puede pedir un orden. Un límite N
  devuelve N registros del conjunto de resultados, que pueden no ser los mismos en cada ejecución. Motivo:
  avanzar paso a paso; ordenar por campos, con el ID como desempate, la posición de los "sin valor" y la
  ordenación externa que eso requiere (C7), pasa a un cambio posterior. Corrige el paso 5 del flujo B del
  padre ("orden ascendente de ID"); el padre queda anotado.

## Referencias

- Documento padre: [chicago-crime.prd.md](chicago-crime.prd.md)
- PRD del generador: [generador.prd.md](generador.prd.md)
