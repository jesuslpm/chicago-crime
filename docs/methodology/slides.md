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
Cómo leerla:

Esta sesión trata de cómo desarrollamos software con agentes de IA de forma práctica y verificable, no de
demos. Vamos a ver tres formas de trabajar que comparten el mismo punto de partida, el PRD: vibe coding, SDD y
ODD.

Todo lo que veremos sale de un experimento real, chicago-crime, un repositorio público. Cada cifra que aparece
en las slides se puede comprobar en él.

Organización: unos 60 minutos. Documento de detalle en la otra pantalla: desarrollo-asistido-por-ia.md.
Ejemplo paso a paso: generacion-del-prd.md.

⏱ Al terminar esta slide: hacia el minuto 1.
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
Cómo leerla:

La tesis es que el PRD es la fuente permanente de verdad funcional, y que las especificaciones, el diseño, las
tareas y el código se derivan de él.

En el desarrollo de software asistido por IA distinguimos tres capas: la especificación de los requisitos, la
generación del código mediante un método de ejecución, y la evidencia que nos da la certeza de que el código
cumple los requisitos.

El PRD se encarga de los requisitos. Está escrito para personas, no necesariamente técnicas, en lenguaje de
producto. Su carácter es permanente, pero no inmutable: evoluciona a lo largo de todo el ciclo de vida del
software, y los cambios se van anotando con fecha en lugar de reescribir su historia.

La ejecución es variable: para cada cambio elegimos el método que mejor encaja, sea vibe coding, SDD u ODD, y las
herramientas también cambian con el tiempo. Lo que produce cada ejecución pertenece a ese cambio; el PRD
permanece.

La evidencia es imprescindible: tests, criterios de aceptación comprobados sobre datos reales, revisiones e
informe de cierre. Sin evidencia, no sabemos si el código hace lo que dice el PRD; solo lo suponemos.

Mensaje clave: no es "la IA sustituye a los ingenieros". Es que un ingeniero con dominio, algoritmos y
arquitectura puede dirigir agentes en una tecnología que todavía no domina, y sigue siendo responsable de
especificar y de evaluar.

⏱ Al terminar esta slide: hacia el minuto 3.
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

<!--
Cómo leerla:

El experimento parte de un CSV público de delitos de Chicago: 8.656.904 registros y 2,4 GB. El objetivo era
convertirlo en un conjunto de datos consultable, ordenado en SSTables, y hacerlo en Rust, un lenguaje que yo
no conocía al empezar.

Hicimos tres cambios, cada uno con su propio PRD: el generador, que construye el conjunto de datos; el
consultor, que responde preguntas sobre él; y el soporte para Linux.

La tabla resume los resultados. El generador tiene 230 tests y pasó 14 de 14 criterios de aceptación sobre el
CSV real, con un pico de memoria de 93 MiB frente a un límite de 512. El consultor tiene 423 tests, pasó 58 de
58 criterios y responde una consulta por ID en unos 40 o 50 milisegundos.

Mensaje clave: es un caso real, no una comparación controlada entre metodologías. Muestra que el método
funciona en un problema no trivial, no que un enfoque sea mejor que otro.

⏱ Al terminar esta slide: hacia el minuto 4.
-->

---

## Qué es un PRD

