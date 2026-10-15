# Copias de trabajo en paralelo (worktrees) — PRD

**Versión**: 1.0
**Fecha**: 2026-10-15
**Documento padre**: ninguno

## Objetivo y contexto

Que el desarrollador pueda trabajar en varios cambios a la vez, cada uno en su propia copia del repositorio
con su propia sesión del agente, sin que una copia estropee, frene o contamine a otra, y que pueda ensayar
todo el proceso y tirarlo después sin dejar rastro en `main`.

**Por qué ahora**: para la charla de metodología se va a implementar la búsqueda por prefijo ("startswith")
del consultor dos veces a la vez a partir de un mismo PRD: una con vibe coding y otra con ODD, midiendo el
tiempo de cada una. El flujo decidido es:

1. Una copia para escribir el PRD, que se fusiona en la base.
2. Dos copias que parten de esa base, una para vibe coding y otra para ODD, cada una con su sesión nueva,
   trabajando a la vez.
3. Solo se fusiona la copia de ODD; la de vibe coding se descarta.

Antes de la demo real, el desarrollador ensaya el flujo completo sobre una base desechable (por ejemplo
`ensayo`) que hace el papel de `main`, y al terminar lo descarta todo. El ensayo y la demo usan los mismos
comandos; solo cambia la base.

Un worktree de git a secas no basta: no trae el CSV de referencia, comparte la memoria del agente con las
demás copias, las verificaciones de aceptación de dos copias chocan si usan la misma carpeta de trabajo, y la
regla actual de "fusionar en `main` al terminar" no se puede cumplir desde una copia ni es la correcta durante
un ensayo.

La idea se toma de las herramientas de copias de trabajo de otro proyecto (LandPro), adaptándola: aquí no hay
base de datos por copia ni rama de integración remota, y lo que hay que aislar es el CSV, las carpetas de
trabajo y la memoria.

### Estado de partida

Verificado el 2026-10-15 en el equipo de referencia, con el repositorio en el commit `e7efa04`:

| Aspecto | Situación |
|---|---|
| Copias de trabajo | Solo existe la copia principal. No hay herramientas para crear, listar, fusionar ni eliminar copias. |
| Ramas | Los cambios se han hecho en ramas `feat/<nombre>` y se han llevado a `main` casi siempre avanzándola (sin commit de fusión). |
| CSV de referencia | 2.406.401.982 bytes en la raíz de la copia principal, ignorado por git y sin protección contra escritura. Una copia nueva no lo tiene. |
| Precedente de pérdida de datos | En la ejecución del generador, apuntar el log a la entrada truncó el CSV. Ya se corrigió en el generador, pero muestra que una escritura accidental es posible. |
| Compilación | Cada copia compila en su propia carpeta de resultados; no hay ninguna configuración que la comparta. Una copia nueva compila desde cero. |
| Verificación de aceptación | Los scripts usan por defecto el CSV y los ejecutables de la copia en la que están, y exigen una carpeta de trabajo. El README usa carpetas fijas (por ejemplo `D:\acceptance`), así que dos copias que sigan el README chocan. |
| Archivos privados | Los permisos locales del agente y el registro de skills existen solo en la copia principal y git no los versiona. |
| Memoria Engram | Toda copia de este repositorio escribe en el proyecto `chicago-crime`, porque Engram lo deduce del remoto de git. Se puede asignar a una carpeta un proyecto propio y borrar un proyecto entero. La carpeta de esa configuración aún no está ignorada por git. |
| Memoria de Claude Code | Va por ruta de carpeta, no por repositorio: una copia nueva no carga la memoria de la principal, y una copia creada en una ruta ya usada recupera la memoria que quedó allí. |
| Regla de fusión | Las instrucciones del proyecto dicen "fusionar la rama en `main` tras la verificación completa". Desde una copia no se puede cambiar de rama a `main` (la tiene la copia principal), y en un ensayo no se debe tocar `main`. |
| Guardianes | El proyecto no tiene ningún guardián del agente (hook) ni configuración compartida del agente. |
| Raíz de copias | `C:\wt` ya existe y contiene copias de LandPro. |

