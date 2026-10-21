# Búsqueda por prefijo en el consultor ("startswith") — PRD

**Versión**: 1.0
**Fecha**: 2026-10-18
**Documento padre**: [Consultor del conjunto de datos — PRD](consultor.prd.md) (v1.1)

## Objetivo y contexto

Que el analista pueda buscar delitos cuyo campo de texto **empieza por** un texto dado, además de las
condiciones de igualdad que ya existen. Hoy, para contar todos los daños y allanamientos "criminales" hay que
escribir cada tipo principal completo (`CRIMINAL DAMAGE`, `CRIMINAL TRESPASS`, `CRIMINAL SEXUAL ASSAULT`...)
y saber de antemano cuáles existen; con un prefijo basta `CRIM`. Lo mismo para los números de caso de una
serie (`JA`), los distritos de un grupo (`01`) o los códigos IUCR de una familia (`04`).

**Por qué ahora**: es la primera ampliación del consultor y el caso de la charla de metodología, en la que
este PRD se implementa dos veces a la vez, con vibe coding y con ODD (ver el
[PRD de worktrees](../worktrees/worktrees.prd.md)).

Este documento no repite las reglas del consultor, que siguen vigentes (C1–C11), ni las del documento padre
general. Solo añade la condición "empieza por" y cómo encaja con ellas.

### Estado de partida

Verificado el 2026-10-18 sobre el repositorio y el CSV de referencia:

| Aspecto | Situación |
|---|---|
| Consultor | Implementado y aceptado (AC-C1 a AC-C18). Admite ID, igualdad (con valores alternativos y "sin valor"), rangos de fechas y filtros sin índice; toda consulta necesita una condición indexada (C10). No admite ninguna búsqueda por prefijo. |
| Índices de texto | Número de caso, tipo principal y distrito están indexados, en minúsculas (C1), ordenados por valor y, dentro de cada valor, por ID. Todos los valores que empiezan por un mismo texto quedan seguidos en el índice. |
| Campos de texto sin índice | IUCR, Location Description, Beat y FBI Code admiten igualdad como filtro (C3). Block y Description no admiten condiciones (C3; el padre general excluye el texto libre). |
| Generador | No necesita cambios: el prefijo se resuelve con los índices ya publicados. El esquema sigue en la versión 2. |
| Selector independiente | La herramienta de aceptación filtra el CSV por igualdad y rango sin código del consultor. No conoce el prefijo. |
| Tamaño de los prefijos | Medido sobre los 8.656.904 registros: `CRIM` en tipo principal abarca 4 tipos y 1.256.646 registros; `JA` en número de caso, 269.607 números y 269.655 registros; `01` en distrito, 9 distritos y 3.386.217 registros. El peor caso es `H` en número de caso: 5.761.892 números distintos y 5.762.199 registros (el 67 % del conjunto). |

## Actores

- **Analista**: el del consultor. Es el único actor de esta entrega.

## Flujo

Sigue el flujo del consultor. Esta entrega añade:

1. El analista añade a su consulta una o varias condiciones "empieza por", con la forma
   `--startswith CAMPO=PREFIJO` (decisión P2), solas o junto a las condiciones que ya existen. La ayuda del
   consultor la describe junto a las demás condiciones.
2. El consultor compara sin distinguir mayúsculas, igual que la igualdad (C1): `crim`, `Crim` y `CRIM` son
   equivalentes. El prefijo se toma tal cual, espacios incluidos; los códigos no se completan con ceros
   (`8` no es prefijo de `008`, `00` sí).
3. Todos los valores y prefijos pedidos sobre un mismo campo son alternativas (O lógico), como hoy los
   valores repetidos de una igualdad (decisión P1): `--where "Primary Type=THEFT" --startswith
   "Primary Type=CRIM"` devuelve THEFT y todos los tipos que empiezan por CRIM. Sobre campos distintos se
   cumplen todas (Y lógico).
4. Sobre número de caso, tipo principal o distrito, la condición es **indexada**: cuenta como la condición
   indexada que exige C10 y puede ir sola (decisión P3). Sobre IUCR, Location Description, Beat o FBI Code es un **filtro
   sin índice** y necesita, como cualquier filtro, al menos una condición indexada.
5. Un registro "sin valor" en el campo nunca empieza por ningún prefijo.
6. La salida no cambia: los registros se muestran como en el CSV, el orden sigue sin especificar (C11) y un
   mismo registro no sale dos veces aunque cumpla varios prefijos alternativos. Las estadísticas indican el
   índice usado o el filtro aplicado como hoy. La salida JSON incluye la condición tal como se aplicó, con el
   prefijo en minúsculas (C9).
