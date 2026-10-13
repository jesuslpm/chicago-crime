# Generador del conjunto de datos — PRD

**Versión**: 1.0
**Fecha**: 2026-10-12
**Documento padre**: [Almacén consultable de delitos de Chicago sobre SSTable — PRD](chicago-crime.prd.md) (v1.2)

## Objetivo y contexto

Entregar el **generador**, la primera de las dos aplicaciones del documento padre. A partir del CSV de
delitos de Chicago, produce el conjunto de datos completo: la SSTable principal, las cinco SSTables de
índices secundarios y el manifiesto. El consultor queda para un cambio posterior.

Este documento no repite las reglas del padre, que siguen vigentes: flujo A de generación, reglas de los
campos, validaciones del generador, restricción de memoria y decisiones D1–D5. Solo añade lo que esta
entrega necesita y el padre deja abierto:

- el límite de la entrega;
- cómo se verifica el generador sin un consultor;
- los detalles operativos del uso por parte del operador: log, códigos de salida, cancelación y restos de
  ejecuciones interrumpidas.

### Estado de partida

Verificado el 2026-10-12:

| Aspecto | Situación |
|---|---|
| Producto | El documento padre v1.2 está cerrado, con las decisiones D1–D6 tomadas. |
| Código | No existe código Rust ni proyecto creado. |
| Datos | Está disponible el CSV de referencia descrito en el padre: 8.656.904 registros, sin ID duplicados y sin ordenar. |
| Seguimiento | No hay ningún documento de implementación abierto. |

## Actores

- **Operador del generador**: el definido en el documento padre. Es el único actor de esta entrega.

## Flujo

Sigue el flujo A del documento padre. Esta entrega concreta los puntos siguientes:

1. El operador ejecuta el generador indicando el CSV de entrada y la carpeta de salida. Opcionalmente indica
   la carpeta temporal, la ubicación del log (por defecto, junto a la carpeta de salida; decisión G1), si se
   permite sobrescribir un conjunto existente y si se desactiva la compresión, que está activada por defecto
   (decisión G2).
2. Antes de procesar ninguna fila, el generador comprueba que puede leer el CSV, escribir el log y usar la
   carpeta temporal. Si una generación anterior se interrumpió y dejó restos, los elimina o los ignora, sin que
   afecten a la generación nueva ni al conjunto publicado.
3. Durante el proceso muestra el progreso en consola y escribe en el log los registros rechazados y los
   avisos, según las reglas del padre. El log es un archivo propio de cada ejecución, fuera de la carpeta del
   conjunto, para que sobreviva aunque la generación falle; su contenido también se muestra en consola.
4. El operador puede cancelar en cualquier momento (Ctrl+C). La cancelación se trata como una generación
   fallida: se limpian los temporales y el conjunto anterior sigue intacto.
5. Al terminar, el resumen final indica también dónde está el log. El código de salida es 0 cuando la
   generación termina, aunque haya rechazos o aviso de memoria, y distinto de 0 cuando aborta o se cancela
   (decisión G3).
6. El manifiesto identifica el CSV de origen por su nombre, su tamaño y una huella del contenido calculada
   durante la misma lectura, sin una pasada adicional (decisión G4).
7. La salida es determinista: el mismo CSV produce siempre SSTables idénticas byte a byte. El manifiesto
   queda excluido porque incluye la fecha de generación (decisión G5).

## Validaciones y mensajes

Se aplican todas las validaciones del generador del documento padre. Esta entrega añade:

| Situación | Regla | Mensaje al usuario |
|---|---|---|
| El CSV no existe o no se puede leer | Aborta antes de procesar filas | `No se puede leer el archivo <ruta>.` |
| La carpeta temporal no existe o no admite escritura | Aborta antes de procesar filas | `No se puede usar la carpeta temporal <carpeta>.` |
| El log no se puede crear | Aborta antes de procesar filas, porque sin log no se pueden registrar los rechazos | `No se puede escribir el log en <ruta>.` |
| El CSV solo tiene la cabecera | Se publica un conjunto vacío, coherente con la decisión D5 del padre, y se avisa | `Aviso: el CSV no contiene registros; el conjunto generado está vacío.` |
| Cancelación por el operador | Se trata como una generación fallida | `Generación cancelada por el operador. No se ha modificado el conjunto existente.` |
| Restos de una generación interrumpida anterior | Se eliminan o se ignoran sin afectar al resultado | `Se han descartado restos de una generación interrumpida anterior.` |

## Verificación sin consultor

Mientras no exista el consultor, los criterios del padre que dicen "se consulta" se comprueban leyendo
directamente las tablas generadas con un lector independiente del formato de tabla de LevelDB, sin usar
código del propio generador. Los criterios que solo tienen sentido con el consultor (AC6, AC7, AC10 y AC11)
quedan para su cambio.

## Reutilización

- **Documento padre**: flujo A, reglas de los campos, validaciones del generador, restricción de memoria y
  decisiones D1–D5.