## Actores

- **Desarrollador**: decide qué copias crear, abre una sesión nueva del agente en cada una, decide qué se
  fusiona y qué se descarta, y ensaya el flujo antes de la demo.
- **Agente en la copia principal**: crea, lista y elimina copias, y crea y descarta la base de ensayo. No
  trabaja en los cambios.
- **Agente en una copia**: trabaja en un único cambio dentro de su copia y, al terminar, lo fusiona en la base
  de esa copia. Nunca toca la copia principal ni otras copias.

## Flujo

### Nombres

De un único nombre de cambio `<n>` (minúsculas, dígitos y guiones simples) se deriva todo:

| Elemento | Valor |
|---|---|
| Rama | `feat/<n>` |
| Carpeta | `C:\wt\chicago-crime-<n>` |
| Proyecto Engram | `chicago-crime-<n>` |
| Carpeta de trabajo | la carpeta `work` dentro de la copia, ignorada por git |
| Etiqueta al descartar | `exp/<n>` |

### Crear una copia

1. Desde la copia principal, el desarrollador pide una copia para el cambio `<n>`, opcionalmente con una base
   distinta de `main` (por ejemplo `ensayo`).
2. Antes de crear nada se comprueba todo: que se está en la copia principal, que el nombre es válido y cabe en
   el límite de rutas de Windows, que la base existe y que no hay ya una rama, carpeta, copia registrada ni
   proyecto Engram con ese nombre. Si algo falla, no se crea nada.
3. Se crea la rama `feat/<n>` a partir de la base y la copia en su carpeta, y se registra cuál es su base.
4. La copia recibe el CSV de referencia como un enlace al de la copia principal (no ocupa espacio). El CSV
   queda marcado de solo lectura, lo que protege todos sus enlaces.
5. Se copian los permisos locales del agente y el registro de skills, se crea la carpeta de trabajo y se
   asigna a la copia su proyecto Engram propio.
6. Se comprueba que la copia está limpia para git y se abre una ventana nueva del editor en su raíz, donde el
   desarrollador inicia una sesión nueva del agente.
7. El resumen final muestra carpeta, rama, base, proyecto Engram y la frase con la que empezar.

Si un paso de la creación falla, se dice qué paso falló y qué quedó creado, y se limpia eliminando la copia;
la creación nunca deshace sus propios pasos.

### Trabajar en una copia

1. El agente trabaja como siempre (PRD, ODD o vibe coding), con commits en su rama.
2. Compila en la carpeta de resultados de su copia y usa la carpeta de trabajo de su copia para la
   verificación de aceptación y los conjuntos generados.
3. Lee y escribe memoria solo en el proyecto Engram de la copia.
4. Un guardián le impide escribir en la copia principal o en otras copias, y mover la base a mano.

### Fusionar una copia en su base

1. Desde la copia, el agente incorpora a su rama lo que la base tenga de nuevo, resuelve los conflictos con el
   desarrollador y vuelve a pasar las comprobaciones completas.
2. Pide la fusión. Se comprueba que la copia está limpia, que la rama contiene toda la base y que aporta algo
   nuevo.
3. La base avanza hasta la rama, sin que el agente salga de su copia. Si la base está abierta en otra copia,
   esa copia se actualiza también, salvo que tenga cambios sin confirmar: entonces la fusión se niega.
4. La copia y su rama se conservan después de fusionar. Nunca se publica nada en el remoto.

### Listar copias

Desde cualquier copia, se ven todas: nombre, rama, base, si la rama ya está fusionada en su base, carpeta y
proyecto Engram.

### Eliminar una copia

