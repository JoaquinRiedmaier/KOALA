# KOALA — Historial de Mediciones de Ping (Plan de Implementación)

> Este es el contenido final de `/home/johnd/Música/SOFTWARE/KOALA/IMPLEMENTATION_SUMMARY.md`:
> el documento contendrá ÚNICAMENTE este plan de implementación (sin notas de pendientes ni
> decisiones internas). Se escribirá al pasar al agente de build.

---

# KOALA — Historial de Mediciones de Ping

## 1) Objetivo

Al encender la computadora, KOALA debe comenzar a **registrar los valores de ping en un archivo persistente**. La interfaz gráfica (`koala gui`) mostrará un **historial** de esas mediciones, con **filtro por empresa** (proveedor: Google, Cloudflare, Facebook, Instagram, etc.).

## 2) Alcance funcional

| # | Requisito | Cómo se cumple |
|---|-----------|----------------|
| 1 | Al encender la PC se registra el primer valor | Unidad systemd `koala.service` (oneshot `koala medir`) habilitada en el arranque del usuario |
| 2 | Los valores se guardan en un archivo de historial | JSONL en `~/.local/state/koala/history.jsonl`; cada medición agrega una línea |
| 3 | Historial continuo | Timer systemd `koala-medir.timer` (cada 5 min desde 90 s tras el boot) |
| 4 | La GUI muestra el historial | Vista nueva **HISTORIAL** en `koala gui` (tabla con scroll) |
| 5 | Filtrar por empresa | ComboBox con la unión de nombres presentes en el historial (`TODAS` por defecto) |

## 3) Antecedentes del código actual

- Cada medición produce un `EstadoGlobal` (`src/core/verdict.rs`) con `ts`, `estado`, `media_ms`, `peor_perdida` y el detalle por servidor. Hoy se **pisa siempre** el mismo archivo (`~/.cache/koala/state.json`): no queda historia.
- `scripts/install.sh` ya crea `koala.service` (systemd --user) con `ExecStart=koala barra` y `Restart=always`. Como proceso residente no hace nada útil (envía el JSON del widget al journal). Ese arranque ya existe pero desperdiciado: es el punto de enganche para disparar `medir` al encender.
- El lock de medición (`cache::tomar_lock`) serializa las mediciones: protege appends del historial.

## 4) Diseño del archivo de historial

### Ruta

- `$XDG_STATE_HOME/koala/history.jsonl` — defecto `~/.local/state/koala/history.jsonl`.
- Override para desarrollo/tests: variable de entorno `KOALA_HISTORIAL`.
- Nota: se usa `~/.local/state` (estado persistente) y **no** `~/.cache`: el cacheo lo borra a voluntad.

### Formato: JSON Lines (append-only)

Una medición por línea, JSON del `RegistroHistorial`. Nunca se reescribe el archivo entero por cada medición (al contrario de `state.json`).

### Registro slim (proyección, no `EstadoGlobal` completo)

