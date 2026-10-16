# Aceptación con cualquier CSV compatible: verificada con una descarga real del portal

Informe de cierre del feature `aceptacion-cualquier-csv` (2026-10-16). Enfoque de ejecución: ODD. El detalle
completo (specs, decisiones, log y evidencias) está en [aceptacion-cualquier-csv.md](aceptacion-cualquier-csv.md),
y el producto en el [PRD de verificación de aceptación con cualquier CSV
compatible](../../docs/prd/chicago-crime/aceptacion-cualquier-csv.prd.md).

Rama `fix/acceptance-any-csv` desde `407d43d`: 9 commits del feature (el PRD y T1–T8) más el de este informe.
Con la verificación completa, `main` avanza por fast-forward hasta la rama, según la regla del proyecto. **No se
hizo ningún push.**

## Resultado

- **Especificación:** S1–S7 entregados, que cubren AC-A1…AC-A4 (ver la cobertura más abajo).
- **Scripts:** el generador y el consultor reconocen el extracto de referencia por la huella SHA-256 del
  manifiesto. Con él exigen las cifras exactas de los PRD y ejecutan el verificador con `--reference`. Con
  cualquier otro CSV, cada cifra esperada sale del propio CSV.
- **Aceptación con datos reales** (cifras de los informes en `work/`):
  - **Descarga del portal del 2026-10-16:** 8.659.584 filas de datos frente a 8.656.904, 2.047.271.622 bytes,
    SHA-256 `a36d8277…3edd`.
  - **RED**, los scripts de `407d43d` sobre la descarga:
    - generador **FAIL 5 de 15** (AC-G1, AC-G4, AC-G9, AC-G5 de AC9 y de AC13);
    - consultor **FAIL 9 de 58** (AC-C1, AC-C2, AC-C6, AC-C8, AC-C11, AC-C12 ×2, AC-C17, AC-C18). `THEFT` dio
      1.839.054 frente a la cifra fija 1.838.502.
  - **GREEN**, los scripts finales (`cd46d82`, árbol limpio):
    - sobre la descarga: generador **PASS 15/15**, consultor **PASS 59/59**;
    - sobre el extracto de referencia: generador **PASS 15/15**, consultor **PASS 59/59** (18 criterios).
  - **Área comunitaria "sin valor":** dio 613.720 también en la descarga, porque las 2.680 filas nuevas traen área
    comunitaria. En la corrida RED, AC-C3 pasó. Con otra descarga podría cambiar, y entonces se contaría sobre el
    CSV.
- **Tests de Rust** (sin cambios en Rust): `cargo fmt --all --check` OK; `cargo clippy --workspace --all-targets
  -- -D warnings` sin avisos; `cargo test --workspace` con 425 pasadas y 0 fallidas.
- **Revisiones y verificación:**
  - 3 revisiones RDD, todas aprobadas y reconocidas, con el consentimiento concedido por regla del proyecto (ver
    la tabla). Todos los commits del feature pasaron por RDD.
  - Un verificador independiente (agente de solo lectura) encontró un blocker (B1), que se corrigió en T8.