1. Desde la copia principal, el desarrollador pide eliminar la copia `<n>`.
2. Si la rama está fusionada en su base y la copia no tiene cambios sin confirmar, se eliminan la copia, su
   carpeta y su rama. El proyecto Engram y la memoria de Claude Code de esa carpeta se conservan.
3. Si la rama no está fusionada, se niega, salvo que se pida descartarla expresamente. Al descartar, se crea
   la etiqueta `exp/<n>` en la punta de la rama (su historia y su informe de cierre siguen accesibles) y
   después se eliminan la copia, la rama, el proyecto Engram y la memoria de Claude Code de esa carpeta.
4. Eliminar nunca fuerza nada ni cierra procesos: si un archivo está en uso, se dice cuál y se pide cerrar lo
   que lo usa. Se puede repetir sin riesgo.
5. El CSV de la copia principal no cambia nunca.

### Ensayar y descartar el ensayo

1. Desde la copia principal, el desarrollador crea la base de ensayo a partir de `main`.
2. Ejecuta el flujo completo con los mismos comandos que en la demo, indicando esa base al crear cada copia.
3. Al terminar, pide descartar el ensayo. En un solo paso se eliminan todas las copias cuya base es la de
   ensayo, sus ramas, sus etiquetas `exp/`, sus proyectos Engram, la memoria de Claude Code de sus carpetas y
   la propia base. `main` no cambia.

### Frases

| Frase del desarrollador | Sesión | Qué hace |
|---|---|---|
| "Crea un worktree para el cambio `<n>`" | principal | Crear con base `main` |
| "Crea un worktree para el cambio `<n>` sobre `<base>`" | principal | Crear con otra base |
| "¿Qué worktrees tengo?" | cualquiera | Listar |
| "Fusiona el cambio en la base" | copia | Fusionar |
| "Elimina el worktree `<n>`" | principal | Eliminar |
| "Descarta el worktree `<n>`" | principal | Eliminar descartando la rama |
| "Crea la base de ensayo `<base>`" | principal | Crear base de ensayo |
| "Descarta el ensayo `<base>`" | principal | Descartar el ensayo |

Si la frase se dice en la sesión equivocada, el agente dice dónde hay que pedirlo y se detiene.

## Validaciones y mensajes