El `EstadoGlobal` incluye las 30+ IPs individuales: pesado e inútil para un historial filtrable. Se guarda la proyección:

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct EmpresaHistorial {
    pub nombre: String,
    pub media: Option<f32>,
    pub jitter: Option<f32>,
    pub peor_perdida: u8,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RegistroHistorial {
    pub ts: u64,
    pub estado: Estado,
    pub media_ms: Option<f32>,
    pub peor_perdida: u8,
    pub empresas: Vec<EmpresaHistorial>,
}

impl From<&EstadoGlobal> for RegistroHistorial { /* descarta ips[] individuales */ }
```

~400–500 bytes por línea. Con medición cada 5 min: ~2 MB en 17 días.

### Retención

- `HISTORIAL_MAX_REGISTROS = 5000` líneas (~17 días a 5 min).
- Al superar `HISTORIAL_LIMITE_BYTES = 4 MiB`, **compaction**: reescribir conservando la cola del archivo (~75 % final, recortado en frontera de línea). El chequeo es solo `fs::metadata` (barato, por append).

### Lectura tolerante

Líneas corruptas (apagado durante un append, escritura parcial): `serde_json::from_str` falla → la línea **se ignora** sin afectar el resto del historial.

## 5) Arranque automático al encender la computadora

Lo agrega `koala instalar` (`src/install.rs`, paso 5/5, idempotente) con systemd --user:

| unidad | contenido |
|--------|-----------|
| `~/.config/systemd/user/koala.service` | `Type=oneshot` · `ExecStart=%h/.cargo/bin/koala medir` · `WantedBy=default.target` → **1 medición en cada boot** (reemplaza el `barra` activo actual) |
| `~/.config/systemd/user/koala-medir.timer` | `OnBootSec=90s` · `OnUnitActiveSec=300s` · `Unit=koala.service` → historial continuo |

Acciones del instalador: escribir ambas unidades → `systemctl --user daemon-reload` → `systemctl --user enable --now koala-medir.timer`.

- **Fallback sin systemctl**: `~/.config/autostart/koala-medir.desktop` (`Exec=koala medir`) → 1 registro por boot (historial más ralo).
- **Nota:** sin `linger`, el servicio usuario arranca en el primer login, que es suficiente para el historial.
- **Compatible con el script viejo:** `scripts/install.sh` escribe el mismo `koala.service`; se actualiza a `medir` oneshot para que ambos caminos coincidan.

## 6) Cambios por archivo

| archivo | cambio |
|---------|--------|
| `src/core/histo.rs` (**NUEVO**, `pub mod histo;` en `core/mod.rs`) | `agregar(&EstadoGlobal)` (proyecta + append), `cargar_ultimas(n)`, compaction, ruta con `$XDG_STATE_HOME`/`$KOALA_HISTORIAL`. Tests: crear+append en tmp, trim por MAX, lectura ignora corruptas, proyección `From<&EstadoGlobal>` |
| `src/app.rs` | En `medir()`, tras `cache::guardar()` → `histo::agregar(&estado)`. Solo medición fresca: el lock evita writes concurrentes y el retorno del caché no se duplica |
| `src/core/config.rs` | Constantes `HISTORIAL_MAX_REGISTROS`, `HISTORIAL_LIMITE_BYTES` |
| `src/gui.rs` | Vista **HISTORIAL** (ver §7) |
| `Cargo.toml` | `chrono = "0.4"` — única dependencia nueva (hora local en la GUI) |
| `src/install.rs` | Paso 5/5: unidades systemd + fallback autostart |
| `scripts/install.sh` | `koala.service` pasa de `barra` residente a `medir` oneshot (coincide con install.rs) |
| `README.md` | Sección "Historial": formato, ruta, retención, systemd |

## 7) Vista HISTORIAL en la GUI

- Toggle arriba: `[VEREDICTO | HISTORIAL]` (mismo panel central; el veredicto actual no cambia).
- **Filtro por empresa:** ComboBox `EMPRESA: TODAS / GOOGLE / CLOUDFARE / ... / FACEBOOK / INSTAGRAM`, con la **unión de los nombres que aparecen en el historial** (sobrevive cambios/renombres en `ips.json`).
- Columnas con `TODAS`: `HORA · ESTADO (color) · MEDIA ms · PEOR PÉRDIDA %`.
- Columnas con `EMPRESA = X`: `HORA · ms(X) · JITTER(X) · PÉRDIDA(X) · ESTADO(X)` — cada fila corresponde al entry `empresas[]` del registro con `nombre == X`; fila «—» si no estaba ese proveedor.
- Carga: `histo::cargar_ultimas(1000)` la primera vez que se abre la vista, cacheada en el struct de la app; botón **ACTUALIZAR** para refetch.
- Hora local con `chrono`: `%H:%M`; con día si no es hoy (`05/11 14:32`).
- Misma paleta existente (`COL_OK / COL_WARN / COL_BAD / COL_OFF`).

## 8) Casos borde cubiertos

- **Primera medición:** archivo/directorio no existen → se crean.
- **Append interrumpido** (apagón): línea incompleta ignorada al leer.
- **Mediciones concurrentes:** imposibles por el lock de `medir`.
- **Rotación de nombres** en `ips.json`: el filtro usa la unión histórica de nombres; nada se descarta.
- **SIN RED:** el registro `Offline` igual se guarda con latencias «—» (timeline de cortes de conexión).
- **Privacidad/datos:** todo queda local; no hay tráfico de red extra (no packs, "absolutely nothing" no: solo `ping` como ya lo hace).

## 9) Fases de implementación

1. **Fase 1 (core + GUI tabla):** `histo.rs`, config, `app.rs` append, systemd, vista tabla con filtro — todo lo funcional.
2. **Fase 2 (opcional):** sparkline/gráfico de línea de `media_ms` con `egui_plot` (dependencia extra, a evaluar).
3. **Fase 3 (opcional):** exportar CSV del rango filtrado.

## 10) Criterios de aceptación

1. `cargo test` OK (tests nuevos de `histo.rs`).
2. Tras `koala instalar`: `systemctl --user list-timers` muestra `koala-medir.timer`; cada medición suma 1 línea (`wc -l`).
3. Al encender: archivo con un registro cuyo `ts` corresponde al arranque.
4. GUI: elegir `FACEBOOK` muestra la serie de `www.facebook.com`; `TODAS` muestra el global.
5. `~/.local/state/koala/history.jsonl` persiste entre boots.

## 11) Resumen de los disparadores de medición (pulse)

| disparo | registrar? |
|---------|-----------|
| `koala.service` systemd en boot (oneshot) | sí (bootstrap) |
| `koala-medir.timer` cada 5 min | sí (ritmo continuo) |
| Clic izquierdo en widget = `koala medir` | sí |
| Refresco automático activable | sí (es `koala medir` igual) |
| `barra` (pintar widget del caché) | **no** (no mide) |

Thought · 2.2s
Corrijo una frase confusa que quedó en §8:
← Edit~/.opencode/plan/koala-historial.md
- **Append interrumpido** (apagón): línea incompleta ignorada al leer.
- **Mediciones concurrentes:** imposibles por el lock de `medir`.
- **Rotación de nombres** en `ips.json`: el filtro usa la unión histórica de nombres; nada se descarta.
- **SIN RED:** el registro `Offline` igual se guarda con latencias «—» (timeline de cortes de conexión).
- **Privacidad/datos:** todo queda local; no hay tráfico de red extra (no packs, "absolutely nothing" no: solo `ping` como ya lo hace).
- **Privacidad/datos:** todo queda local; no hay tráfico de extra de red, solo el `ping` que ya se ejecuta hoy.

## 9) Fases de implementación

1. **Fase 1 (core + GUI tabla):** `histo.rs`, config, `app.rs` append, systemd, vista tabla con filtro — todo lo funcional.
← Edit~/.opencode/plan/koala-historial.md
- **Append interrumpido** (apagón): línea incompleta ignorada al leer.
- **Mediciones concurrentes:** imposibles por el lock de `medir`.
- **Rotación de nombres** en `ips.json`: el filtro usa la unión histórica de nombres; nada se descarta.
- **SIN RED:** el registro `Offline` igual se guarda con latencias «—» (timeline de cortes de conexión).
- **Privacidad/datos:** todo queda local; no hay tráfico de extra de red, solo el `ping` que ya se ejecuta hoy.
- **Privacidad/datos:** todo queda local; no hay tráfico extra de red, solo el `ping` que ya se ejecuta hoy.

## 9) Fases de implementación

1. **Fase 1 (core + GUI tabla):** `histo.rs`, config, `app.rs` append, systemd, vista tabla con filtro — todo lo funcional.
