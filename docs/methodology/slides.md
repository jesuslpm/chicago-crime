---
marp: true
theme: default
paginate: true
title: Desarrollo práctico de software asistido por IA
style: |
  @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;600;800&family=JetBrains+Mono:wght@400;600&display=swap');
  section {
    font-family: 'Inter', 'Segoe UI', system-ui, sans-serif;
    font-size: 26px;
    color: #1f2937;
    background: #f6f7fb;
    padding: 50px 80px 80px 80px;
    place-content: safe start start;
    border-top: 14px solid transparent;
    border-image: linear-gradient(90deg, #4f46e5, #06b6d4) 1;
  }
  h2 {
    color: #312e81;
    font-weight: 800;
    border-left: 8px solid #06b6d4;
    padding-left: 18px;
    margin-top: 10px;
    margin-bottom: 24px;
  }
  strong { color: #312e81; }
  a { color: #0e7490; text-decoration: none; border-bottom: 1px dotted #0e7490; }
  a code { color: #0e7490; background: #e0f7fa; }
  a strong { color: inherit; }
  li::marker { color: #06b6d4; font-weight: 800; }
  table { border-collapse: collapse; font-size: 22px; box-shadow: 0 4px 14px rgba(49, 46, 129, 0.12); }
  th { background: #312e81; color: #ffffff; border: none; padding: 10px 16px; }
  td { background: #ffffff; border: none; border-bottom: 1px solid #e0e7ff; padding: 10px 16px; }
  tr:nth-child(even) td { background: #eef2ff; }
  blockquote {
    border-left: 6px solid #06b6d4;
    background: #ecfeff;
    color: #164e63;
    padding: 14px 24px;
    border-radius: 8px;
    font-size: 30px;
  }
  code { font-family: 'JetBrains Mono', Consolas, monospace; background: #e0e7ff; color: #3730a3; border-radius: 4px; padding: 0 6px; }
  pre { background: #1e1b4b; border-radius: 10px; padding: 16px 22px; }
  pre code { background: transparent; color: #e0e7ff; padding: 0; }
  em { color: #374151; }
  section::after { color: #6b7280; font-weight: 600; }
  .detalle { position: absolute; bottom: 28px; left: 80px; font-size: 18px; color: #6b7280; }
  section.lead {
    background: linear-gradient(135deg, #1e1b4b 0%, #312e81 55%, #0e7490 100%);
    color: #e0e7ff;
    border: none;
    place-content: safe center center;
  }
  section.lead h1 { color: #ffffff; font-size: 58px; font-weight: 800; line-height: 1.15; }
  section.lead strong { color: #67e8f9; }
  section.lead a { color: #67e8f9; border-bottom-color: #67e8f9; }
  section.lead li::marker { color: #67e8f9; }
  section.lead::after { color: #a5b4fc; }
---

<!-- _class: lead -->

# Desarrollo práctico de software asistido por IA

**PRD + vibe coding · PRD + SDD · PRD + ODD**

Un experimento real: [chicago-crime](https://github.com/jesuslpm/chicago-crime)

<!--
Duración total prevista: 60 min (unos 3 min por slide). Documento de detalle en la otra pantalla:
desarrollo-asistido-por-ia.md. Ejemplo paso a paso: generacion-del-prd.md.
-->

---

## La tesis

> El PRD es la **fuente permanente de verdad funcional**.
> Especificaciones, diseño, tareas y código se derivan de él.

| Capa | Qué es | Carácter |
|---|---|---|
| Requisitos | El PRD, escrito para personas | Permanente |
| Ejecución | Vibe coding, SDD u ODD | Variable |
| Evidencia | Tests, criterios, revisiones, informe | Imprescindible |

<span class="detalle">→ Detalle: [sección 1](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#1-introducción)</span>

<!--
Mensaje clave: no es "la IA sustituye ingenieros". Es que un ingeniero con dominio, algoritmos y arquitectura
puede dirigir agentes en una tecnología que todavía no domina, y sigue siendo responsable de especificar y
evaluar.
-->

---

## El experimento: chicago-crime

- CSV de delitos de Chicago: **8.656.904 registros, 2,4 GB**
- Objetivo: convertirlo en SSTables consultables, en **Rust**, un lenguaje que el autor no conocía
- Tres cambios, cada uno con su PRD: [**generador**](https://github.com/jesuslpm/chicago-crime/blob/main/docs/prd/chicago-crime/generador.prd.md), [**consultor**](https://github.com/jesuslpm/chicago-crime/blob/main/docs/prd/chicago-crime/consultor.prd.md), [**soporte Linux**](https://github.com/jesuslpm/chicago-crime/blob/main/docs/prd/chicago-crime/soporte-linux.prd.md)

| | Tests | Aceptación | Dato destacado |
|---|---|---|---|
| Generador | 230 | 14/14 | 93 MiB de pico (límite 512) |
| Consultor | 423 | 58/58 | Consulta por ID en 39–51 ms |

<span class="detalle">→ Detalle: [sección 11](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#11-evidencia-del-experimento)</span>

<!-- Aviso honesto: es un caso real, no una comparación controlada entre metodologías. -->

---

## Qué es un PRD

- **Qué** hace el producto y **por qué**, en lenguaje de negocio
- **No** lleva clases, endpoints, rutas ni números de línea: eso es el *cómo*
- No es un prompt desechable: es **documentación duradera**, versionada en el repositorio
- **Jerárquico**: un [PRD padre](https://github.com/jesuslpm/chicago-crime/blob/main/docs/prd/chicago-crime/chicago-crime.prd.md) por aplicación y un [PRD hijo](https://github.com/jesuslpm/chicago-crime/tree/main/docs/prd/chicago-crime) por cambio
- Si un hijo contradice al padre, el padre se **anota**, no se reescribe

<span class="detalle">→ Detalle: [sección 2](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#2-el-prd)</span>

<!-- Mostrar en la otra pantalla docs/prd/chicago-crime/: el padre y sus tres hijos. -->

---

## Las secciones que más pesan

| Sección | Por qué importa |
|---|---|
| **Estado de partida** | Verificado en el código y en los datos, nunca supuesto |
| **Validaciones y mensajes** | El texto exacto que verá el usuario |
| **Fuera de alcance** | Obligatorio: es donde empieza el próximo cambio |
| **Criterios de aceptación** | Con cifras reales: `THEFT` = 1.838.502 |
| **Decisiones** | Cada una con su motivo y su consecuencia aceptada |

<span class="detalle">→ Detalle: [sección 2](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#2-el-prd)</span>

<!-- "Decisiones" no estaba en la plantilla original: nació en el experimento porque resultó la más útil. -->

---

## Cómo se produce: conversando con la IA

1. El programador **pide el PRD** de un cambio y **explica el cambio**
2. La IA lee el PRD padre y la documentación
3. **Mide** el estado de partida en el código y los datos
4. Borrador con decisiones abiertas, **cada una con recomendación**
5. Se cierran las decisiones **de una en una**
6. La IA **verifica** antes de dar la razón
7. Iterar hasta que no quede nada abierto
8. Escribir el PRD **no autoriza** a implementar

<span class="detalle">→ Detalle: [sección 2](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#2-el-prd) · [ejemplo completo](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/generacion-del-prd.md)</span>

<!--
El skill prd fija estas reglas. Ejemplo de medición: recorrer el CSV encontró que no venía ordenado, que el
número de caso se repetía y 253 horas inexistentes por el cambio de horario. Las tres cambiaron decisiones.
-->

---

## Ejemplo real: la IA verifica

| Decisión | El usuario dijo | La IA comprobó | Resultado |
|---|---|---|---|
| D1 Formato | "el paquete sstable es compatible con LevelDB" | Leyó el código fuente del crate | **Tenía razón** |
| D4 Valor | Guardar el ID "ahorra memoria" | Midió: +33 MiB, 2,9 % | **A medias**: más simple, pero no ahorra RAM |
| D5 Duplicados | "Detectar duplicados lejanos es difícil" | Tras la ordenación externa quedan contiguos | **Premisa incorrecta** |

En los tres casos la evidencia fueron **código y números**, no opiniones.

<span class="detalle">→ Detalle: [Cómo se generó el PRD, sección 4](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/generacion-del-prd.md#4-las-decisiones-una-por-una)</span>

<!-- Leer las citas literales del documento: muestran que el pedido real del usuario no fue perfecto. -->

---

## Ejemplo real: la IA se equivoca

- La IA defendió el orden por ID: "cada índice ya lo entrega ordenado"
- El usuario: *"no es verda que quedaran ordenados por ID, si tienes varios valores…"*
- La IA lo comprobó, **reconoció el error** y cambió su recomendación
- Después, el usuario **recortó el alcance**: *"vamos más poco a poco"* → PRD v1.1

**Lección:** recomendar no es decidir. Las mejores decisiones salieron de respuestas libres, no de las opciones propuestas.

<span class="detalle">→ Detalle: [Cómo se generó el PRD, sección 8](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/generacion-del-prd.md#8-segundo-ejemplo-el-prd-del-consultor)</span>

<!-- También el caso de mayúsculas: un supuesto que nadie había medido obligó a cambiar el generador (esquema 2). -->

---

## Por qué importa el PRD

- **Separa decidir de construir**: el agente trabaja horas sin parar porque no le queda nada de negocio por preguntar
- **Contrato verificable**: criterios con cifras reales
- **Limita el alcance**: el agente no amplía el cambio por su cuenta
- **Lo lee cualquiera**, también quien nunca abre el código
- **Dura más que el código**: cuando cambia una regla, cambia primero el PRD
- **Hace auditable** el trabajo con IA

Bugs, refactors y tooling sin cambio de producto: **no necesitan PRD**.

<span class="detalle">→ Detalle: [sección 2](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#2-el-prd)</span>

---

## Tres reglas antes de ejecutar

1. **Estimación de tiempo** antes de empezar, con la hora real de inicio
   - [Consultor](https://github.com/jesuslpm/chicago-crime/blob/main/odd/tasks/crime-query-report.md): 4–7 h estimadas, unas 3 h 15 min reales
2. **Sin interrupciones**: todas las preguntas al principio; después, valores por defecto documentados
   - Una sola pregunta a mitad de proceso puede dejarlo parado horas
3. **[Informe de cierre](https://github.com/jesuslpm/chicago-crime/blob/main/odd/tasks/crime-query-report.md) en Markdown, guardado en el repositorio**
   - Resultado, tiempo real frente al estimado, líneas por área, resumen y pendientes

<span class="detalle">→ Detalle: [sección 3](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#3-tres-reglas-antes-de-ejecutar)</span>

<!-- Mostrar odd/tasks/crime-query-report.md como ejemplo de informe. -->

---

## Las reglas, versionadas en el repositorio

- **[`CLAUDE.md`](https://github.com/jesuslpm/chicago-crime/blob/main/CLAUDE.md)**: instrucciones del proyecto que Claude Code carga en cada sesión. Corto, solo lo que siempre aplica
- **Skill [`closing-report`](https://github.com/jesuslpm/chicago-crime/tree/main/.claude/skills/closing-report)**: se carga solo al cerrar
  - [plantilla del informe](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/closing-report/assets/closing-report-template.md)
  - [script](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/closing-report/scripts/numstat_by_area.py) que calcula las líneas por área desde `git`
- **Skill [`vibe-coding`](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/vibe-coding/SKILL.md)**: aplica las tres reglas cuando se pide vibe coding
- Así no dependen de la memoria personal de nadie: **las hereda quien clona el repositorio**

Otros agentes, como Codex u OpenCode, leen `AGENTS.md`.

<span class="detalle">→ Detalle: [sección 3](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#3-tres-reglas-antes-de-ejecutar)</span>

<!-- Abrir CLAUDE.md y .claude/skills/closing-report/ en la otra pantalla. -->

---

## Tres formas de ejecutar un PRD

| | Vibe coding | SDD | ODD |
|---|---|---|---|
| Después del PRD | Conversación directa | Fases formales | Flujo orgánico |
| Formalidad | Variable | Estructurada | Adaptativa |
| Sirve para | Prototipos, cambios sencillos | Alto riesgo o complejidad | Funcionalidad real con poca ceremonia |
| Riesgo | Deriva y pérdida de contexto | Sobrecarga documental | Reinterpretar requisitos entre agentes |

En los tres, **el agente no altera en silencio un requisito**.

<span class="detalle">→ Detalle: [sección 4](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#4-tres-formas-de-ejecutar-un-prd)</span>

<!-- Es un encuadre conceptual, no una comparación medida. -->

---

## Prompts para empezar

**PRD:** *"Vamos a escribir el PRD de `<cambio>` con el skill [prd](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/prd/SKILL.md)… Se admiten todo tipo de sugerencias y preguntas… No implementes nada."*

**ODD:** *"Implementa `<PRD>` con ODD. Antes de empezar, dame una estimación de tiempo… No me interrumpas… Verifica siempre… guarda un informe de cierre."*

**SDD:** *"Ejecuta el flujo SDD completo a partir de `<PRD>` con `<OpenSpec | Spec Kit>`, hasta la verificación incluida…"*

**Vibe coding:** *"Implementa lo que pide `<PRD>` **usando vibe coding**."* El skill [`vibe-coding`](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/vibe-coding/SKILL.md) pone el resto: mapa del código existente, DRY, tests por criterio, verificación e informe.

<span class="detalle">→ Detalle: [sección 5](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#5-prompts-para-empezar)</span>

<!--
Hay que nombrar el enfoque: en un proyecto con ODD por defecto, sin decir "vibe coding" el agente usa ODD.
SDD necesita la herramienta instalada.
-->

---

## Vibe coding y SDD

**Vibe coding**
- Implementación conversacional: el PRD es el ancla y la conversación es el plan
- Un criterio por vez, con su test y un commit de punto de control
- No hay documento de ejecución: **los commits son la memoria**
- El skill [`vibe-coding`](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/vibe-coding/SKILL.md) fija **los bordes**, no el proceso: explora antes de escribir y exige DRY

**SDD**

```text
propose → design → spec → apply → verify → archive
```

- Herramientas: [**OpenSpec**](https://github.com/Fission-AI/OpenSpec) (`/opsx:*`; `verify` solo en el perfil ampliado) y [**Spec Kit**](https://github.com/github/spec-kit) (`/speckit-*`)

<span class="detalle">→ Detalle: [vibe coding, sección 6](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#6-el-proceso-vibe-coding-en-detalle) · [SDD, sección 7](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#7-el-proceso-sdd-en-detalle)</span>

---

## ODD: protocolo y documento de ejecución

**Autorizar → explorar → resolver dudas → clasificar → registrar → implementar → cerrar**

- **Pequeño** si se podría terminar solo con el pedido original y el `git diff`. Si no, es **grande**
- Lo grande va a [`odd/tasks/<feature>.md`](https://github.com/jesuslpm/chicago-crime/tree/main/odd/tasks):

| Sección | Contenido |
|---|---|
| Encabezado | Objetivo, rama, tests, estimación, hora real de inicio |
| `## Specs` | `S1…Sn` citando **textualmente** el PRD |
| `## Tasks` | Tarea, `S#` que cubre, estado, commit |
| `## Log` | `L1` = el pedido original, textual; después evidencia y decisiones |

<span class="detalle">→ Detalle: [sección 8](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#8-el-proceso-odd-en-detalle)</span>

<!-- Abrir odd/tasks/crime-query.md en la otra pantalla. -->

---

## ODD: el ciclo de cada tarea

1. Leer el documento y saber qué `S#` cubre
2. **Test primero**: RED → GREEN → refactor
3. Test enfocado, batería completa, formato y lint
4. **Un commit por tarea**, anotado como evidencia
5. Evaluar el riesgo: el alto activa un **verificador independiente**
6. **Revisión automática** con hasta cuatro lentes; los hallazgos se convierten en tareas nuevas
   - [T2a](https://github.com/jesuslpm/chicago-crime/blob/main/odd/tasks/crime-generator.md): un `--log` mal puesto podía **borrar el CSV de origen**
7. Actualizar el documento antes de la siguiente tarea

<span class="detalle">→ Detalle: [sección 8](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#8-el-proceso-odd-en-detalle)</span>

<!-- Las correcciones tienen límite: una tanda y una recomprobación. No hay ciclos de "revisar hasta limpio". -->

---

## Bajo el capó: subagentes

La sesión principal es el **orquestador**: entiende, decide y reparte. Cada subagente tiene **su propio contexto** y devuelve un resumen.

| Papel | Qué hace en el experimento |
|---|---|
| **Escritor** | Implementó cada tarea del [generador](https://github.com/jesuslpm/chicago-crime/blob/main/odd/tasks/crime-generator.md), con sus archivos permitidos y sus comandos de verificación |
| **Explorador** | Lee código desconocido, solo lectura |
| **Verificador** | No escribió el código: prueba la especificación contra el resultado |
| **Revisores** | Uno por lente (riesgo, resiliencia, legibilidad, fiabilidad), en paralelo |
| **Refutador** | Intenta desmentir un bloqueo dudoso antes de exigir una corrección |

En SDD pasa lo mismo: cada fase la ejecuta un subagente.

<span class="detalle">→ Detalle: [sección 9](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#9-bajo-el-capó-subagentes-memoria-skills-y-hooks)</span>

<!--
Ejemplo: en el consultor, mientras un verificador comprobaba T1, un escritor ya implementaba T2.
El ODD actual escribe en la sesión principal por defecto y delega solo con un motivo concreto.
-->

---

## Bajo el capó: memoria, skills y hooks

| Mecanismo | Qué es | En el experimento |
|---|---|---|
| **[`CLAUDE.md`](https://github.com/jesuslpm/chicago-crime/blob/main/CLAUDE.md)** | Memoria del proyecto, versionada | Las reglas: PRD primero, estimación, sin interrupciones |
| **Memoria de Claude Code** | Preferencias del usuario | "No me preguntes, verifica siempre" |
| **[Engram](https://github.com/Gentleman-Programming/engram)** | Memoria persistente (servidor MCP) | Decisiones, hallazgos, resúmenes y copia del documento de ejecución para retomar |
| **Skills** | Procedimientos que se cargan cuando hacen falta | [`prd`](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/prd/SKILL.md), [`closing-report`](https://github.com/jesuslpm/chicago-crime/tree/main/.claude/skills/closing-report), [`vibe-coding`](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/vibe-coding/SKILL.md), [`worktree`](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/parallel-developing.md#6-la-skill-cómo-se-lo-expone-al-agente) y las de [Gentle AI](https://github.com/Gentleman-Programming/gentle-ai) |
| **Hooks** | Comandos automáticos del entorno | Contexto al empezar, revisión obligatoria al terminar, guarda de la base de datos |

**Lo crítico no se deja a la memoria del agente: se automatiza.**

<span class="detalle">→ Detalle: [sección 9](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#9-bajo-el-capó-subagentes-memoria-skills-y-hooks)</span>

<!--
Anécdota: la conversación del PRD del consultor se reconstruyó a partir del historial de sesiones que guarda
Claude Code. Engram, por su parte, recoge los aprendizajes clave con los que cierra cada subagente.
-->

---

## Desarrollo en paralelo con worktrees

- `git worktree` aísla el **código**, pero no la base de datos, la configuración ni los puertos
- **Un nombre, todo derivado**: rama, carpeta, base `<Proyecto>_wt_<n>` y configuración local
- **Nadie escribe el nombre de la base**: lo resuelve un script
- Lógica en **scripts**; [la skill](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/parallel-developing.md#6-la-skill-cómo-se-lo-expone-al-agente) es una tabla de enrutado; un **hook** bloquea la base principal
- Fusionar con prudencia: primero la integración entra en la rama, y solo después al revés

*"Crea un worktree para el cambio `<n>`."* → PRD → ODD → *"Fusiona el cambio."*

<span class="detalle">→ Detalle: [sección 10](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#10-desarrollo-en-paralelo-con-worktrees) · [guía completa](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/parallel-developing.md)</span>

<!-- El PRD cerrado y sin interrupciones es lo que habilita tener varios agentes trabajando a la vez. -->

---

## Qué sabemos y qué no

**Respaldado por el experimento**
- Un PRD trabajado alcanza para que ODD genere un plan detallado
- Un agente produjo Rust no trivial sin que el autor conociera el lenguaje
- Los artefactos persistentes hacen el trabajo auditable

**Falta medir**
- ¿Requiere ODD menos intervención humana que SDD?
- ¿Cuesta menos tiempo o tokens con la misma calidad?
- ¿Da vibe coding resultados comparables con pruebas justas?

<span class="detalle">→ Detalle: [sección 12](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#12-qué-sabemos-y-qué-no)</span>

---

<!-- _class: lead -->

# Gracias

- Repositorio: [github.com/jesuslpm/chicago-crime](https://github.com/jesuslpm/chicago-crime)
- Documento completo: [desarrollo-asistido-por-ia.md](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md)
- Cómo se generó el PRD: [generacion-del-prd.md](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/generacion-del-prd.md)
- Skill PRD: [Gist](https://gist.github.com/jesuslpm/fc36d3503116171eee409b313365cde7)

**Preguntas**

<!-- Reservar unos 5 minutos al final para preguntas. -->