- **Mediciones del CSV de referencia** del padre: son las cifras esperadas en la verificación.

## Fuera de alcance

- El consultor y los criterios AC6, AC7, AC10 y AC11 del padre: van en el siguiente cambio.
- Un modo de inspección o consulta dentro del generador.
- Elegir qué índices se generan: el conjunto de índices es el fijado por la decisión D2 del padre.
- Regenerar un solo índice sin regenerar el conjunto completo.
- Generar a partir de varios CSV a la vez.
- Ejecutar o verificar en sistemas distintos de Windows: el equipo de referencia es Windows 11.
- Instaladores o distribución del ejecutable.

## Criterios de aceptación

Las cifras exactas corresponden al CSV de referencia, con la misma salvedad que en el documento padre.

1. **AC-G1** (padre AC1) — Dado el CSV completo, cuando se ejecuta el generador, entonces la SSTable principal
   contiene 8.656.904 entradas en orden estrictamente creciente de ID, hay 0 rechazados, y el manifiesto y el
   resumen coinciden con esas cifras.
2. **AC-G2** (padre AC2) — Dado el CSV completo, cuando se ejecuta el generador, entonces el pico de memoria
   privada comprometida no supera 512 MiB y el resumen final lo muestra.
3. **AC-G3** (padre AC3 y AC12) — Dada la muestra de ID de AC3 del padre más un registro con hora en el salto
   de primavera, cuando se lee la tabla principal directamente, entonces cada campo es equivalente en valor al
   de la fila del CSV y la fecha de ese registro es exactamente la publicada.
4. **AC-G4** (padre AC4, AC5 y AC8) — Dado el conjunto generado, cuando se leen los índices directamente,
   entonces:
   - cada índice tiene tantas entradas como registros válidos;
   - dentro de cada valor, los ID aparecen en orden creciente;
   - en el índice de tipo principal, el valor `THEFT` tiene 1.838.502 entradas (*anotación del
     2026-10-13: con el esquema 2, el valor indexado es `theft`, por la decisión C1 del
     [PRD del consultor](consultor.prd.md)*);
   - en el índice de área comunitaria, el valor "sin valor" tiene 613.720 entradas;
   - en el índice de número de caso, cada número compartido lista todos los ID que lo comparten.
5. **AC-G5** (padre AC9 y AC13) — Dados los CSV de prueba de AC9 y AC13 del padre, cuando se ejecuta el
   generador, entonces el resultado es el que describen esos criterios.
6. **AC-G6** — Dado un conjunto ya publicado, cuando el proceso se interrumpe bruscamente a mitad de la
   generación, entonces el conjunto anterior sigue intacto, no hay ningún conjunto incompleto visible y la
   siguiente generación termina con normalidad.
7. **AC-G7** — Dado un conjunto ya publicado, cuando el operador cancela con Ctrl+C, entonces no quedan
   temporales, el conjunto anterior sigue intacto y el código de salida indica fracaso.
8. **AC-G8** — Dada una carpeta de salida con un conjunto, cuando se ejecuta sin la opción de sobrescritura,
   entonces se rechaza sin modificar nada; cuando se ejecuta con ella, entonces el conjunto se reemplaza.
9. **AC-G9** — Dada una generación abortada, cuando el operador abre el log, entonces el log existe y explica
   la causa.
10. **AC-G10** — Dado el mismo CSV, cuando se genera dos veces, entonces las SSTables
    resultantes son idénticas byte a byte.

## Decisiones

No queda ninguna decisión abierta. Todas se tomaron el 2026-10-12 y ya están reflejadas en el cuerpo del PRD.

- **G1 — Log**: un archivo por ejecución, fuera de la carpeta del conjunto. Su ubicación es configurable y,
  por defecto, queda junto a la carpeta de salida. Además se muestra en consola. Motivo: un log dentro del
  conjunto se perdería en una generación fallida, que es justo cuando más falta hace.
- **G2 — Compresión**: Snappy activada por defecto, con opción para desactivarla. Motivo: los textos y las
  categorías se repiten mucho, así que es previsible que se reduzca bastante el tamaño. Desactivarla sirve
  para medir y comparar.
- **G3 — Códigos de salida**: 0 cuando la generación termina, aunque haya rechazos o aviso de memoria;
  distinto de 0 cuando aborta o se cancela. Motivo: permite automatizar sin leer el log, y los rechazos ya
  quedan en el resumen y en el manifiesto.
- **G4 — Origen en el manifiesto**: nombre, tamaño y una huella del contenido calculada durante la lectura.
  Motivo: permite saber con certeza de qué extracto salió un conjunto, sin una pasada adicional.
- **G5 — Salida determinista**: el mismo CSV produce SSTables idénticas byte a byte; el manifiesto queda
  excluido. Motivo: facilita verificar y comparar generaciones, y detecta cambios involuntarios.

## Referencias

- Documento padre: [chicago-crime.prd.md](chicago-crime.prd.md)
