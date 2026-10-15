# Herramientas de worktrees terminadas: verificadas y mergeadas a `main`

Informe de cierre del feature `worktree-tooling` (2026-10-15). Enfoque de ejecución: ODD. El detalle completo
(specs, decisiones, log y evidencias) está en [worktree-tooling.md](worktree-tooling.md), y el producto en el
[PRD de copias de trabajo en paralelo](../../docs/prd/worktrees/worktrees.prd.md).

`main` avanza por fast-forward hasta la rama `feat/worktree-tooling` (7 commits del feature más el de este
informe). **No se hizo ningún push.**

## Resultado

- **Especificación:** S1–S9 entregados. AC-W1…AC-W15 en PASS según el verificador independiente.
- **Tests:**
  - Scripts: 42/42 en verde (`scripts/worktree/tests/Test-Worktree.ps1`). Al terminar el escritor eran 39;
    las 3 nuevas cubren hallazgos del verificador y de las revisiones.
  - Guardián: 31/31 (`python -m unittest discover -s .claude/hooks`). El escritor entregó 29.
  - Sin cambios en Rust: `cargo fmt --all --check` OK, `cargo clippy --workspace --all-targets -- -D warnings`
    sin avisos, `cargo test --workspace` con 425 pasadas y 0 fallidas. Los 16 tests de `closing-report`
    siguen en verde.
- **Aceptación con datos reales** (verificador, clon temporal del repositorio con el CSV de referencia enlazado):
  - Ensayo completo con los pasos 0–11 de la guía de prompts. Al descartarlo no queda ninguna copia, rama,
    etiqueta `exp/`, proyecto Engram, memoria de Claude Code ni base de ensayo, y `main` no cambia (AC-W12).
  - AC-W5: dos copias compilaron en release a la vez (12:54:49–12:56:09, 80 s cada una). Después pasaron a la
    vez la aceptación del generador sobre el CSV completo (12:56:22–13:05:58, 576 s cada una), con 15/15 PASS
    en cada copia y sin interferencias.
  - AC-W4: las escrituras al CSV de una copia fallan desde bash, Python y PowerShell. El CSV real conserva sus
    2.406.401.982 bytes y su SHA-256, y queda de solo lectura.
  - AC-W6: una memoria guardada en una copia no aparece desde otra copia ni desde la copia principal.
- **Revisiones:** 3 revisiones RDD, todas aprobadas y reconocidas, con el consentimiento siempre concedido:
  - T0+T2+T3: riesgo alto, 3 lentes.
  - T1 con sus correcciones: escalada a riesgo alto (ítem 1), 4 lentes.
  - Las últimas correcciones: tramo medio por debajo del umbral, revisado al cierre por regla del usuario, 3
    lentes.
- **Verificador independiente:** ningún blocker y 11 avisos (L8 del documento).
- **Hallazgos corregidos por el camino** (todos con su prueba en RED antes de la corrección):
  - **Pérdida de datos:** una carpeta sin `.git` en `C:\wt\chicago-crime-<n>` creada por otra persona se borraba
    entera al pedir eliminar ese nombre. Ahora solo se borra como resto de una eliminación a medias mientras
    la copia conserva su base registrada. Si no, se rechaza con el mensaje de carpeta ajena.
  - **Guardián:**
    - `git checkout ENSAYO` se saltaba el guardián, y en Windows acababa moviendo la base. Ahora los nombres
      se comparan sin distinguir mayúsculas.
    - Una redirección con espacio (`git checkout ensayo > nul`) también se lo saltaba.
    - Si `git` no se podía ejecutar, la base de ensayo quedaba sin proteger. Ahora el guardián lee el archivo
      de configuración directamente.
  - **Fusión:** si la base avanza en la copia que la tiene abierta durante la fusión, el script sale con 3
    (volver al paso 1) en lugar de 1.
  - **Descarte del ensayo:** comprueba que su sección de configuración desaparece de verdad.
  - **Mensajes:** la causa de un paso fallido se limita a la primera línea, con un máximo de 200 caracteres (antes
    arrastraba toda la ayuda de la herramienta). Un nombre no válido al eliminar dice "No se ha eliminado nada".
  - **Fusión al cerrar una copia:** con las instrucciones iniciales, las ejecuciones de vibe coding y de ODD se
    habrían fusionado las dos. Ahora una copia cierra sin fusionar y solo fusiona cuando el desarrollador dice
    `Fusiona el cambio en la base`.