| Situación | Regla | Mensaje al usuario |
|---|---|---|
| Crear, eliminar o gestionar el ensayo desde una copia | Se niega sin cambiar nada | `Esto se hace desde la copia principal, y esta sesión está en la copia '<carpeta>'. No se ha cambiado nada.` |
| Fusionar desde la copia principal | Se niega | `La fusión se hace desde la copia del cambio, no desde la copia principal. No se ha cambiado nada.` |
| Nombre con caracteres no válidos | Se niega | `Nombre '<n>' no válido: solo minúsculas, dígitos y guiones simples (por ejemplo, startswith-odd). No se ha creado nada.` |
| Nombre demasiado largo para las rutas de Windows | Se niega | `El nombre '<n>' tiene <x> caracteres; el máximo es <m> para que ninguna ruta pase de 259. No se ha creado nada.` |
| La base no existe | Se niega; nunca se crea una base implícitamente | `La base '<base>' no existe. Para un ensayo, créala antes con la frase de crear base de ensayo. No se ha creado nada.` |
| Ya existe rama, carpeta, copia registrada o proyecto Engram con ese nombre | Se niega | `Ya existe <qué> '<valor>'. No se ha creado nada.` |
| Un paso de la creación falla | Se detiene y dice qué quedó | `Falló el paso '<paso>': <causa>. Ya creado: <lista>. Corrige la causa y limpia con la frase de eliminar el worktree '<n>'.` |
| Fusionar con cambios sin confirmar | Se niega | `La copia tiene cambios sin confirmar: <lista>. Confírmalos o descártalos antes. No se ha cambiado nada.` |
| La base tiene commits que la rama no contiene | Se niega; se vuelve al paso 1 de la fusión una vez | `La base '<base>' tiene <x> commit(s) que tu rama no contiene. Incorpóralos, vuelve a verificar y repite la fusión.` |
| La rama no aporta nada a la base | Se niega | `La rama 'feat/<n>' no tiene nada que la base '<base>' no tenga. No se ha cambiado nada.` |
| La base está abierta en una copia con cambios sin confirmar | Se niega | `La base '<base>' está abierta en '<carpeta>' con cambios sin confirmar. Confírmalos o guárdalos allí y repite la fusión.` |
| Fusión correcta | La base avanza hasta la rama | `'<base>' avanza hasta 'feat/<n>' (<commit>). La copia y la rama se conservan.` |
| Eliminar una rama sin fusionar sin pedir descartarla | Se niega | `No se elimina '<n>': la rama tiene <x> commit(s) que no están en '<base>'. Para descartarla y conservar su historia en exp/<n>, pide descartar el worktree. No se ha eliminado nada.` |
| Eliminar con cambios sin confirmar | Se niega, también al descartar | `No se elimina '<n>'. Se perdería: <lista>. No se ha eliminado nada.` |
| Eliminar la copia en la que corre la sesión | Se niega | `Esta sesión está dentro de '<carpeta>'. Elimínala desde la copia principal.` |
| Un archivo de la copia está en uso | Se detiene sin forzar | `No se pudo borrar '<archivo>': está en uso. Cierra el editor, la aplicación o las pruebas que lo usan y repite. No se ha cerrado ningún proceso.` |
| Ya existe la etiqueta `exp/<n>` al descartar | Se niega | `Ya existe la etiqueta exp/<n>. No se ha eliminado nada.` |
| Descartar el ensayo con base `main` | Se niega | `'main' no es una base de ensayo. No se ha cambiado nada.` |
| Crear una base de ensayo que ya existe | Se niega | `La base '<base>' ya existe. No se ha creado nada.` |
| No hay copias | Se informa | `No hay worktrees (solo la copia principal).` |
| El agente de una copia intenta escribir en la copia principal u otra copia | El guardián lo bloquea | `Bloqueado: esta sesión trabaja en '<carpeta>' y no escribe fuera de ella.` |
| El agente de una copia intenta mover la base a mano | El guardián lo bloquea | `Bloqueado: la base '<base>' solo avanza con la fusión del worktree.` |
| Escritura sobre el CSV de referencia desde cualquier copia | El sistema la rechaza por ser de solo lectura | (el error habitual de la aplicación o del sistema) |

### Riesgos aceptados

- **El CSV es uno solo.** Todas las copias leen el mismo archivo. El solo lectura impide escribirlo por
  accidente, pero quien le quite ese atributo a mano lo deja expuesto en todas las copias.
- **La primera compilación de cada copia es completa.** Tarda lo mismo en todas las copias, así que no
  distorsiona la comparación entre ellas, pero cuenta en el tiempo de cada ejecución.
- **El guardián solo actúa en sesiones abiertas en la raíz de una copia.** Una sesión abierta en una
  subcarpeta no lo carga.

## Documentación

- **Instrucciones del proyecto**: un apartado breve sobre copias. En una copia, "fusionar en `main`" pasa a
  ser "fusionar en la base de la copia, solo con la fusión del worktree"; la carpeta de trabajo de la
  verificación de aceptación es la de la copia; el agente nunca usa los worktrees integrados de Claude Code.
- **Skill de worktrees**: una tabla que lleva cada frase a su operación y dice en qué sesión se pide, sin
  lógica propia.
- **README**: un apartado corto sobre cómo trabajar con copias, que enlaza a la guía.
- **Guía de uso**: el flujo completo, incluido el ensayo, y una tabla "mensaje → qué hacer" con todos los
  mensajes de esta sección.

## Reutilización

- Las ideas de las herramientas de LandPro y de la guía genérica de desarrollo en paralelo del repositorio:
  un nombre del que se deriva todo, comprobar antes de crear, nunca forzar al eliminar, y mensajes que dicen
  qué hacer.