7. La memoria no crece con el número de resultados ni con el número de valores distintos que abarca el
   prefijo (C7): el prefijo `H` de número de caso, que abarca 5.761.892 números distintos, usa una memoria
   comparable a la de un prefijo que abarque uno solo.

## Validaciones y mensajes

Siguen vigentes las validaciones del consultor. Esta entrega añade:

| Situación | Regla | Mensaje al usuario |
|---|---|---|
| Prefijo vacío (`CAMPO=` sin nada detrás) | Rechaza; "sin valor" solo se pide con la igualdad. Basta un carácter (P6) | `El prefijo de "<campo>" no puede estar vacío.` |
| Prefijo sobre un campo que no es de texto (ID, fechas, números, Arrest, Domestic, coordenadas) | Rechaza | `"Empieza por" solo se admite en campos de texto: Case Number, Primary Type, District, IUCR, Location Description, Beat y FBI Code.` |
| Prefijo sobre Block o Description | Rechaza, como la igualdad (C3) | El de la igualdad: `El campo "<nombre>" no admite condiciones. Campos disponibles: ...` |
| Prefijo sobre un campo inexistente | Rechaza, como la igualdad | El de la igualdad: `El campo "<nombre>" no existe. Campos disponibles: ...` |
| Condición sin `=` | Rechaza por línea de comandos (código 2) | `La condición "<texto>" no tiene la forma CAMPO=PREFIJO.` |
| Solo prefijos sobre campos sin índice | Rechaza (C10) | El de C10: `Indica al menos una condición sobre un campo indexado: ...` |
| Ningún registro empieza por el prefijo | Resultado vacío, no error, sin aviso (P4) | — |

## Reutilización

- **Consultor**: forma de los valores (C1), canales y códigos de salida (C5, C6), formatos y archivo de
  salida (C4, C8, C9), intersección de condiciones indexadas y filtros (C10), orden no especificado (C11) y
  mensajes ya existentes para campos inexistentes o sin condiciones.
- **Índices publicados**: el prefijo se resuelve sobre los mismos índices de texto, sin índices nuevos.
- **Selector independiente**: se amplía con la misma condición para comprobar los criterios contra el CSV
  sin usar código del consultor.

## Fuera de alcance

- Termina por, contiene, comodines y expresiones regulares.
- Prefijos sobre Block y Description: siguen excluidos como texto libre (padre general y C3).
- Prefijos sobre campos que no son de texto (ID, fechas, números, booleanos, coordenadas).
- Prefijos que distinguen mayúsculas.
- Negación ("no empieza por").
- Cambios en el generador, índices nuevos o una versión nueva del esquema.
- Un objetivo de tiempo para las consultas por prefijo (P5).
- Una longitud mínima del prefijo (P6).
- Todo lo que el consultor ya dejó fuera (orden de resultados, O entre condiciones distintas, consultas sin
  condición indexada, plan por costes...).

## Criterios de aceptación

Las cifras exactas corresponden al CSV de referencia. "El filtrado directo del CSV" es el del selector
independiente ampliado con el prefijo.

1. **AC-S1** — Cuando se cuenta el tipo principal que empieza por `crim`, `Crim` y `CRIM`, entonces las tres
   dan 1.256.646 y coincide con el filtrado directo del CSV; al listar, el tipo sale en mayúsculas como en
   el CSV.
2. **AC-S2** — Cuando se cuenta solo el número de caso que empieza por `JA`, entonces se acepta sin otra
   condición, da 269.655 y las estadísticas indican el índice de número de caso.
3. **AC-S3** — Dado el tipo principal `THEFT` y el distrito que empieza por `01`, cuando se cuenta, entonces
   coincide con el filtrado directo del CSV e informa de los dos índices.
4. **AC-S4** — Dado Location Description que empieza por `resid` junto a un rango de fechas de un mes, cuando
   se cuenta, entonces coincide con el filtrado directo del CSV e informa del filtro sin índice. Cuando ese
   prefijo se pide solo, entonces se rechaza con el mensaje de C10.
5. **AC-S5** — Dados dos prefijos alternativos que se solapan sobre el tipo principal (`CRIM` y
   `CRIMINAL D`), cuando se cuenta, entonces el resultado es 1.256.646, el mismo que con `CRIM` solo: ningún
   registro se cuenta dos veces.
6. **AC-S6** (P1) — Dados `--where "Primary Type=THEFT"` y `--startswith "Primary Type=CRIM"`, cuando se
   cuenta, entonces el resultado es 3.095.148 (1.838.502 + 1.256.646) y coincide con el filtrado directo del
   CSV.
