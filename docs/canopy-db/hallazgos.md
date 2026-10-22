# Registro de hallazgos de CanopyDB

Registro común del módulo CanopyDB ([PRD del módulo](../prd/chicago-crime/canopy-db.prd.md), sección "Registro
de hallazgos"; forma fijada por la decisión KG9 del [PRD del generador
CanopyDB](../prd/chicago-crime/canopy-db-generador.prd.md)). Cada comportamiento de CanopyDB que se aparte de lo que
su documentación promete o de lo que exige el contrato del proyecto tiene aquí una entrada con su reproducción
mínima y la versión afectada (flujo E del PRD del módulo).

- Los identificadores siguen el orden de alta y nunca se reutilizan.
- Ninguna acción pública fuera del proyecto (issue, PR, comentario, fork o commits en el fork) se hace sin la
  aprobación del responsable del proyecto, una por una (decisión K4); cada aprobación se anota en su entrada.
- Los issues y PR se redactan en inglés, el idioma del repositorio de CanopyDB.
- Un campo sin contenido lleva `—`.

## Reproducciones

Las reproducciones de los hallazgos que no son de documentación están en [`repro/`](repro/): un programa que solo
depende de CanopyDB y de la biblioteca estándar, fuera del workspace del proyecto. Se ejecutan con una orden desde
esa carpeta:

```text
cargo run -- h2
cargo run -- h3
```

Cada una trabaja en una carpeta nueva de la carpeta temporal del sistema, muestra lo que observa y termina con
`REPRODUCED` y el código 0 si reproduce el comportamiento del hallazgo, o con `NOT REPRODUCED` y el código 1 si no.
La de H5 mide un coste, así que se ejecuta en modo optimizado: `cargo run --release -- h5`.
[`repro/patches/`](repro/patches/) guarda los prototipos de las contribuciones posibles, como parches sobre el
código fuente publicado de CanopyDB; cada entrada explica cómo aplicar el suyo y medirlo.

## Resumen

| Id | Tipo | Estado | Título |
|---|---|---|---|
| H1 | Documentación | Detectado | El tercer ejemplo del README configura la caché, pero no usa esas opciones |
| H2 | Limitación | Detectado | Dos procesos no pueden abrir a la vez la misma base |
| H3 | Limitación | Detectado | No hay apertura de solo lectura |
| H4 | Documentación | Detectado | El tamaño por defecto de la caché no aparece en la portada de la documentación |
| H5 | Rendimiento | Detectado | No hay carga masiva: datos ya ordenados solo se escriben con inserciones una a una |

## H1 — El tercer ejemplo del README configura la caché, pero no usa esas opciones