## Cobertura de criterios de aceptación

La trazabilidad completa está en el documento del feature (T1–T4, L8). Resumen:

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-W1, AC-W2, AC-W3, AC-W14 | Test-Worktree.ps1 (creación y negativas) + verificador | PASS |
| AC-W4 | Test-Worktree.ps1 + verificador (4 formas de escritura) | PASS |
| AC-W5 | Verificador: compilaciones y aceptación simultáneas en dos copias | PASS (80 s y 576 s; 15/15 cada una) |
| AC-W6 | Test-Worktree.ps1 + verificador con el CLI real de Engram | PASS |
| AC-W7, AC-W8 | Test-Worktree.ps1 (salidas 0, 3 y 1) + verificador | PASS |
| AC-W9 | test_worktree_guard.py (31 tests) + verificador (62 casos) | PASS |
| AC-W10, AC-W11 | Test-Worktree.ps1 + verificador | PASS |
| AC-W12, AC-W13 | Test-Worktree.ps1 (ensayo completo) + verificador (pasos 0–11) | PASS |
| AC-W15 | Comprobación: 22/22 mensajes del PRD en la tabla de la guía | PASS |

## Tiempo

- Estimación: 2–3,5 h. Inicio real: 2026-10-15 11:59:02. Cierre: 13:28:13, antes de escribir este informe y del
  merge.
- **Duración activa: 1 h 29 min**, frente a una estimación de 2–3,5 h. Fue un 26–58 % más rápido de lo
  estimado, como en las dos ejecuciones anteriores. Las causas principales: los tres escritores trabajaron en
  paralelo y las revisiones corrieron de fondo mientras avanzaban otras tareas. La aceptación real de AC-W5
  (unos 11 min) también corrió dentro del verificador.
- Esperas por el usuario: ninguna.
- Aparte: el PRD se elaboró en 21 min activos en la misma sesión, antes del ODD. Está medido en el propio PRD.

## Informe de cambios (`e7efa04..HEAD`, sin este informe)

| Área | + | − | Archivos |
|---|---|---|---|
| `scripts` | 2.280 | 0 | 8 |
| `docs` | 1.047 | 0 | 4 |
| `.claude` | 969 | 0 | 4 |
| `odd` | 197 | 0 | 1 |
| `CLAUDE.md` | 20 | 1 | 1 |
| `README.md` | 17 | 0 | 1 |
| `.gitignore` | 6 | 0 | 1 |
| **Total** | **4.536** | **1** | **20** |

No hay archivos generados ni lockfiles. Las pruebas suman 1.348 de las líneas añadidas: 911 en
`scripts/worktree/tests` y 437 en `.claude/hooks/test_worktree_guard.py`. `docs` incluye el PRD en español y en
inglés y la guía de prompts. No había previsión de líneas.

## Resumen

El desarrollador puede trabajar en varios cambios a la vez, cada uno en su copia `C:\wt\chicago-crime-<n>` con
su rama `feat/<n>` y su propia sesión del agente, pidiéndolo con frases en español que la skill `worktree`
lleva a seis scripts de PowerShell:

- **Crear:** la copia recibe un enlace duro al CSV de referencia, que queda de solo lectura, sus archivos
  privados, una carpeta `work` para la aceptación y un proyecto Engram propio. Su base queda registrada.
- **Fusionar:** la base avanza hasta la rama sin commit de fusión ni push.
- **Eliminar y descartar:** el descarte etiqueta la rama como `exp/<n>` y borra su memoria.
- **Ensayar:** una base de ensayo se crea y se descarta con todo lo que cuelga de ella en un solo paso.

