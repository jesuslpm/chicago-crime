# Registro de escenarios de robustez

Resultados de los escenarios R1 a R13 del [PRD de robustez](../prd/chicago-crime/canopy-db-robustez.prd.md)
(sección "Registro de escenarios"), en cada sistema. Los ejecuta `crime-canopy-scenarios` (ver el
[README](../../README.md#robustness-scenarios)), que escribe las filas de cada ejecución en
`<carpeta de trabajo>/registro-<Sistema>-<fecha>.md`.

- Cada vez que se pasan los escenarios se añaden filas nuevas al final; las anteriores no se cambian, para
  comparar versiones de CanopyDB.
- Una fila por escenario, variante y sistema. Una variante que no se ejecutó no se anota: se repite hasta
  ejecutarse.
- **Resultado**: `Sin bug`; `Bug del proyecto` con el commit que lo corrige (la fila de la repetición posterior
  dice `Sin bug`); `Hallazgo` con los identificadores del [registro de hallazgos](hallazgos.md); o `No aplica
  en este sistema`. El ejecutor deja como "por clasificar" las variantes no cumplidas: se clasifican antes de
  añadirlas.
- **Sistema**: `Windows` es Windows 11; `Linux` es Ubuntu 24.04 sobre WSL2, con el código y los datos en el
  sistema de archivos de Linux.

| Escenario | Sistema | Fecha | Versión de CanopyDB | Commit del proyecto | Resultado | Observado |
|---|---|---|---|---|---|---|
