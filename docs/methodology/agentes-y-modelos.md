# Agentes de Gentle AI: modelos y esfuerzo

Qué agentes intervienen cuando se trabaja con ODD y RDD en Claude Code, quién los lanza, cómo se elige el
modelo y el esfuerzo de cada uno, y qué se observó realmente en el experimento. Complementa la
[sección 9 de Desarrollo práctico de software asistido por IA](desarrollo-asistido-por-ia.md#9-bajo-el-capó-subagentes-memoria-skills-y-hooks).

> **Estado de la información.** Cada afirmación indica su fuente: la documentación de Gentle AI o de Claude
> Code, la pantalla de configuración del TUI de Gentle AI 4.1.1, o los historiales de las sesiones del
> experimento. Lo que no se pudo comprobar está en la sección 5.

## 1. Los papeles

La pantalla **Configure Models** de Gentle AI lista estos papeles para Claude Code:

| Papel en el TUI | Grupo | Qué hace | Quién lo lanza | Definición instalada (`~/.claude/agents/`) |
|---|---|---|---|---|
| ODD Explorer | ODD | Lee código y devuelve un mapa, solo lectura | La sesión principal (orquestador) | Ninguna |
| ODD Worker | ODD | Escribe una unidad de trabajo | La sesión principal | Ninguna |
| ODD Verify | ODD | Verificador independiente de un cambio de alto riesgo | La sesión principal | Ninguna |
| General delegation | Delegación | Otras tareas delegadas | La sesión principal | Ninguna |
| RDD Risk, Readability, Reliability, Resilience | RDD | Revisor de una lente | `gentle-ai`, durante la captura de la revisión | `review-risk.md`, `review-readability.md`, `review-reliability.md`, `review-resilience.md`: `model: sonnet`, `tools: []` |
| RDD Refuter | RDD | Intenta desmentir un bloqueo dudoso | `gentle-ai` | `review-refuter.md`: `model: sonnet` |
| RDD Validator | RDD | Valida la única corrección acotada | `gentle-ai` | Ninguna |
| JD Judge A, JD Judge B, JD Fix Agent | Judgment Day | Revisión adversarial doble y corrección | El skill `judgment-day` | `jd-judge-a.md`, `jd-judge-b.md`, `jd-fix-agent.md`: `model: sonnet` |

Dos grupos se comportan distinto:

- **Los papeles de ODD los lanza la sesión principal** como subagentes nativos de Claude Code. Sin una
  definición propia, heredan el modelo y el esfuerzo de la sesión (sección 3).
- **Los revisores de RDD los lanza `gentle-ai`** por su cuenta, sin herramientas y sobre el código congelado.
  Por eso no dejan historial de subagente en Claude Code.

## 2. Cómo se configuran

### 2.1 Gentle AI: pantalla Configure Models

Se abre desde el TUI de `gentle-ai`. Primero se elige un preset (textos tal como aparecen en la pantalla):

| Preset | Descripción en el TUI |
|---|---|
| **balanced** | Smart defaults for ODD delegation and review roles. Es el preset activo en este equipo |
| **performance** | Maximum quality for delegation and review roles |
| **economy** | Cost-optimised delegation and review roles |
| **diversity** | Diversity: Opus for Judge A, Haiku for Judge B, Sonnet for fixes |
| **custom** | Pick model and supported effort for ODD, JD and RDD roles individually |

En **custom**, cada papel recibe un modelo y un esfuerzo:

- `j` / `k` (o las flechas) para moverse por la lista;
- `enter` sobre un papel para elegir su modelo y después su esfuerzo;
- `enter` sobre **Confirm** para guardar y sincronizar;
- `esc` para volver a los presets sin guardar.

La documentación de Gentle AI ([`usage.md`](https://github.com/Gentleman-Programming/gentle-ai/blob/main/docs/usage.md),
sección *model assignment*, y [`agents.md`](https://github.com/Gentleman-Programming/gentle-ai/blob/main/docs/agents.md))
dice que la asignación es "client-specific and applies to supported generic and review roles, not a formal
development-phase matrix", y describe el esfuerzo solo para Codex. La pantalla del TUI 4.1.1 es más concreta
que la documentación: incluye los papeles de ODD y ofrece esfuerzo también para Claude Code.

### 2.2 Claude Code: mecanismos propios

Según la [documentación de subagentes de Claude Code](https://code.claude.com/docs/en/sub-agents):

- **El modelo de un subagente** se decide en este orden:
  1. el parámetro `model` de la llamada que lo lanza;
  2. el campo `model` de su definición: `sonnet`, `opus`, `haiku`, `fable`, un ID completo como
     `claude-opus-5-5`, o `inherit`;
  3. la variable de entorno `CLAUDE_CODE_SUBAGENT_MODEL`;
  4. el modelo de la sesión principal.
- **El esfuerzo** se fija con el campo `effort` de la definición (`low`, `medium`, `high`, `xhigh` o `max`, según
  el modelo). Pisa el esfuerzo de la sesión, salvo que esté puesta la variable `CLAUDE_CODE_EFFORT_LEVEL`.
- **Para forzar un modelo en todos los subagentes**, se combina `CLAUDE_CODE_SUBAGENT_MODEL` con
  `CLAUDE_CODE_SUBAGENT_MODEL_FORCE=1`. Sin forzar, esa variable no afecta a los agentes integrados Explore y
  Plan.

Ejemplo de definición:

```markdown
---
name: odd-verifier
description: Independent verifier for high-risk ODD changes.
model: sonnet
effort: high
---
```

## 3. Lo que se observó en el experimento

Datos extraídos de los historiales de los 58 subagentes de las sesiones del proyecto:

| Sesión | Papeles | Modelo | Esfuerzo |
|---|---|---|---|
| Generador, PRDs y metodología | Escritores de las tareas T1–T7 y sus correcciones, traducción de los PRD | `claude-opus-5-5` | xhigh |
| Consultor | Escritores T1–T6c y verificadores independientes | `claude-opus-5-5` | high |
| Soporte Linux | Verificador independiente | `claude-opus-5-5` | high |
| Revisión de ODD | Escritor de documentación; consulta a la documentación de Claude Code | `claude-opus-5-5`; `claude-haiku-5-5` | high |

- **Ningún papel de ODD tenía definición propia**, así que todos los subagentes heredaron el modelo y el
  esfuerzo de su sesión. Por eso el esfuerzo cambia de una sesión a otra (xhigh en una, high en las demás).
- **El único subagente en Haiku** fue uno de consulta a la documentación, cuyo tipo de agente trae Haiku en su
  propia definición.
- **Los revisores de RDD** tienen `model: sonnet` en su definición. No dejan historial de subagente y los
  registros de revisión de Gentle AI no anotan el modelo, así que el modelo con el que corrieron no está
  confirmado por un registro.

## 4. Asignación recomendada

| Papel | Modelo | Esfuerzo | Por qué |
|---|---|---|---|
| ODD Worker | opus | high | Escribir es la tarea más difícil; ahí no conviene ahorrar |
| ODD Explorer | sonnet | medium | Solo lee y resume |
| ODD Verify | sonnet | high | Un modelo distinto al del escritor aporta una mirada realmente independiente |
| General delegation | sonnet | medium | Tareas sueltas |
| RDD | sonnet | por defecto | Como está |
| Judgment Day | preset `diversity` | por defecto | Jueces con modelos distintos detectan errores distintos |

ODD ya tiene en cuenta el abaratamiento: si un escritor corre con un modelo pequeño o con poco esfuerzo, sube el
nivel de verificación de su trabajo. Por eso conviene ahorrar en exploración y verificación, y dejar al
escritor con el modelo fuerte.

## 5. Pendiente de verificar

- **Que la configuración custom se aplique a los papeles de ODD.** El primer intento de **Confirm**, el
  2026-10-15, falló al reescribir `review-resilience.md`, y no se guardó ninguna asignación. Hay que repetirlo
  y comprobar qué escribe Gentle AI (nuevas definiciones en `~/.claude/agents/`, cambios en `CLAUDE.md` o en
  `settings.json`) y si el orquestador empieza a usarlas.
- **El modelo real de los revisores de RDD**, observando una revisión en curso.
- **Qué modelo asigna a cada papel** cada preset distinto de `custom`. La pantalla solo muestra la
  descripción general, y con el preset `balanced` el modo custom parte de `sonnet` en todos los papeles.

## Fuentes

- Gentle AI: [`docs/usage.md`](https://github.com/Gentleman-Programming/gentle-ai/blob/main/docs/usage.md) y
  [`docs/agents.md`](https://github.com/Gentleman-Programming/gentle-ai/blob/main/docs/agents.md); pantalla
  Configure Models del TUI, versión 4.1.1.
- Claude Code: [Subagents](https://code.claude.com/docs/en/sub-agents).
- Historiales de las sesiones del experimento y definiciones en `~/.claude/agents/`, revisados el 2026-10-15.