- **Identificador**: H1
- **Fecha**: 2026-10-20
- **Versión de CanopyDB**: 0.2.5
- **Tipo**: Documentación
- **Descripción y efecto**: el tercer ejemplo del README ("Multiple Databases with Atomic Cross-Database
  Commits"), que también es la portada de la documentación en docs.rs, crea unas `EnvOptions` con un tamaño de
  caché, pero abre el entorno con `Environment::new`, que no recibe esas opciones. El ajuste no tiene efecto, y
  quien copie el ejemplo cree que ha limitado la memoria cuando la caché sigue con su valor por defecto. Para este
  módulo, que tiene un límite de 512 MiB, es justo el ajuste que hay que hacer bien (ver H4).
- **Reproducción mínima**: cita del README de CanopyDB 0.2.5, líneas 151 a 155:

  ```rust
  let _dir = tempfile::tempdir().unwrap();
  let mut options = canopydb::EnvOptions::new(_dir.path());
  // all databases in the same environment will share this 1GB cache
  options.page_cache_size = 1024 * 1024 * 1024;
  let env = canopydb::Environment::new(_dir.path()).unwrap();
  ```

  `options` no se vuelve a usar. La forma que aplica el ajuste es `Environment::with_options(options)`.
- **Estado**: Detectado
- **Aprobaciones**: —
- **Enlaces**: —
- **Fork**: —

## H2 — Dos procesos no pueden abrir a la vez la misma base

- **Identificador**: H2
- **Fecha**: 2026-10-20
- **Versión de CanopyDB**: 0.2.5
- **Tipo**: Limitación
- **Descripción y efecto**: CanopyDB admite muchos lectores a la vez dentro de un mismo proceso, pero abrir una
  base toma un bloqueo exclusivo de su carpeta. Si otro proceso la tiene abierta, la apertura espera
  `file_lock_timeout` (5 segundos por defecto) y falla con `EnvironmentLocked`. Para el módulo, mientras un
  consultor CanopyDB tiene un conjunto abierto, otro consultor del mismo conjunto espera y se rechaza (decisión
  K6), y el verificador y las consultas de la verificación se ejecutan de uno en uno.
- **Reproducción mínima**: `cargo run -- h2` en [`repro/`](repro/). El proceso A abre la base y lanza el proceso
  B, que intenta abrirla. Resultado en el equipo de referencia (Windows 11) el 2026-10-21:

  ```text
  process B could not open the base after 5.0 s: EnvironmentLocked (EnvironmentLocked)
  process B ended with code Some(3) after 5.0 s
  REPRODUCED
  ```
- **Estado**: Detectado
- **Aprobaciones**: —
- **Enlaces**: —
- **Fork**: —

## H3 — No hay apertura de solo lectura

- **Identificador**: H3
- **Fecha**: 2026-10-20
- **Versión de CanopyDB**: 0.2.5
- **Tipo**: Limitación
- **Descripción y efecto**: CanopyDB no tiene un modo de apertura de solo lectura. Abrir una base para leerla abre
  sus archivos internos para escritura, así que una base en una carpeta de solo lectura no se puede abrir; y si
  la base se cerró sin volcar su registro de escritura anticipada, solo leerla reorganiza ese registro. Además, al
  abrir una base de datos con unas opciones distintas de las guardadas, CanopyDB reescribe su archivo `OPTIONS`
  con las del que abre. Para el módulo, el consultor CanopyDB puede escribir en los archivos internos del motor
  (decisión K6), el generador publica la base ya volcada y sin registro de escritura anticipada, y el verificador
  abre la base con las opciones guardadas para no cambiar nada.
- **Reproducción mínima**: `cargo run -- h3` en [`repro/`](repro/). Primero intenta leer una base cuyos archivos
  son de solo lectura; después abre solo para leer una base cerrada sin volcado y compara sus archivos antes y
  después. Resultado en el equipo de referencia (Windows 11) el 2026-10-21:

  ```text
  a) the read-only base cannot be opened: Io(Os { code: 5, kind: PermissionDenied, message: "Access is denied." }) (...)
  b) read 1000 entries without writing
  b) WAL-0000000000: 1048576 bytes -> 34121 bytes
  REPRODUCED
  ```

  La reescritura de `OPTIONS` se observó en la prueba previa del generador: abrir con las opciones por defecto
  una base escrita con otras cambió `OPTIONS` de 300 a 299 bytes (registro de la ejecución
  [`canopy-db-generador`](../../odd/tasks/canopy-db-generador.md), L3).
- **Estado**: Detectado
- **Aprobaciones**: —
- **Enlaces**: —
- **Fork**: —

## H4 — El tamaño por defecto de la caché no aparece en la portada de la documentación

- **Identificador**: H4
- **Fecha**: 2026-10-20
- **Versión de CanopyDB**: 0.2.5
- **Tipo**: Documentación
- **Descripción y efecto**: la caché de páginas crece bajo demanda hasta 1 GiB por defecto, pero la portada de la
  documentación (el README, que docs.rs muestra como página principal) no lo dice: solo lo menciona el comentario
  del ejemplo de H1, como si fuera un valor elegido. El dato condiciona cualquier presupuesto de memoria. En la
  prueba previa del generador, leer la base completa con las opciones por defecto llegó a un pico de 1.161 MiB,
  frente a 73 MiB con la caché limitada a 64 MiB (registro de la ejecución
  [`canopy-db-generador`](../../odd/tasks/canopy-db-generador.md), L3).
- **Reproducción mínima**: cita de la documentación de la opción en CanopyDB 0.2.5 (`EnvOptions::page_cache_size`,
  archivo `src/options.rs`), el único sitio que da el valor por defecto:

  ```text
  Database pages read from the Database files are cached in the page cache.

  Note that all databases in the same Environment share the page cache.

  Default: `1 GB`
  ```

  El README no menciona ningún valor por defecto de la caché.
- **Estado**: Detectado
- **Aprobaciones**: —
- **Enlaces**: —
- **Fork**: —

## H5 — No hay carga masiva: datos ya ordenados solo se escriben con inserciones una a una

- **Identificador**: H5
- **Fecha**: 2026-10-21
- **Versión de CanopyDB**: 0.2.5
- **Tipo**: Rendimiento
- **Descripción y efecto**: la única forma de escribir en un árbol es `Tree::insert`, una entrada cada vez dentro
  de una transacción. No hay una carga masiva que construya el árbol de abajo arriba a partir de entradas ya
  ordenadas, como `MDB_APPEND` en LMDB, ni un modo de añadir al final. Cada entrada baja desde la raíz por todos
  los niveles del árbol, aunque llegue en orden. CanopyDB sí optimiza la división de una hoja llena cuando la clave
  es mayor que todas las suyas (`node_split_right` en `src/tree.rs`, "Optimize for sequential inserts"): la hoja
  queda llena y la clave va a una página nueva. Pero no optimiza el descenso, y los nodos rama se dividen por la
  mitad. Para el módulo, el generador CanopyDB ordena los registros y los índices igual que el original, pero no
  puede aprovecharlo:
  - el generador completo tarda 1 min 41 s en su mejor medida, frente a 65–75 s del original, que escribe sus
    SSTables de forma secuencial (aceptación del 2026-10-21, Run A);
  - la base ocupa 2.718.945.280 bytes, frente a 1,1 GB de las SSTables con Snappy;
  - solo la escritura de los seis árboles con el volumen del conjunto tardó 66,6 s en la prueba previa
    (registro de la ejecución [`canopy-db-generador`](../../odd/tasks/canopy-db-generador.md), L3, L11 y L15);
  - en un perfil del 2026-10-22 (la carga de la reproducción mínima con los límites de memoria del generador y
    contadores de ciclos en una copia de CanopyDB), el 99,7 % del tiempo está dentro de `Tree::insert`: los
    `commit` suman 0,1 s y el checkpoint final menos de 0,1 s, porque el volcado corre en otro hilo. El árbol
    tiene cuatro niveles; en cada inserción, la hoja se lleva el 27–29 % del tiempo y el descenso por los tres
    niveles de nodos rama, el resto.

  Las cifras no son de un banco controlado; las mide `canopy-db-comparacion`. Medición del banco del
  2026-10-22 con el extracto de referencia y 5 repeticiones
  ([informe](comparisons/2026-10-22-130851-82ffd76.md)): la base ocupa 2.718.946.733 bytes frente a
  1.101.808.809 de las SSTables, y la generación tarda 204,8 s de mediana frente a 156,4 s; en la ejecución
  anterior ([informe](comparisons/2026-10-22-120128-fea8c09.md)), 180,0 s frente a 111,6 s. Los tiempos de las
  dos ejecuciones difieren más que la tolerancia del banco por carga externa en el equipo, así que la
  proporción entre los dos motores es más fiable que las cifras absolutas.
- **Reproducción mínima**: `cargo run --release -- h5` en [`repro/`](repro/). Escribe con `Tree::insert`, en
  orden, el volumen del conjunto (8.656.904 entradas de 138 bytes y cinco árboles de índices con otras tantas),
  con la configuración del generador, y muestra el tiempo y el tamaño de la base. Resultado en el equipo de
  referencia (Windows 11) el 2026-10-21:

  ```text
  h5) 51941424 sorted entries in 6 trees written with Tree::insert in 31.7 s; base size 3284664620 bytes
  REPRODUCED
  ```

  La misma carga tardó 66,6 s en la prueba previa: la variación entre ejecuciones no está controlada, y es otra
  razón para medirla en el banco. La ausencia de la API se ve en
  la documentación de `Tree` de CanopyDB 0.2.5: su único método de escritura de una entrada es `insert`.
- **Posible contribución**: dos, de menor a mayor alcance. El generador CanopyDB usaría cualquiera de ellas sin
  cambiar su contrato, y el banco de comparación mediría la diferencia. Antes de publicar un issue o un PR hace
  falta la aprobación del responsable del proyecto (decisión K4).
  1. **Un camino rápido para añadir al final**, sin API nueva. El prototipo está en
     [`repro/patches/h5-append-fast-path.patch`](repro/patches/h5-append-fast-path.patch) (142 líneas añadidas a
     `src/tree.rs` y `src/lib.rs` de la 0.2.5). `Tree` recuerda su hoja de la derecha. Una clave mayor que la
     última de esa hoja que cabe en ella se inserta directamente, sin bajar por los nodos rama. Si no cabe, o el
     prefijo de la hoja no encaja, la inserción toma el camino normal y después se busca otra vez la hoja de la
     derecha. Los borrados, `delete_range`, `clear` y la compactación olvidan la hoja. Una inserción que no es al
     final desactiva el camino rápido hasta el final de la transacción. No se aplica a valores que van a páginas
     aparte ni a las transacciones concurrentes. La variable de entorno `CANOPY_APPEND=0` lo desactiva, para
     comparar con el mismo binario.
     - Verificación del 2026-10-22, con el camino rápido activo:
       - Los tests de CanopyDB pasan: 57 correctos, 0 fallidos y 1 ignorado, que ya lo estaba. El paquete publicado
         no trae `assets/gh_events.json`, los datos de sus tests de compresión, así que se usó un JSON sintético.
       - Una comparación aleatoria con un `BTreeMap` no encontró diferencias en 14 semillas. Mezcla inserciones al
         final y en medio, sobrescrituras, borrados, `delete_range`, `clear`, rollbacks, valores grandes y
         transacciones con 256 KiB de memoria, que obligan a volcar nodos.
       - La carga completa de H5 se leyó de vuelta en orden.
     - Medida con la reproducción mínima, tres pares alternados el 2026-10-22: 39,0 / 35,3 / 33,2 s sin el camino
       rápido y 14,7 / 14,1 / 13,4 s con él, unas 2,5 veces más rápido. La base ocupa lo mismo (3.284.644.140
       bytes): el camino rápido no reduce el tamaño. Para medirlo, desde la raíz del repositorio, en PowerShell:

       ```powershell
       $dir = Join-Path $env:TEMP h5-append   # a folder that does not exist yet
       New-Item -ItemType Directory $dir | Out-Null
       $canopy = Resolve-Path "$env:USERPROFILE\.cargo\registry\src\*\canopydb-0.2.5" | Select-Object -First 1
       Copy-Item -Recurse $canopy.Path "$dir\canopydb"
       New-Item -ItemType Directory "$dir\repro" | Out-Null
       Copy-Item -Recurse docs\canopy-db\repro\Cargo.toml, docs\canopy-db\repro\Cargo.lock, docs\canopy-db\repro\src "$dir\repro"
       git -C "$dir\canopydb" apply (Resolve-Path docs\canopy-db\repro\patches\h5-append-fast-path.patch).Path
       Push-Location "$dir\repro"
       $patch = "patch.crates-io.canopydb.path='$dir\canopydb'"
       $env:CANOPY_APPEND = "0"; cargo run --release --quiet --config $patch -- h5
       $env:CANOPY_APPEND = "1"; cargo run --release --quiet --config $patch -- h5
       Remove-Item Env:CANOPY_APPEND
       Pop-Location
       ```

       El código fuente de CanopyDB 0.2.5 está en el registro de Cargo después de compilar `repro/` una vez. La
       medida trabaja sobre una copia de `repro/` porque enlazar la copia parcheada cambia su `Cargo.lock`.
  2. **Una API de carga masiva** para un árbol vacío (o de añadido al final con claves crecientes) que escriba las
     hojas llenas de forma secuencial y construya los niveles superiores después. Ganaría más que el camino rápido
     y es la única de las dos que reduciría el tamaño de la base, porque también llenaría los nodos rama.
- **Estado**: Detectado
- **Aprobaciones**: —
- **Enlaces**: —
- **Fork**: —
