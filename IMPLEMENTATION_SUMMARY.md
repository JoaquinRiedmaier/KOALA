# KOALA — Historial de Mediciones de Ping

Plan de implementación. La Fase 1 está implementada; las fases 2 y 3 son opcionales.

## 1) Objetivo

Al iniciar sesión, KOALA hace **una medición** y la guarda en un **archivo persistente**, junto con las que el usuario pida después. No hay mediciones periódicas: fuera de la del inicio, solo se mide a pedido. La interfaz gráfica (`koala gui`) muestra un **historial** de esas mediciones, con **filtro por empresa** (proveedor: Google, Cloudflare, Facebook, Instagram, etc.).

Hay un único archivo, que se crea en la primera medición y se conserva entre reinicios. No se crea uno nuevo en cada encendido.

## 2) Alcance funcional

| # | Requisito | Cómo se cumple |
|---|-----------|----------------|
| 1 | Al iniciar se registra un valor | Timer systemd de usuario `koala-medir.timer`, una vez, 60 s después de iniciar sesión |
| 2 | Los valores se guardan en un archivo de historial | JSONL en `~/.local/state/koala/history.jsonl`; cada medición agrega una línea |
| 3 | Mediciones siguientes | Solo manuales: clic en el widget, abrir la GUI o "Re-medir" |
| 4 | La GUI muestra el historial | Vista nueva **HISTORIAL** en `koala gui` (tabla con scroll) |
| 5 | Filtrar por empresa | ComboBox con la unión de nombres presentes en el historial (`TODAS` por defecto) |

## 3) Antecedentes del código

- Cada medición produce un `EstadoGlobal` (`src/core/verdict.rs`) con `ts`, `estado`, `media_ms`, `peor_perdida` y el detalle por servidor. `~/.cache/koala/state.json` guarda solo la última: no queda historia.
- El refresco automático (`AUTO_REFRESH`) está apagado a pedido del usuario: se mide al hacer clic en el widget o al abrir la GUI. Este plan no cambia eso; solo agrega la medición del inicio.
- `koala instalar` se ejecuta en cada inicio de Hyprland (hook del usuario en `hyprland.lua`). Todo lo que agregue tiene que ser idempotente: si no hay nada que cambiar, no escribe ni recarga nada.
- El lock de medición (`cache::tomar_lock`) serializa las mediciones. El append se hace mientras se tiene el lock.

## 4) Diseño del archivo de historial

### Ruta

- `$XDG_STATE_HOME/koala/history.jsonl` — por defecto `~/.local/state/koala/history.jsonl`.
- Override: variable de entorno `KOALA_HISTORIAL`.
- Se usa `~/.local/state` y **no** `~/.cache`, porque el caché se puede borrar en cualquier momento.

### Formato: JSON Lines (append-only)

Una medición por línea, JSON del `RegistroHistorial`. Nunca se reescribe el archivo entero por cada medición.

### Registro reducido (no el `EstadoGlobal` completo)