7. **AC-S7** — Cuando se cuenta el distrito que empieza por `8`, entonces el resultado es 0 sin error; con
   `00`, coincide con el filtrado directo del CSV.
8. **AC-S8** (C7) — Dados el conteo del número de caso que empieza por `H` (5.762.199 registros) y el de un
   prefijo que abarque un solo número de caso (`JK438266`), cuando se mide el pico de memoria, entonces los
   dos son del mismo orden.
9. **AC-S9** (C4, C9) — Dada una consulta con prefijo, cuando se pide en CSV y en JSON, entonces la salida
   contiene los mismos registros que en tabla, y el JSON incluye la condición "empieza por" con el prefijo
   en minúsculas.
10. **AC-S10** (C11) — Dada una consulta con prefijo con más de N resultados y un límite N, cuando se
    ejecuta, entonces se devuelven exactamente N registros del resultado sin límite.
11. **AC-S11** — Dada cada fila de la tabla de validaciones, cuando se provoca, entonces se muestra el
    mensaje indicado con el código de salida de C6.
12. **AC-S12** — Dadas las consultas de los criterios del consultor, cuando se repiten, entonces los
    resultados no cambian.

## Decisiones

No queda ninguna decisión abierta. Todas se confirmaron con el usuario el 2026-10-18 y ya están reflejadas en
el cuerpo del PRD.

- **P1 — Igualdad y prefijo sobre el mismo campo** (confirmada el 2026-10-18): se combinan con O lógico,
  igual que los valores repetidos de una igualdad; también "sin valor" pedido con la igualdad es una
  alternativa más. Motivo: la regla del consultor es que un campo repetido da alternativas; con Y, una
  igualdad y un prefijo distintos darían casi siempre un resultado vacío.
- **P2 — Forma de la condición** (confirmada el 2026-10-18): una opción propia, `--startswith CAMPO=PREFIJO`,
  repetible. Motivo: marcar el prefijo dentro de `--where` (por ejemplo `CRIM*`) sería un comodín, fuera de
  alcance, y haría ambiguo un valor que contenga ese carácter.
- **P3 — Condición indexada** (confirmada el 2026-10-18): sobre número de caso, tipo principal o distrito el
  prefijo cuenta como condición indexada para C10 y puede ir solo. Consecuencia aceptada: puede abarcar casi
  todo el conjunto (`H` en número de caso, `0` en distrito), la misma que C10 ya aceptó para un rango de
  fechas de 25 años.
- **P4 — Sin aviso por resultado vacío** (confirmada el 2026-10-18): un prefijo sin coincidencias da 0
  resultados sin aviso, también en número de caso. Motivo: el aviso del consultor para un número de caso
  inexistente señala una búsqueda puntual que falló; un prefijo es una búsqueda abierta y que no encuentre
  nada es un resultado normal.
- **P5 — Sin objetivo de tiempo** (confirmada el 2026-10-18): no se fija un tiempo máximo para las consultas
  por prefijo; la garantía es la de memoria (AC-S8). Motivo: no hay medición en la que apoyar una cifra. El
  tiempo real se mide y se anota en el informe de cierre.
- **P6 — Sin longitud mínima** (confirmada el 2026-10-18): el prefijo solo tiene que no estar vacío; basta un
  carácter. Motivo: la longitud no indica el coste. `H` en número de caso (1 carácter) abarca 5.762.199
  registros, pero `01` en distrito (2) abarca 3.386.217, `TH` en tipo principal (2) abarca 1.838.502 y `JA` en
  número de caso (2) abarca 269.655. Un mínimo dejaría pasar prefijos muy amplios y bloquearía consultas
  legítimas, y en distrito, IUCR, Beat o FBI Code un prefijo útil ya tiene uno o dos caracteres. El volumen ya
  está acotado como en el resto del consultor: la memoria no crece (AC-S8) y quedan el solo conteo, el límite
  y Ctrl+C. Si el coste preocupara, la vía es el plan por costes que el consultor dejó fuera de alcance.

## Referencias

- Documento padre: [consultor.prd.md](consultor.prd.md)
- Documento padre general: [chicago-crime.prd.md](chicago-crime.prd.md)
- [PRD de worktrees](../worktrees/worktrees.prd.md): contexto de la doble implementación.

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 13133ac1 | 2026-10-18 05:27 UTC+01:00 | 19 min | 0 min | 19 min | 8 min | 11 min | 10 |

<!-- prd-time: [{"session": "13133ac1-236b-41c5-9ac5-59cebb5b57b4", "start": "2026-10-18T04:27:17.977000+00:00", "wall": 1158, "discarded": 0, "agent": 474, "human": 683, "interventions": 10, "idle_minutes": 10}] -->