- **Hallazgos corregidos por el camino:**
  - **El portal ya no entrecomilla los campos.** La descarga real (L7) rompía dos supuestos del script del
    generador:
    - la primera fila para AC-G5 se leía con una expresión que exigía comillas;
    - AC-G9 alteraba la cabecera con `.Replace('"IUCR"', ...)`, que no cambiaba una cabecera sin comillas. La
      generación "alterada" salía bien y reemplazaba Run A.

    Ahora los campos se separan respetando las comillas y se reescriben tal como venían (T6). Con el extracto de
    referencia, las filas y la cabecera son idénticas byte a byte a las de antes.
  - **Comparación byte a byte en el consultor** (AC-C1, AC-C6, AC-C8 y AC-C12): solo vale cuando el CSV de origen
    entrecomilla como el consultor. El PRD pide equivalencia en valor ("equivalente en valor", C4 "la misma
    cabecera"), así que con otro CSV se compara campo a campo; con el extracto de referencia se sigue comparando
    byte a byte (T7).
  - **B1, del verificador:** `$sample.Id + ','` con un ID `[long]` lanzaba excepción, y el script del consultor
    se caía en AC-C1 con cualquier CSV. Lo introdujo T7: su prueba ejercitó funciones extraídas, no esa línea.
    Corregido en T8.
  - **De la primera revisión** (T5):
    - el consultor no exigía 0 rechazados con otro CSV;
    - una huella mal formada habría pasado por "otro CSV" en silencio;
    - nombres de las cifras fijas;
    - carpeta `full` para el conjunto completo.
  - **De la segunda revisión** (T8): guardia de arranque de la cabecera de AC-G9; `-ByValue` explícito; la
    comparación por valor ya no descarta líneas vacías intermedias.
  - **Recuento de criterios** en el resumen del consultor: la fila nueva sin número AC-C contaba como criterio
    (T5a).

## Cobertura de criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-A1 | Las dos aceptaciones sobre el extracto de referencia con `cd46d82` | PASS 15/15 y 59/59. Cabecera "CSV: the reference extract (SHA-256 5cb5fa41…ae35d); the exact figures of the PRDs are required."; verificador con `--reference`; `8.656.904 registros válidos, 0 rechazados`; `THEFT` 1.838.502 = filtrado = cifra del PRD; AC-C1 byte a byte |
| AC-A2 | Las dos aceptaciones sobre la descarga del portal con `cd46d82` | PASS 15/15 y 59/59. Cabecera "CSV: not the reference extract (SHA-256 a36d8277…3edd); the expected figures are counted on this CSV."; verificador sin `--reference`; `8.659.584 registros válidos, 0 rechazados` = filas del CSV; `THEFT` 1.839.054 y "sin valor" 613.720 = filtrado directo |
| AC-A3 | AC-G5 (y AC-G9) en la aceptación del generador sobre la descarga, cuya primera fila es 13311263 y no 14348063 | PASS (no hizo falta la copia de respaldo de la decisión A5) |
| AC-A4 | README (sección Verify, lista de PRD, fila de `scripts/acceptance`); búsqueda de `D:\` en README, guía de worktrees y scripts; enlaces del README | PASS: los dos modos explicados, 0 `D:\`, 0 enlaces rotos |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `407d43d..9ea8828` (PRD, T1–T3) | readability, reliability | aprobada | 3 WARNING y 6 SUGGESTION (L6): 0 rechazados en el consultor, corregido; huella sin validar, corregido; aceptación sin probar, resuelto con T4; 4 sugerencias de legibilidad, corregidas; documento desactualizado, resuelto; primera fila sin número de caso, descartado porque el campo es obligatorio | `review-5265ff0ca918d340` |
| `9ea8828..f529fbe` (T5–T7) | readability, reliability | aprobada | 1 WARNING y 7 SUGGESTION (L13): camino por valor sin corrida, resuelto con T4; `$ByValue` implícito, guardia de AC-G9 (2 hallazgos), `$RowMatch` y líneas vacías, corregidos en T8; constantes de columnas y tests de los parsers, pendientes | `review-a29e96e73e4bb850` |
| `f529fbe..cd46d82` (T8) | readability, reliability | aprobada | 1 WARNING y 4 SUGGESTION (L15): corrección de B1 sin corrida, resuelto con T4; `-ByValue` opcional (2 hallazgos), pendiente; literales 22/4, pendiente; guardia sin prueba negativa, resuelto con una comprobación ejecutada (L16) | `review-df72399a8f12bea3` |

Las evaluaciones dieron riesgo medio y `under_budget`. Los tramos se revisaron al cierre por la regla del
proyecto.

## Tiempo

- Estimación: 1–2 h. Inicio real: 2026-10-16 07:05:01. Fin de la verificación: 09:15:23, antes de escribir este
  informe y del merge.
- **Duración activa: 1 h 43 min**, dentro de la estimación de 1–2 h pero en su mitad alta, a diferencia de las
  ejecuciones anteriores, que acabaron en la mitad de lo estimado. Las causas:
  - la descarga del portal tardó 23 min (07:05–07:28), con respuestas 503/500;
  - la descarga real destapó dos supuestos de formato (T6, T7);
  - el blocker B1 obligó a una tercera tanda de corridas completas (unos 22 min cada tanda de cuatro).
- Esperas por el usuario: de 07:34:19 a 08:01:20 (27 min). La causa fue una carpeta `work$3` creada por error del
  agente en la raíz del repositorio; el permiso de borrado se denegó y la borró el usuario (L10).
- Aparte: el PRD se elaboró en 28 min activos en la misma sesión, antes del ODD. Está medido en el propio PRD.

## Informe de cambios (`407d43d..cd46d82`, sin el commit de este informe)

| Área | + | − | Archivos |
|---|---|---|---|
| `scripts` | 242 | 69 | 3 |
| `docs` | 309 | 1 | 3 |
| `odd` | 159 | 0 | 1 |
| `README.md` | 25 | 11 | 1 |
| **Total** | **735** | **81** | **8** |

No hay archivos generados ni lockfiles. `docs` son el PRD en español y en inglés y la guía de worktrees. En
`scripts`: el archivo compartido nuevo `reference-extract.ps1` (+28), `run-acceptance.ps1` (+87 −20) y
`run-query-acceptance.ps1` (+127 −49). El commit de este informe también cierra el documento del feature. No había
previsión de líneas.

## Resumen

Las dos verificaciones de aceptación, la del generador y la del consultor, funcionan ahora con cualquier descarga
compatible del CSV de Chicago, no solo con el extracto de referencia. Después de generar el conjunto, comparan la
huella SHA-256 que guarda el manifiesto con la que publica el README:

- **Si coinciden:** hacen las mismas comprobaciones que antes, con las cifras exactas de los PRD y el verificador
  independiente con `--reference`.
- **Si no:** cuentan cada cifra sobre el propio CSV. Los registros válidos tienen que ser sus filas, con 0
  rechazados; `THEFT` y el área comunitaria "sin valor" salen del filtrado directo; el verificador corre sin
  `--reference`, y las filas del consultor se comparan campo a campo, porque el portal ya no entrecomilla como el
  extracto de referencia.

El informe dice en su cabecera qué caso aplicó, con la huella. La identidad del extracto de referencia vive en un
solo archivo compartido (`scripts/acceptance/reference-extract.ps1`). El README explica los dos modos, y todos los
ejemplos usan `C:\` en lugar de `D:\`.

## Valores por defecto elegidos sin consulta

- **Detalles técnicos** del documento del feature ("Implementation decisions"):
  - archivo compartido con la huella;
  - el resumen de AC-G1 con otro CSV se toma de los válidos del manifiesto, que el verificador contrasta con las
    filas del CSV;
  - hash del CSV solo si Run A falla.
- **Comparación por valor con otro CSV:** byte a byte con el extracto de referencia (S1) y campo a campo con
  cualquier otro CSV (T7), según el "equivalente en valor" del PRD del consultor. Con otro CSV, `""` y un campo
  vacío sin comillas cuentan como iguales.
- **Cambios en la salida con el extracto de referencia**, salidos de la primera revisión (L6) y señalados por el
  verificador. No se quitó ninguna comprobación:
  - la carpeta del conjunto completo del consultor se llama `full`, y las corridas anteriores dejaban `reference`,
    que ahora se limpia;
  - una fila nueva `Full dataset: 0 rejected` lleva el total de 58 a 59 comprobaciones.

## Pendientes (no bloqueantes)

- **`-ByValue` opcional (R2-001/R3-002 de la tercera revisión):** una llamada nueva con dos argumentos compararía
  byte a byte. Las dos llamadas actuales lo pasan.
- **Constantes de columnas (R2-002):** el diseño de 22 columnas aparece como literales (22, 0, 2, 4) junto a sus
  comentarios. Si cambia el esquema, los guardias de arranque lo detectan.
- **Sin tests de fixture para los parsers (R3-parsers-without-fixture-tests):** los scripts de aceptación no tienen
  arnés de tests. Los dos parsers se ejercitan en corridas completas con los dos formatos de CSV y en la prueba de
  identidad byte a byte con el extracto de referencia.
- **Medición fechada del README:** la línea "all 58 checks of the 18 criteria" (2026-10-13) refleja aquella corrida;
  hoy son 59 comprobaciones con el extracto de referencia.
- **Espacio en disco fijo y CSV con filas rechazadas:** fuera de alcance según el PRD.

**Riesgo:** ítem 6. Ningún test automático cubre los scripts de aceptación: una regresión solo aparece al
ejecutarlos. Lo cubren las corridas RED/GREEN sobre los dos CSV reales, tres revisiones RDD y un verificador
independiente.

## Limpieza manual

Todo en `work/` (ignorado por git), unos 39 GB en total; se puede borrar sin afectar al repositorio:

- La descarga del portal, `work/portal-download/` (2,0 GB). Consérvala si quieres repetir AC-A2 con la misma
  descarga, porque el portal cambia a diario.
- Las carpetas de las corridas, de 2,1 a 3,3 GB cada una, con sus `.log`:
  - `acc-ref-gen`, `acc-ref-query`;
  - `red-gen`, `red-query`;
  - `new-gen`, `new-query`, `new-query2`;
  - `final-gen-ref`, `final-query-ref`, `final-query-ref2`;
  - `final-gen-dl`, `final-query-dl`, `final-gen-ref3`, `final-query-ref3`.

  Los informes de las cuatro corridas finales están en `final-gen-dl`, `final-query-dl`, `final-gen-ref3` y
  `final-query-ref3`.
- Los scripts de la cadena, `work/chain.sh`, `chain2.sh` y `chain3.sh`.
- Las carpetas temporales de las sondas, fuera del repositorio: `C:\Users\jesus.lopez\AppData\Local\Temp\tmp.FvapyeX9iq`
  (196 KB, del verificador) y la de la prueba del guardia (1 KB).