- Los scripts de aceptación actuales, sin cambios: ya usan el CSV y los ejecutables de la copia en la que
  están y aceptan cualquier carpeta de trabajo.
- Los mecanismos existentes de Engram para asignar un proyecto a una carpeta y borrar un proyecto.
- El flujo actual de trabajo (PRD, ODD, vibe coding, informe de cierre), que no cambia dentro de una copia.

## Fuera de alcance

- Linux y otros sistemas: las herramientas son solo para Windows.
- Publicar ramas en el remoto, abrir PR o fusionar en el remoto.
- Copiar el informe de cierre o las métricas de una rama descartada a la base: quedan en su etiqueta `exp/<n>`.
- Compartir resultados de compilación entre copias o compilar al crear la copia.
- Cambiar los scripts de aceptación o el comportamiento del generador y del consultor.
- Conservar el historial de sesiones de Claude Code: eliminar o descartar solo borra su memoria.
- El experimento de "startswith" en sí: este cambio solo prepara las herramientas.

## Criterios de aceptación

1. **AC-W1** — Dado un nombre válido y la copia principal, cuando se crea una copia sin indicar base,
   entonces existen la rama `feat/<n>` a partir de `main`, la carpeta `C:\wt\chicago-crime-<n>`, el CSV
   enlazado con el mismo tamaño que el original, los permisos locales y el registro de skills, la carpeta de
   trabajo, el proyecto Engram `chicago-crime-<n>` y la base registrada, y git ve la copia limpia.
2. **AC-W2** — Dada una base de ensayo existente, cuando se crea una copia sobre ella, entonces su rama parte
   de esa base y el listado la muestra como base.
3. **AC-W3** — Dado un nombre no válido, demasiado largo, ya usado o una base inexistente, cuando se pide crear
   la copia, entonces se muestra el mensaje correspondiente y no se crea ninguna rama, carpeta, copia
   registrada ni proyecto Engram.
4. **AC-W4** — Dada una copia, cuando se escribe en su CSV, entonces la escritura falla y el CSV de la copia
   principal conserva su tamaño y contenido.
5. **AC-W5** — Dadas dos copias, cuando cada una compila y ejecuta la verificación de aceptación con su
   carpeta de trabajo a la vez, entonces ninguna bloquea ni altera los resultados de la otra.
6. **AC-W6** — Dadas dos copias, cuando el agente de una guarda una memoria en Engram, entonces el agente de
   la otra no la encuentra, y tampoco la encuentra la copia principal.
7. **AC-W7** — Dada una copia con su rama al día con la base, cuando se fusiona, entonces la base avanza hasta
   la rama, la copia principal (si tiene la base abierta y limpia) queda actualizada, y la copia y la rama se
   conservan.
8. **AC-W8** — Dada una base con commits que la rama no contiene, o una copia con cambios sin confirmar, o
   una base abierta en una copia con cambios sin confirmar, cuando se pide fusionar, entonces se muestra el
   mensaje correspondiente y la base no cambia.
9. **AC-W9** — Dada una sesión del agente en una copia, cuando intenta escribir en la copia principal u otra
   copia, o mover la base con git, entonces el guardián lo bloquea con su mensaje; y en la copia principal el
   guardián no bloquea nada.
10. **AC-W10** — Dada una copia fusionada y limpia, cuando se elimina, entonces desaparecen la copia, su
    carpeta y su rama; se conservan su proyecto Engram y la memoria de Claude Code; y el CSV de la copia
    principal no cambia.
11. **AC-W11** — Dada una copia sin fusionar, cuando se pide eliminarla sin descartarla, entonces se niega y
    no cambia nada; cuando se pide descartarla, entonces existe la etiqueta `exp/<n>` en la punta de su rama
    y desaparecen la copia, la rama, el proyecto Engram y la memoria de Claude Code de su carpeta.
