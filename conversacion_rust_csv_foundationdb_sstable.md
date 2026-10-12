# CSV masivo a SSTable con Rust, FoundationDB Tuple y ordenación externa

> Documento organizado a partir de nuestra conversación. Los ejemplos son propuestas de diseño y **no se han compilado ni validado** en esta conversación; antes de usarlos deben contrastarse con las versiones concretas de los crates.

## Objetivo

Procesar un CSV de delitos de Chicago que no cabe en RAM, ordenarlo por `ID` **numérico ascendente** y generar una **SSTable** con:

- **Key:** tupla FoundationDB que contiene el `ID`.
- **Value:** tupla FoundationDB con los campos del CSV, convertidos a tipos adecuados.
- **Restricción operativa:** objetivo de memoria de **512 MiB** sin construir un ordenador externo complejo.

El archivo tiene **22 columnas**, con esta cabecera:

```csv
"ID","Case Number","Date","Block","IUCR","Primary Type","Description","Location Description","Arrest","Domestic","Beat","District","Ward","Community Area","FBI Code","X Coordinate","Y Coordinate","Year","Updated On","Latitude","Longitude","Location"
```

Ejemplo de fila:

```csv
"14348063","JK438266","10/03/2026 12:00:00 AM","064XX S ELLIS AVE","1090","ARSON","ATTEMPT ARSON","VEHICLE NON-COMMERCIAL","false","false","0314","003","20","42","09","1184164","1862455","2026","10/10/2026 03:42:19 PM","41.777750747","-87.600396201","(41.777750747, -87.600396201)"
```

**Origen del dataset:** Chicago, no Nueva York. La cabecera coincide con *Crimes — 2001 to Present* de la ciudad de Chicago: https://data.cityofchicago.org/Public-Safety/Crimes-2001-to-Present/ijzp-q8t2. Los códigos `IUCR`, `Beat`, `Ward` y `Community Area` y las coordenadas son indicadores característicos.

## Decisiones de diseño

### 1. Ordenación por ID numérico

No se comparan los textos del ID: se hace `parse::<i64>()` y se comparan enteros. Así `2 < 10 < 100`, a diferencia de la ordenación lexicográfica de cadenas.

La codificación *Tuple Layer* de FoundationDB permite conservar el orden de enteros al comparar las **claves serializadas** lexicográficamente.

### 2. Ordenación externa

Se consideraron `extsort` y `external_sort`. Para una implementación simple se prefirió **`extsort`**, porque permite definir la comparación, configurar un tamaño de segmento y procesar registros mediante iteración, usando almacenamiento temporal cuando los datos exceden la RAM.

Se valoró también `ext-sort` por su aproximación a buffers limitados por memoria, pero la decisión fue priorizar simplicidad.

**Nota:** las versiones, las firmas exactas de los métodos y las garantías de memoria de los crates deben comprobarse antes de compilar.

### 3. Representación de la entrada durante la ordenación

No conviene mantener 22 `String` ni un `Vec<Vec<u8>>` por fila si buscamos eficiencia. Es preferible **codificar la tupla del valor al leer el CSV** y ordenar una estructura mínima:

```rust
struct Entry {
    id: i64,
    value: Vec<u8>,
}
```

El comparador trabaja directamente con `id`:

```rust
|a: &Entry, b: &Entry| a.id.cmp(&b.id)
```

Así se evita decodificar FoundationDB Tuple durante la ordenación. Al escribir la SSTable se codifica el `id` como tupla para la clave; el valor ya está serializado.

### 4. Esquema tipado propuesto

| Campo | Rust recomendado | Tupla FDB | Observación |
|---|---|---|---|
| ID | `i64` | Integer | Clave; puede omitirse del valor si el contrato lo permite |
| Case Number | `String` | String | Identificador, no número |
| Date | `i64` | Integer | Timestamp de época en milisegundos; interpretar zona horaria de Chicago |
| Block | `String` | String | Texto |
| IUCR | `String` | String | Código; conservar ceros |
| Primary Type | `String` | String | Texto |
| Description | `String` | String | Texto |
| Location Description | `String` | String | Texto |
| Arrest | `bool` | Boolean | `true`/`false` |
| Domestic | `bool` | Boolean | `true`/`false` |
| Beat | `String` | String | Código con ceros iniciales |
| District | `String` | String | Código con ceros iniciales |
| Ward | `Option<i64>` | Integer / Nil | Admite vacío |
| Community Area | `Option<i64>` | Integer / Nil | Admite vacío |
| FBI Code | `String` | String | Código |
| X Coordinate | `Option<i64>` | Integer / Nil | Admite vacío |
| Y Coordinate | `Option<i64>` | Integer / Nil | Admite vacío |
| Year | `i64` | Integer | Año numérico |
| Updated On | `i64` | Integer | Timestamp en milisegundos |
| Latitude | `Option<f64>` | Double / Nil | Admite vacío |
| Longitude | `Option<f64>` | Double / Nil | Admite vacío |
| Location | `String` | String | Redundante respecto a coordenadas; conservar si lo exige el contrato |

**Precauciones:** la conversión de fechas requiere decidir zona horaria y reglas de horario de verano. `f64` puede cambiar la representación textual original de coordenadas. Los nulos deberían representarse explícitamente con `Nil`.

Para eficiencia, crear una **tupla plana** con los campos seleccionados, no una tupla accidentalmente anidada con un único elemento contenedor.

### 5. Lectura y codificación

- Usar `csv::ByteRecord` y reutilizar su buffer durante el recorrido.
- Convertir los campos numéricos y booleanos según el esquema.
- Tratar las cadenas como referencias temporales a los bytes del registro cuando sea posible.
- Codificar directamente los valores a un buffer `Vec<u8>` sin crear 22 cadenas propietarias.
- Mantener los pares `(id, value)` para `extsort`.

