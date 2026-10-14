# Soporte para Linux — PRD

**Versión**: 1.0
**Fecha**: 2026-10-14
**Documento padre**: [Almacén consultable de delitos de Chicago sobre SSTable — PRD](chicago-crime.prd.md) (v1.2)

## Objetivo y contexto

Que el proyecto se pueda compilar y probar en Linux con el mismo resultado que en Windows, y que cualquier
persona sepa cómo hacerlo, tanto en Linux dentro de Windows (WSL) como en un equipo Linux normal.

**Por qué**: los PRD del [generador](generador.prd.md) y del [consultor](consultor.prd.md) dejaron fuera de
alcance "ejecutar o verificar en sistemas distintos de Windows". Una prueba hecha el 2026-10-14 mostró que el
código ya compila en Linux, pero que el generador deja en la carpeta de salida un archivo propio que en
Windows desaparece solo. Ese archivo hace fallar las pruebas y, lo que es peor para el operador, queda dentro
de carpetas que el generador no debería tocar.

Este documento no cambia ninguna regla del padre ni de los PRD del generador y del consultor: lo que vale en
Windows pasa a valer también en Linux, incluidos la medición del pico de memoria y el descarte de restos de
generaciones interrumpidas, con las excepciones que se indican en [Fuera de alcance](#fuera-de-alcance).

### Estado de partida

Verificado el 2026-10-14 en Ubuntu 24.04 sobre WSL2, en el equipo de referencia, con el repositorio en el
commit `98c5b4d`:

| Aspecto | Situación |
|---|---|
| Compilación | Todo el proyecto compila en Linux y el análisis estático no da ningún aviso. |
| Pruebas | Se compilan 420 de las 424 pruebas (4 son exclusivas de Windows). Pasan 407 y **fallan 13**, todas del generador. |
| Causa de los fallos | Mientras trabaja, el generador marca la carpeta de salida como ocupada con un archivo de bloqueo. En Windows ese archivo se borra solo al terminar; en Linux se queda. |
| Efectos para el operador | En Linux: una carpeta de salida creada por una generación fallida no se borra entera; después de una generación correcta queda un archivo además del conjunto; y si el generador rechaza una carpeta con contenido ajeno, deja dentro ese archivo. |
| Consultor | Sus pruebas pasan todas en Linux. |
| Memoria | En Linux, el resumen del generador dice `Pico de memoria privada comprometida: no disponible en este sistema.` y nunca avisa de que se ha superado el límite. |
| Restos de generaciones interrumpidas | En Linux, el generador no detecta que la generación que dejó unos restos de ordenación ya terminó, así que nunca los descarta. Esto lo permite el PRD del generador ("los elimina o los ignora"). |
| Scripts de aceptación | Están escritos para PowerShell en Windows y usan funciones propias de Windows. |
| CSV de referencia | No está en el repositorio. Las pruebas automáticas no lo necesitan; la verificación de aceptación sí. |
| Documentación | El README solo explica cómo compilar y ejecutar en Windows. |

## Actores

- **Desarrollador**: compila el proyecto y pasa las pruebas en Linux, en WSL o en un equipo Linux.
- **Operador del generador**: el del documento padre. En Linux espera el mismo comportamiento que en Windows.

## Flujo

1. El desarrollador lee en el README un resumen de cómo trabajar en Linux y sigue el enlace a la guía que le
   corresponda: WSL o Linux normal.
2. Siguiendo la guía, prepara el entorno, obtiene el código y, si va a hacer la verificación de aceptación,
   copia el CSV de referencia.
3. Compila el proyecto y pasa las pruebas. Pasan todas las que se compilan en Linux.
4. El operador ejecuta el generador en Linux. Al terminar, con éxito, con error o cancelado, la carpeta de
   salida queda exactamente igual que quedaría en Windows.
5. El resumen final muestra el pico de memoria medido y, si supera el límite, el log lleva el mismo aviso que
   en Windows (decisión L3).
6. Si una generación anterior se interrumpió, la siguiente descarta sus restos de ordenación igual que en
   Windows.

## Validaciones y mensajes

No hay mensajes nuevos. En Linux se aplican las mismas reglas y los mismos mensajes que en Windows:

| Situación | Regla | Mensaje al usuario |
|---|---|---|
| Dos generaciones a la vez sobre la misma carpeta de salida | La segunda aborta sin tocar nada | `Otra generación está usando la carpeta de salida <carpeta>.` |
| Generación correcta | En la carpeta de salida solo quedan el conjunto publicado y lo que indica cuál es el actual | (resumen habitual) |
| Generación fallida o cancelada que creó la carpeta de salida | La carpeta se borra entera | (mensaje habitual de la causa) |
| Carpeta de salida con contenido ajeno | Se rechaza sin crear, cambiar ni borrar nada en ella | (mensaje habitual de contenido ajeno) |
| Fin de la generación | El resumen muestra el pico medido contra el límite, nunca "no disponible" | `Pico de memoria privada comprometida: <cifra> ...` (mismo formato que en Windows) |
| El pico supera el límite | Se anota un aviso una sola vez y la generación continúa | `Aviso: el pico de memoria privada comprometida (<cifra> MiB) supera el límite de <límite> MiB.` |
| Restos de ordenación de una generación que ya no está en marcha | Se descartan | `Se han descartado restos de una generación interrumpida anterior.` |
| Restos de ordenación de una generación que sigue en marcha | Se dejan intactos | (ninguno) |

## Documentación

- **README**: un apartado nuevo de un par de párrafos que resume cómo compilar y pasar las pruebas en Linux.
  Distingue entre WSL y Linux normal, y enlaza con las guías detalladas.
- **Guía de WSL**: cómo preparar una distribución de Linux en WSL; instalar el compilador y las herramientas;
  obtener el código dentro del sistema de archivos de Linux y no en el disco de Windows, explicando por qué;
  copiar el CSV de referencia, que no está en el repositorio; compilar; pasar las pruebas; y ejecutar las dos
  aplicaciones.
- **Guía de Linux normal**: lo mismo que la de WSL sin la parte propia de WSL.
- **Ambas guías** dicen qué no funciona todavía en Linux (los scripts de aceptación) y cómo se mide la
  memoria allí (decisión L3), para que nadie lo descubra por sorpresa.

## Reutilización

- Todas las reglas del documento padre y de los PRD del generador y del consultor, sin cambios.
- Las pruebas automáticas actuales: en Linux deben pasar las mismas que en Windows, salvo las 4 que solo
  existen en Windows.

## Fuera de alcance

- Ejecutar los scripts de aceptación en Linux: siguen siendo solo para Windows.
- Integración continua en Linux.
- Paquetes, instaladores o binarios publicados para Linux.
- macOS y otros sistemas.
- Cualquier cambio de comportamiento en Windows.

## Criterios de aceptación

1. **AC-L1** — Dado el repositorio en Linux (Ubuntu 24.04 sobre WSL2), cuando se pasan las pruebas, entonces
   pasan todas las que se compilan en Linux, sin ningún fallo.
2. **AC-L2** — Dado el repositorio en Windows, cuando se pasan las pruebas, entonces pasan todas, y ninguna
   de las 423 que pasaban antes del cambio desaparece ni deja de pasar (de las 424 del proyecto, una solo
   existía fuera de Windows).
3. **AC-L3** — Dada una generación correcta en Linux, cuando el operador mira la carpeta de salida, entonces
   solo contiene lo mismo que contendría en Windows.
4. **AC-L4** — Dada en Linux una generación fallida o cancelada que creó la carpeta de salida, cuando
   termina, entonces la carpeta ya no existe.
5. **AC-L5** — Dada en Linux una carpeta de salida con contenido ajeno, cuando el generador la rechaza,
   entonces la carpeta queda exactamente como estaba, sin ningún archivo nuevo.
6. **AC-L6** — Dadas dos generaciones a la vez sobre la misma carpeta de salida en Linux, cuando arranca la
   segunda, entonces aborta con `Otra generación está usando la carpeta de salida <carpeta>.` y la primera
   termina con normalidad.
7. **AC-L7** — Dado el README, cuando un desarrollador lo lee, entonces encuentra un apartado de un par de
   párrafos sobre Linux con enlaces a la guía de WSL y a la de Linux normal, y siguiendo cualquiera de las
   dos puede compilar y pasar las pruebas sin más ayuda.
8. **AC-L8** (generador AC-G2 en Linux) — Dado el CSV de referencia completo en Linux, cuando se ejecuta el
   generador, entonces el pico de memoria medido no supera 512 MiB y el resumen final lo muestra.
9. **AC-L9** — Dados en Linux unos restos de ordenación de una generación que ya no está en marcha y otros de
   una que sigue en marcha, cuando arranca una generación, entonces descarta los primeros con su mensaje y
   deja intactos los segundos.

## Decisiones

- **L1 — Linux de referencia**: Ubuntu 24.04 sobre WSL2 en el equipo de referencia. Motivo: es el Linux
  disponible en ese equipo, y WSL2 ejecuta un núcleo Linux real, así que lo que pasa ahí vale para un
  equipo Linux normal. La guía de Linux normal no se verifica en otro equipo.
- **L2 — Mismo resultado que en Windows**: en Linux, el generador no deja en la carpeta de salida nada que
  no dejaría en Windows. Motivo: el operador no debe tener que saber en qué sistema se generó un conjunto, y
  el generador nunca debe dejar archivos propios en carpetas ajenas.
- **L3 — Memoria en Linux**: el pico se mide como la memoria residente máxima del proceso, que el propio
  sistema guarda desde que el proceso arranca, igual que Windows guarda su pico de memoria comprometida. El
  resumen y el aviso usan el mismo texto que en Windows. Motivo: es la cifra más cercana que da Linux; incluye
  el runtime y las bibliotecas y excluye la caché de archivos, como pide el padre. Sus diferencias con la
  cifra de Windows son pequeñas y se documentan en las guías: incluye el código del propio programa y no
  cuenta la memoria que el sistema haya llevado al disco de intercambio.

## Referencias

- Documento padre: [chicago-crime.prd.md](chicago-crime.prd.md)
- [PRD del generador](generador.prd.md) y [PRD del consultor](consultor.prd.md): los dos dejaban Linux fuera
  de alcance. Al cerrar este cambio se anotan esos puntos, y el padre se anota con la medición de memoria en
  Linux.