El `EstadoGlobal` incluye las 28 IPs y dominios individuales, algo pesado e innecesario para un historial por empresa. Se guarda esta proyección, con latencias redondeadas a 0,1 ms:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmpresaHistorial {
    pub nombre: String,
    pub media: Option<f32>,
    pub jitter: Option<f32>,
    pub peor_perdida: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RegistroHistorial {
    pub ts: u64,
    pub estado: Estado,
    pub media_ms: Option<f32>,
    pub peor_perdida: u8,
    pub empresas: Vec<EmpresaHistorial>,
}

impl From<&EstadoGlobal> for RegistroHistorial { /* descarta las ips individuales */ }
```

El estado por empresa no se guarda: se deriva de `media`, `jitter` y `peor_perdida` con la misma regla que `Proveedor::etiqueta()`, compartida en `verdict::etiqueta_proveedor`.

Tamaño: unos 820 bytes por línea con las 11 empresas actuales.

### Retención

- Hay un solo límite: `HISTORIAL_LIMITE_BYTES = 8 MiB`, unas 10.000 mediciones.
- Al superarlo se **compacta**: se conserva el 75 % más reciente del límite, cortando en un límite de línea.
- La compactación escribe un archivo temporal y lo renombra, para no perder el historial si hay un apagón a mitad de camino.
- El chequeo es solo el tamaño del archivo, sin leerlo, por lo que es barato en cada append.

### Tolerancia a fallos

- Una línea corrupta se ignora al leer, sin afectar al resto.
- El archivo se lee como bytes (`from_utf8_lossy`): con `read_to_string`, un solo byte inválido descartaría todo el historial.
- Si un apagón deja una línea sin terminar, el siguiente append agrega antes un salto de línea. Sin esto, la línea nueva se pegaría a la rota y también se perdería.

## 5) Arranque automático

Lo agrega `koala instalar` (`src/install.rs`, paso 5/5) con systemd de usuario. `ExecStart` usa la ruta absoluta del binario en uso, igual que el módulo de waybar.

`~/.config/systemd/user/koala-medir.service`:

```ini
[Unit]
Description=KOALA: registra una medición en el historial

[Service]
Type=oneshot
ExecStart=<ruta del binario> medir
TimeoutStartSec=60
```

`~/.config/systemd/user/koala-medir.timer`:

```ini
[Unit]
Description=KOALA: medición al iniciar sesión

[Timer]
OnStartupSec=60s
AccuracySec=15s

[Install]
WantedBy=timers.target
```

Decisiones:

- **No hay un servicio que mida en `default.target`.** Al iniciar sesión la red puede no estar lista y la primera fila sería un `SIN RED` falso. Los usuarios de systemd tampoco tienen `network.target` ni `network-online.target`, así que no se puede esperar a la red con una dependencia. Por eso se esperan 60 s.
- **`OnStartupSec`, no `OnBootSec`.** `OnStartupSec` cuenta desde que arranca el systemd del usuario (el login). `OnBootSec` cuenta desde el encendido de la máquina y puede estar vencido al iniciar sesión.
- **Un solo disparo.** Sin `OnUnitActiveSec` ni `OnCalendar`, el timer dispara una vez cada vez que arranca el systemd del usuario. Un test lo controla.
- **`[Install]` es obligatorio en el timer.** Sin `WantedBy=timers.target`, `systemctl enable` no tiene nada que habilitar.
- **Nombre `koala-medir.service`.** No se reutiliza `koala.service`, que corresponde a la versión vieja con `barra`. Si `koala instalar` encuentra esa unidad, la deshabilita y la borra.
- **Sin `--now`.** `enable --now` mediría en el momento de instalar, porque `OnStartupSec` ya estaría vencido. Con `enable` solo, la primera medición llega en el próximo login.
- **Idempotente.** Las unidades se escriben solo si cambiaron, y `daemon-reload` y `enable` corren solo en ese caso. Si el usuario deshabilita el timer, un inicio posterior no lo vuelve a habilitar. Si `systemctl` falla, se borra el timer recién escrito para reintentar en la próxima ejecución.
- **Solo systemd.** Sin `systemctl`, el resto de la instalación sigue y se avisa del fallo.
- **Sin `linger`.** El timer depende del systemd del usuario, que arranca en el primer login y corre en cualquier escritorio (Hyprland, GNOME). Una nueva sesión mide de nuevo solo si antes se cerraron todas.

`scripts/install.sh` no escribe unidades: llama a `koala instalar`, así hay un solo lugar que las define.

## 6) Cambios por archivo

| Archivo | Cambio |
|---------|--------|
| `src/core/histo.rs` (**nuevo**) | `agregar`, `cargar`, `nombres_empresas`, compactación y ruta. Tests: crear y agregar, recorte por tamaño, líneas corruptas, línea sin terminar y proyección |
| `src/core/mod.rs` | `pub mod histo;` |
| `src/core/verdict.rs` | `etiqueta_proveedor(media, jitter, peor_perdida)`, compartida con `Proveedor::etiqueta()` |
| `src/app.rs` | En `medir()`, `histo::agregar(&estado)` antes de soltar el lock. Si falla, se avisa por stderr y no invalida la medición |
| `src/core/config.rs` | Constante `HISTORIAL_LIMITE_BYTES` |
| `src/gui.rs` | Vista **HISTORIAL** (§7) |
| `Cargo.toml` | `chrono` sin features por defecto, solo `clock` (hora local en la GUI) |
| `src/install.rs` | Paso 5/5: unidades systemd y retiro de `koala.service` viejo |
| `scripts/install.sh` | Solo actualiza el resumen de pasos |
| `README.md` | Sección "Historial" |

## 7) Vista HISTORIAL en la GUI

- Selector arriba: `[VEREDICTO | HISTORIAL]`. La vista de veredicto no cambia.
- **Filtro por empresa:** ComboBox `TODAS / GOOGLE / CLOUDFLARE / ... / INSTAGRAM`, con la unión de los nombres que aparecen en el historial (sobrevive a cambios en `ips.json`).
- Con `TODAS`: `HORA · ESTADO · MEDIA · PÉRDIDA`.
- Con una empresa: `HORA · PING · JITTER · PÉRDIDA · ESTADO`. Si esa medición no incluía a la empresa, la fila muestra `—`.
- Las filas más recientes van primero.
- Se carga el historial completo al abrir la vista. La tabla usa `ScrollArea::show_rows`, que solo dibuja las filas visibles, así que no hace falta limitar la cantidad.
- Botón **ACTUALIZAR** para releer el archivo. También se relee después de una medición hecha desde la GUI.
- Hora local con `chrono`: `14:32`, o `05/11 14:32` si no es de hoy.
- Misma paleta existente (`COL_OK / COL_WARN / COL_BAD / COL_OFF`).

## 8) Casos borde

- **Primera medición:** el archivo y el directorio se crean si no existen.
- **Apagón durante un append o una compactación:** ver §4.
- **Mediciones concurrentes:** el lock de `medir` las evita; la que llega segunda devuelve el caché y no agrega nada.
- **Cambio de nombres en `ips.json`:** el filtro usa la unión histórica; un nombre nuevo es una serie nueva.
- **SIN RED:** el registro `Offline` se guarda igual, con latencias vacías (sirve como línea de tiempo de cortes).
- **Privacidad:** todo queda en el disco local.
- **Tráfico de red:** cada medición son 28 destinos × 4 pings, 112 paquetes ICMP. La del inicio agrega una por login.

## 9) Fases de implementación

1. **Fase 1:** `histo.rs`, configuración, append en `medir`, systemd y vista de tabla con filtro.
2. **Fase 2 (opcional):** gráfico de `media_ms`, dibujado con `egui::Painter` para evitar una dependencia nueva.
3. **Fase 3 (opcional):** exportar a CSV lo filtrado.

## 10) Criterios de aceptación

1. `cargo test` pasa, con los tests nuevos de `histo.rs`.
2. Tras `koala instalar`, `koala-medir.timer` queda habilitado y no se mide nada en ese momento.
3. Un minuto después de iniciar sesión hay un registro nuevo con esa hora, y no aparecen más sin pedirlos.
4. En la GUI, elegir `FACEBOOK` muestra la serie de esa empresa, y `TODAS` muestra la global.
5. El archivo persiste entre reinicios.
6. Repetir `koala instalar` no modifica ningún archivo ni recarga nada.

## 11) Qué dispara un registro

| Disparador | ¿Registra? |
|------------|-----------|
| `koala-medir.timer`, una vez, 60 s tras el login | Sí |
| Clic izquierdo en el widget (`koala medir`) | Sí |
| Abrir `koala gui` o tocar "Re-medir" | Sí |
| Refresco automático, si el usuario lo activa | Sí |
| Medición bloqueada por el lock | No (devuelve el caché) |
| `koala barra` (pinta el widget desde el caché) | No |

El widget arranca cada sesión con un dato fresco. Después, ese dato envejece hasta la próxima medición manual, como hasta ahora.