### 6. SSTable

El escritor recibe pares clave/valor **ya ordenados**:

```text
Key   = pack((id,))
Value = pack((case_number, date, block, ..., longitude, location))
```

Por diseño la clave debe aparecer **en orden estrictamente creciente**. Para IDs duplicados se propuso **rechazar la entrada** en lugar de sobrescribir silenciosamente. Es importante validar esta condición antes o durante la escritura.

**Importante:** SSTable no es un formato binario único. Una SSTable de estilo LevelDB no equivale necesariamente a un archivo de ingestión externa de RocksDB. Elegir el escritor en función del sistema que consumirá el archivo.

## Presupuesto de RAM: 512 MiB

A partir de la muestra, se estimó orientativamente:

| Concepto | Bytes aproximados por fila |
|---|---:|
| Tupla FoundationDB | 180–250 |
| `id: i64` | 8 |
| Cabecera de `Vec<u8>` | 24 |
| Asignador, capacidad y margen | 20–60 |
| Total ilustrativo | 232–342 |

Las cifras anteriores **no son mediciones**. Para dimensionar con margen se adoptaron **512 bytes por registro**.

Con `250_000` registros por segmento:

- Presupuesto ilustrativo: `250_000 × 512 = 128_000_000 bytes` ≈ **122,1 MiB**.
- Margen teórico frente a 512 MiB: ≈ **389,9 MiB** para el resto de estructuras y picos.

Configuración propuesta:

```rust
const SEGMENT_SIZE: usize = 250_000;
```

Comparador propuesto:

```rust
let mut sorter = ExternalSorter::new()
    .with_segment_size(SEGMENT_SIZE)
    .pushed_by(|a: &Entry, b: &Entry| a.id.cmp(&b.id));
```

**Cuidado:** el límite por número de registros **no garantiza** un máximo estricto de memoria. Filas atípicamente grandes, el sort interno y las estructuras auxiliares pueden incrementar el pico. La elección de 250.000 es un punto de partida conservador que debe medirse con muestras reales. Inicialmente, sin ordenación paralela para evitar posibles picos añadidos.

## Flujo propuesto

```text
CSV de entrada (streaming)
      |
      v
csv::ByteRecord reutilizable
      |
      v
Validar 22 columnas y parsear tipos
      |
      v
Entry { id: i64, value: bytes de tupla FDB }
      |
      v
extsort: ordenar por id en segmentos de 250.000
      |        
      +--> segmentos temporales en SSD
      |
      v
Fusión secuencial ordenada
      |
      v
Clave pack((id,)) + valor ya codificado
      |
      v
SSTable writer
      |
      v
Archivo SSTable definitivo
```

## Mejoras sencillas para producción

1. **Cabecera y esquema:** validar columnas, su orden y sus nombres; versionar el contrato de la tupla del valor.
2. **Errores y duplicados:** fallar si `ID` no es numérico, si un campo tipado no se puede convertir o si aparecen claves duplicadas.
3. **Archivo temporal de salida:** generar la SSTable en un archivo provisional y renombrarlo tras finalizar correctamente.
4. **Directorio de temporales:** poner los segmentos de ordenación en SSD con espacio suficiente.
5. **Compresión:** medir SSTable con y sin Snappy; hay textos y categorías repetitivos.
6. **Medición:** probar sobre una muestra representativa para medir memoria máxima y rendimiento.
7. **Datos ya ordenados:** si la fuente garantiza IDs crecientes, se puede omitir completamente el sort externo y escribir la SSTable en una pasada.

## Guía rápida de Rust para alguien que conoce C#

| Rust | Idea comparable en C# |
|---|---|
| `use` | `using` |
| `crate` | paquete/dependencia |
| `Vec<u8>` | `List<byte>` o buffer variable |
| `struct Entry { ... }` | tipo con campos |
| `trait` | interfaz (no equivalente exacto) |
| `impl Trait for Type` | implementación de interfaz |
| `let` / `let mut` | variable inmutable / mutable |
| `&T` | referencia prestada de lectura |
| `&mut T` | referencia prestada mutable |
| `move` en closure | captura por valor |
| `Option<T>` | valor opcional / nullable |
| `Result<T, E>` | éxito o error explícito |
| operador `?` | propagar el error inmediatamente |
| `.iter().map(...).collect()` | operaciones sobre secuencias al estilo LINQ |

La diferencia clave respecto a C# es **ownership / borrowing**: Rust controla en compilación qué parte del código posee un valor y quién puede tomarlo prestado. Al pasar `Entry` por valor al sorter, normalmente se transfiere su propiedad sin copiar toda la fila.

## Estado actual de la decisión

**Solución preferida:** `csv` + tipos adecuados + codificación FoundationDB Tuple + `extsort` con segmentos de **250.000 registros**, comparando `i64`, y escritura secuencial con el writer de SSTable apropiado para el consumidor.

**Pendiente de concretar antes del programa final:**

- Motor/formato exacto de SSTable de destino (LevelDB, RocksDB u otro).
- Si el valor omite `ID` porque ya aparece en la clave.
- Si se mantiene `Location` a pesar de duplicar coordenadas.
- Política de fechas, nulos y registros erróneos.
- Comprobación de las APIs exactas de las versiones seleccionadas.

## Referencias

- Chicago Crimes — 2001 to Present: https://data.cityofchicago.org/Public-Safety/Crimes-2001-to-Present/ijzp-q8t2
- FoundationDB tuple layer: https://apple.github.io/foundationdb/data-modeling.html
- Documentación de crates Rust: https://docs.rs/