12. **AC-W12** — Dado un ensayo completo (PRD fusionado en la base de ensayo, dos copias desde ella, una
    fusionada y otra descartada), cuando se descarta el ensayo, entonces no queda ninguna copia, rama,
    etiqueta `exp/`, proyecto Engram ni memoria de Claude Code del ensayo, la base de ensayo ya no existe y
    `main` está en el mismo commit que antes de empezar.
13. **AC-W13** — Dado el flujo de la demo, cuando se ejecuta con base `main`, entonces se usan exactamente
    las mismas frases que en el ensayo, cambiando solo la base al crear cada copia.
14. **AC-W14** — Dada la creación o eliminación pedida desde una copia, o la fusión pedida desde la copia
    principal, entonces se niega con su mensaje sin cambiar nada.
15. **AC-W15** — Dada la guía de uso, cuando un desarrollador la sigue, entonces puede hacer el ensayo
    completo sin más ayuda, y cada mensaje de este documento aparece en su tabla "mensaje → qué hacer".

## Decisiones

- **W1 — Ubicación**: este PRD abre un dominio propio de herramientas de desarrollo, sin padre, separado de
  los PRD del producto. Las instrucciones del proyecto dicen que las herramientas no necesitan PRD; el
  desarrollador eligió PRD + ODD para este cambio.
- **W2 — Nombres**: rama `feat/<n>`, como las ramas que ya tiene el repositorio, y carpeta
  `C:\wt\chicago-crime-<n>`, con el prefijo del proyecto para distinguirla entre ventanas.
- **W3 — CSV**: enlace al CSV de la copia principal, marcado de solo lectura. Motivo: no ocupa espacio ni
  tiempo, los scripts de aceptación funcionan sin cambios y el solo lectura protege todas las copias a la vez.
- **W4 — Carpeta de trabajo**: dentro de cada copia e ignorada por git. Motivo: dos copias no pueden chocar y
  desaparece con la copia.
- **W5 — Descartar**: una rama sin fusionar solo se elimina si se pide descartarla, y antes se etiqueta como
  `exp/<n>`. El informe de cierre no se copia a la base.
- **W6 — Memoria**: un proyecto Engram por copia, para que las sesiones de vibe coding y de ODD no se vean
  entre sí durante la carrera. La memoria de Claude Code se borra al descartar porque el ensayo y la demo usan
  los mismos nombres y, por tanto, las mismas carpetas.
- **W7 — Ensayo**: la base de ensayo se crea y se descarta con frases propias; crear una copia sobre una base
  inexistente falla, para que un error al escribir no cree otra base.
- **W8 — Guardián**: en una copia, bloquea escribir en la copia principal o en otras copias y mover la base a
  mano. Tiene sus propias pruebas.
- **W9 — Fusión**: la base avanza hasta la rama sin commit de fusión, como se ha hecho hasta ahora en `main`;
  por eso la rama debe contener antes toda la base. Si la base está abierta en una copia con cambios sin
  confirmar, la fusión se niega para no mezclar esos cambios.
- **W10 — Solo Windows**: como los scripts de aceptación; la demo y el ensayo se hacen en Windows.

## Referencias

- [Guía genérica de desarrollo en paralelo con worktrees](../../methodology/parallel-developing.md).
- PRD de copias de trabajo de LandPro y sus herramientas, de donde se toman las ideas.
- [Instrucciones del proyecto](../../../CLAUDE.md): flujo de trabajo, fusión tras verificación y cierre. Al
  cerrar este cambio se anotan con la regla de fusión en una copia.

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| bd359d3a | 2026-10-15 11:22 UTC+01:00 | 21 min | 0 min | 21 min | 6 min | 15 min | 10 |

<!-- prd-time: [{"session": "bd359d3a-70a8-430d-813a-307fe28bf9dc", "start": "2026-10-15T10:22:00.662000+00:00", "wall": 1237, "discarded": 0, "agent": 337, "human": 899, "interventions": 10, "idle_minutes": 10}] -->
