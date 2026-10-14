# Soporte para Linux terminado: verificado y mergeado a `main`

Informe de cierre del feature `soporte-linux` (2026-10-14). El detalle completo (specs, decisiones, log y
resultados) está en [soporte-linux.md](soporte-linux.md), y el producto en el
[PRD de soporte para Linux](../../docs/prd/chicago-crime/soporte-linux.prd.md).

`main` está en `171eacf` (merge fast-forward de `feat/soporte-linux`). **No se hizo ningún push.** Siguen sin
subir los 14 commits del feature, más el de este informe.

## Resultado

- **Especificación:** S1–S5 entregados. AC-L1…AC-L9 observados.
- **Tests:**
  - Windows: 425/425 en verde. La base eran 423, no 424: un test solo existía fuera de Windows. Ninguno
    de los 423 desapareció ni dejó de pasar.
  - Linux (Ubuntu 24.04 sobre WSL 2): 429/429. Al empezar eran 407 en verde y 13 en rojo.
  - Clippy sin avisos en los dos sistemas.
- **Aceptación en el CSV de referencia, en WSL:**
  - Generación completa en 1 min 32 s, con un pico de memoria de 103,5 MiB (límite 512 MiB).
  - El verificador independiente con `--reference` da PASS en sus 18 comprobaciones.
  - Dos generaciones a la vez: la segunda se rechaza y la primera termina con normalidad.
  - Ctrl+C borra la carpeta de salida que había creado esa ejecución.
  - Tras un `kill -9`, la siguiente ejecución descarta sus restos y respeta los de un proceso vivo.
- **Revisiones:** 7 revisiones RDD, todas aprobadas y reconocidas, con el consentimiento siempre concedido.
  Cuatro se escalaron a riesgo alto: el bloqueo de la carpeta de salida, la detección de procesos
  terminados, el bloqueo de las carpetas de ordenación y el borrado del archivo de bloqueo antiguo.
- **Verificador independiente:** S1–S5 pasan. Su único blocker era una cita errónea de un mensaje en el
  PRD, solo de documentación, y ya está corregida.
- **Hallazgos corregidos por el camino:**
  - En Linux, el archivo `.generator.lock` se quedaba en la carpeta de salida. Esa era la causa de los 13
    tests en rojo. Ahora se bloquea la propia carpeta y no se crea ningún archivo.
  - La detección por `/proc` no ve los procesos de otro contenedor, ni los de `/proc` oculto. Ahora cada
    ordenación en marcha bloquea su carpeta, y una carpeta bloqueada nunca se borra.
  - Entre crear una carpeta de ordenación y bloquearla quedaba un instante. Si otro proceso la toma en
    ese hueco, se crea otra.
  - Un test fallaba 1 de cada 5 veces en paralelo: entre su `fork` y su `exec`, un proceso hijo compartía
    el bloqueo. Ahora esos tests se excluyen entre sí: 0 fallos en 300 ejecuciones.
  - En Unix, un `.generator.lock` dejado por una versión anterior ahora se descarta como resto.
  - El PRD daba dos cifras mal, ya corregidas: 424 tests en Windows (son 423) y el mensaje de restos de
    ordenación (es `Se han descartado carpetas temporales de una generación interrumpida anterior.`).

## Tiempo

- Inicio a las 07:27:41 y cierre a las ~08:15: **~48 min reales**, frente a una estimación de 1,5–2,5 h. No
  hubo esperas por el usuario.
- Fue más rápido de lo estimado porque los cambios de código eran pequeños, y porque las revisiones, el
  verificador y la aceptación con el CSV completo corrieron de fondo mientras avanzaban otras tareas.

## Informe de cambios (`98c5b4d..main`)

| Área | + | − | Archivos |
|---|---|---|---|
| `crates/crime-generator` | 387 | 52 | 7 |
| `docs/linux` | 182 | 0 | 2 |
| `docs/prd` | 157 | 5 | 6 |
| `odd/tasks` | 151 | 0 | 1 |
| `README.md` | 23 | 4 | 1 |
| **Total** | **900** | **61** | **17** |

No cambian `Cargo.toml` ni `Cargo.lock`: no hay dependencias nuevas. En el generador, 185 de las líneas
añadidas están en `tests/`, sin contar los tests unitarios dentro de `src`.

## Resumen

El proyecto compila y pasa todos los tests en Linux igual que en Windows, y Windows no cambia de
comportamiento:

- **Bloqueo:** en Unix, el generador bloquea la carpeta de salida en sí, con `flock` sobre el directorio, en
  lugar de crear dentro un archivo de bloqueo. La carpeta queda igual que en Windows tras una ejecución
  correcta, fallida o cancelada.
- **Memoria:** el pico se lee de `VmHWM`, la memoria residente máxima que guarda el propio kernel, y se
  muestra con el mismo texto y el mismo aviso que en Windows.
- **Restos de ordenación:** se descartan los de las ejecuciones terminadas. Para saber cuáles lo están, se
  consulta `/proc/<pid>/stat` y se comprueba el bloqueo que cada ordenación en marcha tiene sobre su carpeta.
- **Documentación:** el README tiene un apartado de dos párrafos que enlaza la
  [guía de WSL](../../docs/linux/wsl.md) y la [guía de Linux nativo](../../docs/linux/native.md). Las dos
  guías explican qué difiere de Windows.

## Valores por defecto elegidos sin consulta

- **Linux de referencia:** Ubuntu 24.04 sobre WSL 2 (decisión L1). La guía de Linux nativo no se ha
  verificado en otro equipo.
- **Memoria en Linux:** memoria residente máxima (L3). Incluye el código del programa y no cuenta la memoria
  llevada al disco de intercambio.
- **Bloqueo en Unix:** `flock` sobre la carpeta. Se descartó borrar el archivo de bloqueo antes de soltarlo,
  porque dejaba una carrera abierta.
- **Documentación en inglés,** como el README. El PRD está en español, como los anteriores.

Todo está documentado en [soporte-linux.md](soporte-linux.md).

## Pendientes (no bloqueantes)

- **Reintento de la carpeta de ordenación:** no tiene test. Forzar esa carrera exige un enganche de test en
  el código de producción.
- **Versiones mezcladas:** una versión antigua y una nueva corriendo a la vez en Linux sobre la misma
  carpeta no se excluyen, porque bloquean objetos distintos.
- **Dos ejecuciones en el mismo segundo:** chocan en el nombre del log por defecto. Ya pasaba antes de este
  cambio.
- **Scripts de aceptación:** siguen siendo solo para Windows, porque son PowerShell con APIs de Windows. En
  Linux se puede usar el verificador independiente directamente.

**Riesgo:** ítem 4. Cambia el bloqueo entre generaciones y la decisión de borrar carpetas de otro proceso.
Está cubierto por tests en los dos sistemas, revisiones de riesgo alto, el verificador independiente y la
aceptación con datos reales.

## Limpieza manual

Quedan copias que se pueden borrar si ya no hacen falta:

- **En WSL:** `~/acc` (~1,1 GB, conjuntos de la aceptación), `~/chicago-crime` (~1,8 GB, clon de trabajo con
  `target`) y `~/data` (~2,3 GB, copia del CSV de referencia).
- **En Windows:** el scratchpad de la sesión (~2,4 GB, clones del verificador independiente).