Un guardián de Claude Code impide que la sesión de una copia escriba en la copia principal o en otras copias,
y que mueva a mano la base o `main`. La guía de uso, el README, CLAUDE.md y la guía de prompts explican el flujo,
incluido el del experimento "startswith".

## Valores por defecto elegidos sin consulta

- **Detalles técnicos** del documento del feature ("Implementation decisions"):
  - scripts para Windows PowerShell 5.1 guardados en UTF-8 con BOM;
  - base registrada en la configuración compartida de git, con la lista de nombres creados por base;
  - límite de longitud del nombre;
  - borrado del enlace al CSV quitando y restaurando el solo lectura;
  - guardián en Python;
  - documentación en inglés.
- **`main` también está protegido por el guardián** aunque no sea la base, para que un ensayo nunca la toque.
- **Una copia cierra sin fusionar:** la fusión solo ocurre con la frase del desarrollador (L3), porque el PRD dice
  que el desarrollador decide qué se fusiona.
- **Desviaciones aceptadas de los escritores** (L3, L6):
  - alias de PowerShell como comandos de solo lectura;
  - más formas de git que mueven la base;
  - marca de base de ensayo;
  - salto de nombres reutilizados;
  - 16 mensajes extra en el estilo del PRD, documentados en la guía.

## Pendientes (no bloqueantes)

- **El guardián es textual (R1-001, A2, A3):** se lo saltan las rutas relativas, las variables, los shells
  anidados, los alias de git y los nombres cortos 8.3. Es una red de seguridad, no una barrera. El PRD ya lo
  acepta para las sesiones abiertas fuera de la raíz.
- **Carreras sin test:** el exit 3 en la carrera de la fusión y el fallo al limpiar la configuración del ensayo
  no tienen test, porque forzarlos exige provocar una carrera. El código comparte la comprobación con la ruta
  que sí está probada.
- **Sugerencias de la última revisión:** `merge-base` con un error de git se lee como "la base avanzó"; falta un
  test del caso positivo de los restos sin `.git`, aunque lo cubre el test de "archivo en uso, repetir
  completa"; y la limpieza del test de nombre reutilizado no está en un `finally`.
- **Mensajes mejorables:** el resumen de creación propone un siguiente paso genérico (A7), y una eliminación
  normal menciona el proyecto Engram aunque nunca se creara (A6).
- **`worktreebase.main.created`** crece sin límite (A8). Son unas pocas líneas por copia.
- **El descarte del ensayo actúa por nombre (A9):** la protección es el orden documentado, descartar el ensayo
  antes de la demo.
- **Lógica duplicada (R2-dup-copy-plan):** las comprobaciones por copia están en dos scripts. Las dos rutas
  tienen test.
- **El registro privado de skills** (`.atl/skill-registry.md`, fuera de git) no indexa todavía la skill
  `worktree`. Hay que regenerarlo con `skill-registry` antes del ensayo, porque las copias lo heredan.
- **Fuera del cambio:** la salida de `run-acceptance.ps1` sale mal codificada cuando se redirige (A11).

**Riesgo:** ítems 1 y 2.

- **Ítem 1:** los scripts borran copias, ramas, etiquetas, proyectos Engram y memoria de Claude Code, cambian
  el atributo del CSV compartido y mueven refs.
- **Ítem 2:** el guardián es un control de seguridad.

Están cubiertos por 42 + 31 tests, tres revisiones RDD aprobadas y un verificador independiente con un ensayo
y una aceptación reales.

## Notas de entorno

- En el repositorio real existe la copia `C:\wt\chicago-crime-start-confirmation` (`feat/start-confirmation`),
  creada a las 12:10 por otra sesión del usuario. No se tocó.
- El CSV de referencia de la copia principal quedó marcado de solo lectura. Es el diseño (W3) y no hay que
  quitarlo.