- **Qué** hace el producto y **por qué**, en lenguaje de negocio
- **No** lleva clases, endpoints, rutas ni números de línea: eso es el *cómo*
- No es un prompt desechable: es **documentación duradera**, versionada en el repositorio
- **Jerárquico**: un [PRD padre](https://github.com/jesuslpm/chicago-crime/blob/main/docs/prd/chicago-crime/chicago-crime.prd.md) por aplicación y un [PRD hijo](https://github.com/jesuslpm/chicago-crime/tree/main/docs/prd/chicago-crime) por cambio
- Si un hijo contradice al padre, el padre se **anota**, no se reescribe

<span class="detalle">→ Detalle: [sección 2](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#2-el-prd)</span>

<!--
Cómo leerla:

Un PRD describe qué hace el producto y por qué, en lenguaje de negocio. No contiene clases, endpoints, rutas
de archivos ni números de línea: eso es el cómo, y pertenece al código y a los documentos de ejecución.

No es un prompt desechable que se escribe para arrancar una sesión y se olvida. Es documentación duradera,
versionada en el repositorio junto al código.

Es jerárquico: un PRD padre por aplicación o módulo grande, que describe el producto completo, y un PRD hijo
por cada cambio, que nombra a su padre y solo añade lo que ese cambio necesita.

Cuando un hijo contradice al padre, el padre no se reescribe: se anota con fecha. Así se conserva la historia
de cada decisión.

Mostrar en la otra pantalla: docs/prd/chicago-crime/, el padre y sus tres hijos.

⏱ Al terminar esta slide: hacia el minuto 6.
-->

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

<!--
Cómo leerla:

La plantilla tiene más secciones, pero estas cinco son las que marcan la diferencia.

El estado de partida se verifica en el código y en los datos; nunca se supone. En el experimento, medir el CSV
antes de escribir cambió varias decisiones.

Las validaciones y mensajes recogen el texto exacto que verá el usuario, de modo que nadie lo improvisa al
programar.

El fuera de alcance es obligatorio: delimita el cambio y marca dónde empieza el siguiente. Sin él, el alcance
crece solo.

Los criterios de aceptación llevan cifras reales, por ejemplo que el índice de tipo principal tenga 1.838.502
entradas THEFT. Así "funciona" deja de ser una opinión y pasa a ser algo comprobable.

Las decisiones registran cada elección con su motivo y su consecuencia aceptada. Esta sección no estaba en la
plantilla original: nació en el experimento porque resultó la más útil.

⏱ Al terminar esta slide: hacia el minuto 7.
-->

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
Cómo leerla:

El PRD se escribe conversando con la IA, y el skill prd le fija las reglas.

Empieza el programador: pide el PRD de un cambio y explica qué quiere y por qué, con el contexto que tenga. En
el experimento, ese contexto fue un resumen de una conversación previa con ChatGPT.

La IA lee el PRD padre y la documentación, y mide el estado de partida en el código y en los datos. Recorrer
el CSV completo reveló que no venía ordenado, que el número de caso se repetía y que había 253 horas
inexistentes por el cambio de horario. Las tres cosas cambiaron decisiones.

Con eso redacta un borrador con las decisiones abiertas, cada una con su recomendación, y las cerramos de una
en una. Si afirmo algo técnico, la IA lo verifica antes de darme la razón.

Iteramos hasta que no queda nada abierto. Y un detalle importante: escribir el PRD no autoriza a implementar;
la implementación se pide aparte.

Mostrar, si hay tiempo: generacion-del-prd.md.

⏱ Al terminar esta slide: hacia el minuto 10.
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

<!--
Cómo leerla:

Esta slide muestra el paso de verificación en acción, con tres decisiones reales del PRD del generador.

En D1 afirmé que el crate sstable era compatible con LevelDB. La IA no me dio la razón sin más: leyó el código
fuente del crate y comprobó que el formato coincide. Tenía razón.

En D4 propuse guardar el ID también en el valor, porque ocupaba poco y ahorraba memoria. La IA midió: 33 MiB,
un 2,9 %. Era más simple, pero no ahorraba RAM. Tenía razón a medias.

En D5 dije que detectar duplicados lejanos sería difícil. La IA explicó que, después de la ordenación externa,
los duplicados quedan contiguos por lejos que estuvieran en el CSV. La premisa era incorrecta.

Mensaje clave: en los tres casos la evidencia fueron código y números, no opiniones. La IA no está para darnos
la razón, sino para comprobarla.

Mostrar: las citas literales del documento; enseñan que el pedido real no fue perfecto.

⏱ Al terminar esta slide: hacia el minuto 12.
-->

---

## Ejemplo real: la IA se equivoca

- La IA defendió el orden por ID: "cada índice ya lo entrega ordenado"
- El usuario: *"no es verda que quedaran ordenados por ID, si tienes varios valores…"*
- La IA lo comprobó, **reconoció el error** y cambió su recomendación
- Después, el usuario **recortó el alcance**: *"vamos más poco a poco"* → PRD v1.1

**Lección:** recomendar no es decidir. Las mejores decisiones salieron de respuestas libres, no de las opciones propuestas.

<span class="detalle">→ Detalle: [Cómo se generó el PRD, sección 8](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/generacion-del-prd.md#8-segundo-ejemplo-el-prd-del-consultor)</span>

<!--
Cómo leerla:

Ahora el caso contrario, del PRD del consultor.

La IA defendió que los resultados salieran ordenados por ID, con el argumento de que cada índice ya los
entrega ordenados. Le respondí que no era cierto: con varios valores, el índice los agrupa por valor, no por
ID. La IA lo comprobó contra el contrato del conjunto de datos, reconoció el error y cambió su recomendación.

Después recorté el alcance: dejamos la ordenación para un cambio posterior, para ir más poco a poco. Así nació
la versión 1.1 del PRD.

Lección: recomendar no es decidir. Las mejores decisiones del consultor salieron de mis respuestas libres, no
de las opciones que proponía la IA.

También pasó con las mayúsculas: un supuesto que nadie había medido obligó a cambiar el generador y a subir el
esquema a la versión 2.

⏱ Al terminar esta slide: hacia el minuto 13.
-->

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

<!--
Cómo leerla:

Resumimos por qué merece la pena el esfuerzo de escribir el PRD.

Separa decidir de construir: las decisiones de producto se cierran antes de programar, y por eso el agente
puede trabajar horas sin parar, porque no le queda nada de negocio por preguntar.

Es un contrato verificable, con criterios que llevan cifras reales. Limita el alcance: el agente no amplía el
cambio por su cuenta. Lo lee cualquiera, también quien nunca va a abrir el código. Dura más que el código:
cuando cambia una regla, se cambia primero el PRD y después el código. Y hace auditable el trabajo con IA:
cualquiera puede revisar qué se pidió, qué se decidió y qué se comprobó.

Eso sí, no todo necesita un PRD: los bugs, las refactorizaciones y el tooling que no cambian el comportamiento
del producto van directos a la ejecución.

⏱ Al terminar esta slide: hacia el minuto 15.
-->

---

## Tres reglas antes de ejecutar

1. **Estimación de tiempo** en un mensaje propio, junto con «¿comienzo?»; la hora real de inicio, al confirmar
   - [Consultor](https://github.com/jesuslpm/chicago-crime/blob/main/odd/tasks/crime-query-report.md): 4–7 h estimadas, unas 3 h 15 min reales
2. **Sin interrupciones**: todas las preguntas al principio; después, valores por defecto documentados
   - Una sola pregunta a mitad de proceso puede dejarlo parado horas
3. **[Informe de cierre](https://github.com/jesuslpm/chicago-crime/blob/main/odd/tasks/crime-query-report.md) en Markdown, guardado en el repositorio**
   - Resultado, revisiones con sus hallazgos, tiempo real frente al estimado, líneas por área, resumen y pendientes

<span class="detalle">→ Detalle: [sección 3](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#3-tres-reglas-antes-de-ejecutar)</span>

<!--
Cómo leerla:

Estas tres reglas valen para cualquier forma de ejecutar el PRD, y atacan los problemas que más tiempo nos
hacen perder.

La primera es pedir una estimación de tiempo antes de empezar. El agente nos la da en un mensaje propio, junto
con la pregunta de si empieza, para que no se pierda entre todo lo que va haciendo. Cuando confirmamos, anota
la hora real de inicio. Sin esa referencia no hay forma de calibrar ni de planificar. En el consultor se estimaron entre 4 y 7 horas, y se
tardaron unas 3 horas y cuarto.

La segunda es que no haya interrupciones. Todas las preguntas se hacen al principio, y después el agente no se
detiene: ante una duda menor toma un valor razonable, lo documenta y lo informa al final. Una sola pregunta a
mitad de proceso puede dejarlo parado horas mientras nadie mira la pantalla, y eso nos pasó.

La tercera es exigir un informe de cierre en Markdown, guardado en el repositorio, con el resultado, las
revisiones automáticas con sus hallazgos, el tiempo real frente al estimado, las líneas por área, un resumen y
los pendientes. Los hallazgos hay que anotarlos en el momento, porque la herramienta de revisión no los guarda.

Mostrar: odd/tasks/crime-query-report.md.

⏱ Al terminar esta slide: hacia el minuto 17.
-->

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

<!--
Cómo leerla:

Las tres reglas no se quedan en buenas intenciones: viven en el repositorio.

CLAUDE.md son las instrucciones del proyecto que Claude Code carga en cada sesión. Es deliberadamente corto:
solo lleva lo que siempre aplica.

El skill closing-report se carga solo al cerrar. Trae la plantilla del informe y un script que calcula las
líneas por área a partir de git, para que el agente no tenga que sumarlas a mano.

El skill vibe-coding aplica las tres reglas cuando se pide vibe coding, que es el único enfoque que no trae un
proceso propio.

Mensaje clave: así las reglas no dependen de la memoria personal de nadie; cualquiera que clone el repositorio
las hereda. Otros agentes, como Codex u OpenCode, leen AGENTS.md en lugar de CLAUDE.md.

Mostrar: CLAUDE.md y .claude/skills/closing-report/.

⏱ Al terminar esta slide: hacia el minuto 19.
-->

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

<!--
Cómo leerla:

Después del PRD tenemos tres formas de ejecutar. Conviene aclarar que no son tres frameworks equivalentes: el
PRD es común a las tres, y lo que cambia es cómo se implementa, se coordina y se verifica.

Con vibe coding implementamos conversando directamente con la IA, y la formalidad depende de nuestra
disciplina. Sirve para prototipos y cambios sencillos; su riesgo es la deriva de requisitos y la pérdida de
contexto en sesiones largas.

SDD sigue fases formales, cada una con su artefacto. Es más estructurado y encaja con cambios de alto riesgo o
complejidad, a cambio de más sobrecarga documental.

ODD adapta la ceremonia al tamaño y al riesgo de cada cambio. Sirve para funcionalidad real con poca
ceremonia; su riesgo es que los requisitos se reinterpreten al pasar de un agente a otro.

En los tres casos, el agente no altera en silencio un requisito: si encuentra una decisión de negocio abierta,
vuelve a preguntar.

Aviso: es un encuadre conceptual, no una comparación medida.

⏱ Al terminar esta slide: hacia el minuto 20.
-->

---

## Prompts para empezar

**PRD:** *"Vamos a escribir el PRD de `<cambio>` con el skill [prd](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/prd/SKILL.md)… Se admiten todo tipo de sugerencias y preguntas… No implementes nada."*

**ODD:** *"Implementa `<PRD>` con ODD. Antes de empezar, dame en un mensaje aparte una estimación de tiempo y pregúntame si empiezas… No me interrumpas… Verifica siempre… guarda un informe de cierre."*

**SDD:** *"Ejecuta el flujo SDD completo a partir de `<PRD>` con `<OpenSpec | Spec Kit>`, hasta la verificación incluida…"*

**Vibe coding:** *"Implementa lo que pide `<PRD>` **usando vibe coding**."* El skill [`vibe-coding`](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/vibe-coding/SKILL.md) pone el resto: mapa del código existente, DRY, tests por criterio, verificación, revisión con RDD si está activo e informe.

<span class="detalle">→ Detalle: [sección 5](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#5-prompts-para-empezar)</span>

<!--
Cómo leerla:

Estos son los prompts para arrancar cada forma de trabajar; están completos en la sección 5 del documento.

El del PRD invita expresamente a hacer sugerencias y preguntas, y deja claro que no se implementa nada. Esa
invitación funcionó en el experimento: de ahí salieron el manifiesto, la publicación atómica y la indexación
de los valores vacíos.

Los de ODD y SDD repiten las tres reglas: estimación, sin interrupciones e informe de cierre. El de SDD
necesita además una herramienta instalada, como OpenSpec o Spec Kit.

El de vibe coding es el más corto porque el skill vibe-coding pone el resto: el mapa del código existente, el
código DRY, los tests por criterio, la verificación, la revisión automática si RDD está activo y el informe.

Detalle importante: hay que nombrar el enfoque. En un proyecto con ODD por defecto, como este, si el prompt no
dice "vibe coding", el agente usa ODD.

⏱ Al terminar esta slide: hacia el minuto 22.
-->

---

## Vibe coding

- Implementación conversacional: el PRD es el ancla y la conversación es el plan
- Un criterio por vez, con su test y un commit de punto de control
- No hay documento de ejecución: **los commits son la memoria**
- Con RDD activo, **revisión automática** como en ODD: tras cada commit y al cerrar
- El skill [`vibe-coding`](https://github.com/jesuslpm/chicago-crime/blob/main/.claude/skills/vibe-coding/SKILL.md) fija **los bordes**, no el proceso: explora antes de escribir y exige DRY

<span class="detalle">→ Detalle: [sección 6](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#6-el-proceso-vibe-coding-en-detalle)</span>

<!--
Cómo leerla:

Vibe coding es una implementación conversacional: el PRD hace de ancla y la conversación hace de plan.
Avanzamos criterio a criterio, cada uno con su test y un commit que sirve de punto de control. Como no hay
documento de ejecución, los commits son la memoria del proceso. Si RDD está activo, la revisión automática
funciona igual que en ODD: revisa el diff después de cada commit y, al cerrar, el último tramo.

El skill vibe-coding fija los bordes, no el proceso. Antes de escribir explora el código existente para no
duplicar lo que ya hay, porque el vibe coding tiene la mala costumbre de dejar el código WET, y al final exige
la verificación y el informe.

⏱ Al terminar esta slide: hacia el minuto 24.
-->

---

## SDD: desarrollo guiado por especificaciones

Una especificación formal precede al código; cada fase deja un artefacto revisable.

| Fase | Qué produce |
|---|---|
| `propose` | La propuesta: por qué hace falta el cambio y qué cambia |
| `design` | El enfoque técnico |
| `spec` | Escenarios *cuando / entonces*, como cambios sobre las especificaciones vigentes |
| `apply` | La implementación, marcando cada tarea al terminarla |
| `verify` | La comprobación del resultado contra las especificaciones |
| `archive` | Los cambios integrados en las especificaciones vigentes, listas para el próximo cambio |

- Herramientas: [**OpenSpec**](https://github.com/Fission-AI/OpenSpec) (`/opsx:*`) y [**Spec Kit**](https://github.com/github/spec-kit) (`/speckit-*`), de GitHub

<span class="detalle">→ Detalle: [sección 7](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#7-el-proceso-sdd-en-detalle)</span>

<!--
Cómo leerla:

SDD es desarrollo guiado por especificaciones: una especificación formal precede al código. El trabajo se
divide en seis fases, y cada una deja un artefacto que podemos revisar antes de pasar a la siguiente.

Primero, proponer: por qué hace falta el cambio y qué cambia. Después, diseñar: el enfoque técnico. Luego,
especificar: los requisitos de comportamiento, con escenarios del tipo "cuando pasa esto, entonces el sistema
hace aquello", escritos como cambios sobre las especificaciones que ya existen. La lista de tareas también se
genera antes de implementar.

Aplicar es implementar esas tareas y marcar cada una al terminarla. Verificar es comprobar el resultado contra
las especificaciones. Y archivar integra esos cambios en las especificaciones vigentes, que quedan listas para
el próximo cambio.

Las herramientas más conocidas son OpenSpec, cuya fase de verificación solo está en el perfil ampliado, y Spec
Kit, de GitHub. Un detalle importante: las especificaciones no reemplazan al PRD. Si una especificación lo
contradice, manda el PRD.

⏱ Al terminar esta slide: hacia el minuto 25.
-->

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

<!--
Cómo leerla:

ODD sigue siete pasos en cada pedido: autorizar, explorar, resolver dudas, clasificar, registrar, implementar
y cerrar.

El paso clave es clasificar. Una tarea es pequeña si alguien podría terminarla solo con el pedido original y
el git diff; si no, es grande. El número de archivos o de líneas nunca decide el tamaño.

Lo pequeño se resuelve directamente, sin documentos. Lo grande se registra en un documento de ejecución,
odd/tasks/<feature>.md, con cuatro secciones: un encabezado con el objetivo, la rama, la estimación y la hora
real de inicio; las specs, que citan textualmente el PRD; las tareas, con el commit que cierra cada una; y el
log, cuya primera entrada es el pedido original, palabra por palabra.

El PRD sigue mandando: si una spec lo contradice, gana el PRD.

Mostrar: odd/tasks/crime-query.md.

⏱ Al terminar esta slide: hacia el minuto 27.
-->

---

## ODD: el ciclo de cada tarea

1. Leer el documento y saber qué `S#` cubre
2. **Test primero**: RED → GREEN → refactor
3. Test enfocado, batería completa, formato y lint
4. **Un commit por tarea**, anotado como evidencia
5. Evaluar el riesgo: el alto activa un **verificador independiente**
6. **Revisión automática** con hasta cuatro lentes; los hallazgos se convierten en tareas nuevas
   - [T2a](https://github.com/jesuslpm/chicago-crime/blob/main/odd/tasks/crime-generator.md): un `--log` mal puesto podía **borrar el CSV de origen**
   - Busca defectos del cambio, **no compara con la especificación**; al cerrar revisa también el último tramo
7. Actualizar el documento antes de la siguiente tarea

<span class="detalle">→ Detalle: [sección 8](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md#8-el-proceso-odd-en-detalle)</span>

<!--
Cómo leerla:

Cada tarea sigue el mismo ciclo. Primero el agente lee el documento y sabe qué specs tiene que cubrir.

Después, test primero: escribe el test, lo ve fallar, implementa hasta que pasa y refactoriza con los tests en
verde. Ejecuta el test enfocado, la batería completa, el formato y el lint, y cierra la tarea con un commit
que queda anotado como evidencia.

Luego evalúa el riesgo del cambio. Si es alto, porque toca datos persistidos, seguridad, contratos que otros
consumen u otros puntos delicados, un verificador independiente, que no escribió el código, comprueba la
especificación contra el resultado.

Además, una revisión automática con hasta cuatro lentes revisa el commit, y sus hallazgos se convierten en
tareas nuevas. Así nació T2a: un --log mal puesto podía borrar el CSV de origen, y la revisión lo detectó.

Un matiz: esta revisión no compara el código con la especificación, eso lo hace el verificador. Busca defectos
que introdujo el cambio. Y al cerrar se revisa también el último tramo, aunque sea pequeño, para que todo lo
que se fusiona haya pasado por ella.

Las correcciones tienen límite: una tanda y una recomprobación. No hay ciclos de "revisar hasta que quede
limpio".

⏱ Al terminar esta slide: hacia el minuto 29.
-->

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
Cómo leerla:

Desde fuera parece una sola conversación, pero por dentro la sesión principal actúa como orquestador: entiende
el pedido, decide y reparte el trabajo. Cada subagente tiene su propio contexto y devuelve un resumen; así el
orquestador no se llena de código y puede sostener una conversación larga.

En el experimento vimos cinco papeles. Los escritores implementaron cada tarea del generador, con su lista de
archivos permitidos y los comandos de verificación que debían ejecutar. Los exploradores leen código
desconocido sin modificarlo. Los verificadores no escribieron el código y prueban la especificación contra el
resultado. Los revisores miran el cambio desde una lente cada uno, en paralelo. Y el refutador intenta
desmentir un bloqueo dudoso antes de exigir una corrección.

En SDD pasa lo mismo: cada fase la ejecuta un subagente.

Ejemplo: en el consultor, mientras un verificador comprobaba T1, un escritor ya implementaba T2. La versión
actual de ODD es más austera: el orquestador escribe directamente y solo delega con un motivo concreto.

⏱ Al terminar esta slide: hacia el minuto 30.
-->

---

## Bajo el capó: modelos y esfuerzo

| Grupo | Papeles | Se configura en | En el experimento |
|---|---|---|---|
| **ODD** | Explorer, Worker, Verify, delegación general | Gentle AI (*Configure Models*) o `.claude/agents/` | Sin definición: **heredaron Opus 5.5** y el esfuerzo de la sesión (xhigh o high) |
| **RDD** | Cuatro lentes, refutador, validador | Gentle AI → `~/.claude/agents/review-*.md` | Definidos en **Sonnet** |
| **Judgment Day** | Dos jueces y corrección | Gentle AI → `~/.claude/agents/jd-*.md` | Definidos en **Sonnet** |

Presets: **balanced**, performance, economy, diversity y **custom** (modelo y esfuerzo por papel).

**Recomendación:** escritor fuerte (Opus), exploración y verificación más baratas (Sonnet).

<span class="detalle">→ Detalle: [Agentes de Gentle AI: modelos y esfuerzo](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/agentes-y-modelos.md)</span>

<!--
Cómo leerla:

Cada papel puede correr con un modelo y un esfuerzo distintos, y eso afecta directamente al coste y a la
calidad.

Los papeles de ODD, explorador, escritor y verificador, se pueden configurar desde Gentle AI o con
definiciones propias en .claude/agents. En el experimento no tenían definición, así que heredaron el modelo y
el esfuerzo de la sesión principal: Opus 5.5, con esfuerzo alto o extra alto. Lo comprobamos en los
historiales de 58 subagentes.

Los revisores de RDD y de Judgment Day tienen definiciones propias en Sonnet.

Gentle AI ofrece cinco presets: balanced, performance, economy, diversity y custom, que permite elegir el
modelo y el esfuerzo de cada papel.

Recomendación: un escritor fuerte, en Opus, porque escribir es lo más difícil, y exploración y verificación
más baratas, en Sonnet. Un verificador con un modelo distinto al del escritor aporta además una mirada más
independiente.

Pendiente: la configuración custom de los papeles de ODD todavía no está verificada; el primer intento falló
sin guardar nada.

⏱ Al terminar esta slide: hacia el minuto 31.
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
Cómo leerla:

La memoria tiene tres capas, y cada una cumple un papel distinto.

CLAUDE.md es la memoria del proyecto: está versionada y la hereda cualquiera que clone el repositorio. La
memoria de Claude Code guarda mis preferencias personales, como "no me preguntes, verifica siempre". Y Engram,
un servidor MCP, guarda de forma proactiva decisiones, hallazgos y resúmenes de sesión, además de una copia
del documento de ejecución para poder retomar si la sesión se corta.

Los skills son procedimientos que se cargan solo cuando hacen falta: prd, closing-report, vibe-coding,
worktree y los de Gentle AI. Esa es la diferencia con CLAUDE.md, que se carga siempre.

Los hooks son comandos que el entorno ejecuta automáticamente: inyectan contexto al empezar la sesión, obligan
a pasar la revisión antes de dar el trabajo por terminado y bloquean el acceso a la base de datos principal
desde un worktree.

Mensaje clave: lo crítico no se deja a la memoria del agente; se automatiza.

Anécdota: la conversación del PRD del consultor se reconstruyó a partir del historial de sesiones que guarda
Claude Code.

⏱ Al terminar esta slide: hacia el minuto 33.
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

<!--
Cómo leerla:

Con un PRD cerrado y sin interrupciones, el agente puede trabajar horas solo, y eso habilita el paso
siguiente: varios cambios a la vez, cada uno en su propia copia del repositorio.

git worktree aísla el código, pero no la base de datos, la configuración ni los puertos. La regla de diseño es
que cada copia sea un entorno completo y autónomo.

Todo se deriva de un solo nombre: la rama, la carpeta, la base de datos y la configuración local. Y nadie
escribe el nombre de la base: lo resuelve un script, para que nunca se toque la principal por error.

La lógica vive en scripts versionados; el skill es solo una tabla que traduce cada frase del equipo a un
script; y un hook bloquea cualquier comando que apunte a la base principal desde un worktree.

Al fusionar, primero la rama de integración entra en la del cambio y se prueba; solo después el cambio entra
en la integración.

El flujo completo cabe en una línea: crear el worktree, PRD, ODD y fusionar.

⏱ Al terminar esta slide: hacia el minuto 34.
-->

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

<!--
Cómo leerla:

Para terminar, conviene separar lo que el experimento respalda de lo que todavía son hipótesis.

Respaldado: un PRD bien trabajado alcanza para que ODD genere un plan de implementación detallado; un agente
produjo una implementación en Rust que no es trivial sin que yo conociera el lenguaje; y tener artefactos
persistentes, como el PRD, los documentos de ejecución, los tests y los informes, hace que el trabajo sea
auditable.

Falta medir: si ODD requiere menos intervención humana que SDD, si cuesta menos tiempo o tokens con la misma
calidad, y si el vibe coding da resultados comparables con pruebas justas. Para eso habría que repetir cambios
comparables con cada enfoque y medir lo mismo en todos.

Mensaje clave: no estoy diciendo que ODD sea mejor. Estoy diciendo que, con un buen PRD, las tres formas de
ejecutar parten de una base sólida y verificable.

⏱ Al terminar esta slide: hacia el minuto 36.
-->

---

<!-- _class: lead -->

# Gracias

- Repositorio: [github.com/jesuslpm/chicago-crime](https://github.com/jesuslpm/chicago-crime)
- Documento completo: [desarrollo-asistido-por-ia.md](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/desarrollo-asistido-por-ia.md)
- Cómo se generó el PRD: [generacion-del-prd.md](https://github.com/jesuslpm/chicago-crime/blob/main/docs/methodology/generacion-del-prd.md)
- Skill PRD: [Gist](https://gist.github.com/jesuslpm/fc36d3503116171eee409b313365cde7)

**Preguntas**

<!--
Cómo leerla:

Todo el material está en el repositorio: el documento completo, el ejemplo de cómo se generó el PRD y el skill
de PRD, que también está publicado en un Gist para quien quiera usarlo en sus proyectos.

Reservar unos 5 minutos para preguntas.

⏱ Al llegar aquí: hacia el minuto 36. En una sesión de 60 minutos quedan unos 24 para preguntas o para una demo en vivo.
-->
