# Verificación de aceptación con cualquier CSV compatible — PRD

**Versión**: 1.0
**Fecha**: 2026-10-16
**Documento padre**: [Almacén consultable de delitos de Chicago sobre SSTable — PRD](chicago-crime.prd.md) (v1.2)

## Objetivo y contexto

Que la verificación de aceptación del generador y la del consultor funcionen con cualquier descarga
compatible del CSV de Chicago, no solo con el extracto de referencia. Con el extracto de referencia se siguen
comprobando las cifras exactas de los PRD. Con otra descarga, cada cifra esperada sale del propio CSV.

**Por qué**: el portal actualiza el CSV a diario, y el README explica cómo descargarlo. Una descarga nueva
tiene más registros, así que hoy la verificación falla aunque el generador y el consultor funcionen bien. Los
PRD ya prevén este caso: "Con otro CSV compatible, esos resultados deben coincidir con los registros válidos
realmente procesados" (criterios de aceptación del padre, repetido en los del generador y el consultor). La
verificación no cumple esa regla.

De paso, los ejemplos del README usan carpetas en `D:\`, que no existe en todos los equipos. Pasan a usar
`C:\`.

Este documento no cambia ninguna regla del padre ni de los PRD del [generador](generador.prd.md) y del
[consultor](consultor.prd.md). El generador y el consultor no cambian.

### Estado de partida

Verificado el 2026-10-16 sobre el repositorio en el commit `407d43d`:

| Aspecto | Situación |
|---|---|
| Regla de los PRD | Las cifras exactas son del extracto de referencia; con otro CSV compatible deben coincidir con los registros realmente procesados. |
| Identidad del extracto de referencia | El README publica su fecha (2026-10-10), registros (8.656.904), tamaño (2.406.401.982 bytes) y huella SHA-256. El generador calcula la huella del CSV mientras lo lee y la guarda en el manifiesto. La verificación no la lee. |
| Verificación del generador | Exige el resumen `8.656.904 registros válidos, 0 rechazados` (AC-G1). Construye los CSV de prueba de AC-G5 a partir de la primera fila del extracto de referencia, con su ID y su fecha escritos en el script: si el CSV empieza por otra fila, la fila de fecha inválida se rechaza por ID repetido y AC-G5 falla. |
| Verificación del consultor | Exige `THEFT` = 1.838.502 (AC-C2 y AC-C17; AC-C11 compara con esa cifra la lista de robos del filtrado directo) y área comunitaria "sin valor" = 613.720 (AC-C3). El resto de conteos ya se compara con el filtrado directo del CSV. |
| Verificador independiente | Las dos verificaciones lo ejecutan siempre pidiendo las cifras de referencia: 8.656.904 registros, 0 rechazados, `THEFT`, "sin valor", 632 números de caso repetidos y 253 registros en el salto de primavera. Sin esa petición compara todo con el CSV. |
| Comprobaciones omitidas | Sin las cifras de referencia, el verificador omite una comprobación cuando el CSV no tiene el dato que necesita (por ejemplo, ningún registro en el salto de primavera). Las dos verificaciones cuentan cualquier omisión como fallo. |
| Datos históricos elegidos | Algunas consultas usan datos elegidos sobre el extracto de referencia: agosto de 2019, el salto de primavera de 2019, el tipo raro `RITUALISM`, un ID inexistente (123) y un número de caso inexistente. Son de 2019 o anteriores, así que una descarga más nueva los conserva casi con seguridad. |
| Pruebas por tiempo | La interrupción y la cancelación de AC-G6 y AC-G7 no dependen del tamaño del CSV: un CSV más grande solo les da más margen. |
| Espacio en disco | Las verificaciones exigen 6 GB y 5 GB libres, fijos, sin relación con el tamaño del CSV. |
| Informes | Muestran la ruta y el tamaño del CSV, pero no su huella, y lo llaman "CSV de referencia" aunque sea otro. |
| README | Los ejemplos de generar, consultar, verificar y del verificador independiente usan carpetas en `D:\`, igual que los ejemplos de ayuda de los dos scripts y la guía de worktrees, que cita la carpeta de aceptación del README. Dice que las verificaciones comprueban los criterios "on the reference CSV". |

## Actores

- **Desarrollador**: descarga el CSV del portal, o usa el extracto de referencia, y ejecuta la verificación
  del generador o la del consultor.

## Flujo

1. El desarrollador ejecuta la verificación del generador o la del consultor, igual que hoy, con el extracto
   de referencia o con otra descarga.
2. Después de generar el conjunto, la verificación compara la huella del CSV que guarda el manifiesto con la
   del extracto de referencia que publica el README (decisión A1).
3. **Si coinciden**, la verificación hace las mismas comprobaciones que hoy, incluidas las cifras exactas y
   el verificador independiente con las cifras de referencia.
4. **Si no coinciden**, cada cifra esperada sale del propio CSV (decisión A2):
   - registros válidos: las filas de datos del CSV, con 0 rechazados;
   - `THEFT` y área comunitaria "sin valor": el filtrado directo del CSV;
   - el verificador independiente se ejecuta sin pedir las cifras de referencia.
5. Los CSV de prueba de AC-G5 se construyen a partir de la primera fila del CSV recibido, sea cual sea.
6. Toda comprobación tiene que pasar, igual que hoy: una comprobación omitida por falta de datos cuenta como
   fallo (decisión A3).
7. El informe dice en su cabecera si el CSV es el extracto de referencia y muestra su huella.

## Validaciones y mensajes

Los informes de aceptación se escriben en inglés, como hasta ahora.

| Situación | Regla | Mensaje en la cabecera del informe |
|---|---|---|
| El CSV es el extracto de referencia | Se exigen las cifras exactas de los PRD | `CSV: the reference extract (SHA-256 <huella>); the exact figures of the PRDs are required.` |
| El CSV es otro | Las cifras esperadas salen del propio CSV | `CSV: not the reference extract (SHA-256 <huella>); the expected figures are counted on this CSV.` |

## Documentación

- **README**, sección de verificación: dice que las dos verificaciones funcionan con el extracto de referencia
  y con cualquier descarga posterior, explica los dos modos y cómo se reconoce el extracto de referencia (su
  huella, que el README ya publica).
- **Carpetas de ejemplo**: todos los ejemplos con carpetas en `D:\` pasan a `C:\` (decisión A4), en el
  README, en los ejemplos de ayuda de los dos scripts y en la guía de worktrees.

## Reutilización

- La huella SHA-256 del CSV que el generador ya guarda en el manifiesto.
- La identidad del extracto de referencia publicada en el README.
- El verificador independiente, que ya compara todo con el CSV cuando no se le piden las cifras de
  referencia.
- El filtrado directo del CSV que la verificación del consultor ya usa para el resto de conteos.

## Fuera de alcance

- Cambios en el generador, el consultor o las reglas del verificador independiente.
- Actualizar las cifras de los PRD o del README a una descarga nueva: siguen siendo las del extracto de
  referencia.
- CSV con filas rechazadas o ID repetidos: con otro CSV se siguen exigiendo 0 rechazados, así que la
  verificación no pasa.
- Mensajes específicos para un CSV al que le faltan los datos históricos elegidos (agosto de 2019, el salto
  de primavera): la verificación no pasa, como hoy.
- Ajustar el espacio en disco exigido al tamaño del CSV.
- Descargar el CSV automáticamente.
- Ejecutar las verificaciones en Linux.
- Cambiar las carpetas `D:\` que aparecen en los PRD de cambios anteriores: son historia.

## Criterios de aceptación

1. **AC-A1** — Dado el extracto de referencia, cuando se ejecutan las dos verificaciones, entonces pasan,
   comprueban las mismas cifras exactas que antes del cambio, incluido el verificador independiente con las
   cifras de referencia, y la cabecera del informe dice que es el extracto de referencia y muestra su huella.
2. **AC-A2** — Dada una descarga del portal posterior al extracto de referencia, cuando se ejecutan las dos
   verificaciones, entonces pasan, la cabecera del informe dice que no es el extracto de referencia y muestra
   su huella, y los registros válidos, `THEFT` y área comunitaria "sin valor" coinciden con el filtrado
   directo de esa descarga.
3. **AC-A3** — Dado un CSV cuya primera fila no es la del extracto de referencia, cuando se ejecuta la
   verificación del generador, entonces AC-G5 pasa.
4. **AC-A4** — Dado el README, cuando se lee la sección de verificación, entonces explica los dos modos y
   cómo se reconoce el extracto de referencia; y ningún ejemplo del README, de la ayuda de los dos scripts ni
   de la guía de worktrees usa una carpeta en `D:\`.

## Decisiones

- **A1 — Cómo se reconoce el extracto de referencia**: por la huella SHA-256 que el generador guarda en el
  manifiesto, comparada con la que publica el README. Motivo: no cuesta ninguna lectura extra y nadie tiene
  que acordarse de una opción. Se descartó una opción de los scripts para elegir el modo, porque permitiría
  verificar el extracto de referencia sin sus cifras exactas.
- **A2 — Cifras esperadas con otro CSV**: salen del propio CSV, mediante el verificador independiente sin
  las cifras de referencia y el filtrado directo que la verificación del consultor ya usa. Motivo: es la
  salvedad que ya fijan los PRD.
- **A3 — Comprobaciones omitidas**: con otro CSV, una comprobación omitida por falta de datos sigue contando
  como fallo. Motivo: la aceptación solo vale si prueba cada criterio.
- **A4 — Carpetas de ejemplo**: `C:\` en lugar de `D:\`. Motivo: `C:\` existe en cualquier equipo Windows;
  `D:\` no.
- **A5 — CSV para probar AC-A2 y AC-A3** (2026-10-16): una descarga real del portal, hecha durante la
  verificación de este cambio y que no se guarda en el repositorio. Motivo: es el caso real que motiva el
  cambio. Se descartó una copia recortada del extracto de referencia porque es sintética. Consecuencia
  aceptada: la descarga no se puede repetir, porque el portal cambia a diario. Si su primera fila coincide
  con la del extracto de referencia, AC-A3 se prueba con una copia del extracto con otra fila en primer
  lugar, generada en el momento.

## Referencias

- Documento padre: [chicago-crime.prd.md](chicago-crime.prd.md), criterios de aceptación y su salvedad.
- [PRD del generador](generador.prd.md) (AC-G1, AC-G4 y AC-G5) y [PRD del consultor](consultor.prd.md)
  (AC-C2, AC-C3, AC-C11, AC-C17 y AC-C18).
- README, apartado de obtención de los datos: identidad del extracto de referencia.

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 89464d25 | 2026-10-16 06:34 UTC+01:00 | 28 min | 0 min | 28 min | 12 min | 16 min | 7 |

<!-- prd-time: [{"session": "89464d25-9c83-4b20-8557-c6611235a8df", "start": "2026-10-16T05:34:29.114000+00:00", "wall": 1650, "discarded": 0, "agent": 699, "human": 951, "interventions": 7, "idle_minutes": 10}] -->
