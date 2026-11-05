# KOALA - Monitor de Conectividad de Internet

## Overview

KOALA es una aplicación de terminal que analiza la conectividad de internet ejecutando ping a múltiples proveedores importantes (v4 e v6). Incluye un widget para waybar (panel de HyDE) y una interfaz gráfica de usuario.

## Características

- **Medición de ping** a múltiples proveedores (Google, Cloudflare, Verizon, etc.)
- **Ping a dominios** (www.facebook.com, www.instagram.com, ...): valida el DNS y la ruta real, integrado en el widget y en la GUI
- **Widget para waybar** con indicadores visuales de estado
- **Interfaz gráfica** para monitoreo en tiempo real
- **Historial de mediciones** con filtro por empresa: una medición al iniciar sesión y las que pidas después
- **Integración con HyDE** (Hyprland Desktop Environment)
- **Configuración idempotente** - se puede ejecutar `koala instalar` múltiples veces sin crear duplicados

## Instalación

### Requisitos

- Linux con systemd
- [Rust y Cargo](https://rustup.rs) 1.85 o superior
- `git`
- `ping` (paquete `iputils`)
- Waybar con HyDE, para el widget. Sin HyDE, `koala medir` y `koala gui` funcionan igual

### Pasos

1. **Clonar el repositorio:**

   ```bash
   git clone https://github.com/JoaquinRiedmaier/KOALA.git
   ```

   ```bash
   cd KOALA
   ```

2. **Instalar el programa.** Crea el binario en `~/.cargo/bin/koala`:

   ```bash
   cargo install --path .
   ```

   Si después la terminal no encuentra `koala`, agregá `~/.cargo/bin` a tu `PATH`.

3. **Integrarlo con tu escritorio:**

   ```bash
   koala instalar
   ```

   Este comando se puede repetir sin riesgo: si no hay nada que cambiar, no toca ningún archivo. La primera vez:
   - Copia la lista de servidores a `~/.config/koala/ips.json`.
   - Agrega el widget a waybar, junto a `custom/caffeine`, con sus colores.
   - Agrega la regla de ventana flotante en `~/.config/hypr/hyprland.lua`.
   - Activa la medición al iniciar sesión.
   - Recarga waybar.

4. **Comprobar que funciona:**

   ```bash
   koala medir && koala barra
   ```

   Tiene que mostrar el resultado y una línea JSON. El widget aparece en waybar: con clic izquierdo mide y con clic derecho abre la ventana.

### Conservar el widget en HyDE

HyDE regenera `~/.config/waybar/config.jsonc`, y al hacerlo el widget desaparece. Para que vuelva solo, agregá esto al final de `~/.config/hypr/hyprland.lua`:

```lua
hl.on("hyprland.start", function()
    hl.exec_cmd("sh -c 'sleep 8; $HOME/.cargo/bin/koala instalar'")
end)
```

### Instalación con el script

`scripts/install.sh` hace los pasos 2 a 4 de una vez, y comprueba antes que Cargo esté instalado. Se ejecuta desde la carpeta del proyecto:

```bash
./scripts/install.sh
```

### Ejecutar sin instalar

Para desarrollo, se indica el subcomando después de `--`:

```bash
cargo run -- gui
```

## Comandos

KOALA soporta los siguientes subcomandos:

### `koala medir`
- Ejecuta mediciones de ping a todos los proveedores
- Guarda el caché para uso por el widget y la GUI
- Muestra resultados detallados

### `koala barra`
- Imprime el JSON del widget para waybar
- El formato del JSON sigue el estándar de return-type json de waybar
- Se ejecuta cada 5 segundos por defecto

### `koala gui`
- Abre la ventana gráfica de monitoreo
- Muestra el estado de conectividad en tiempo real

### `koala instalar`
- Integra KOALA con waybar (HyDE)
- Crea/actualiza archivos de configuración
- Añade regla de ventana flotante para Hyprland
- Activa la medición al iniciar sesión
- Recarga waybar si algo cambió

## Configuration

### Archivo de Configuración de IPs (`~/.config/koala/ips.json`)

Contiene las listas de IPs de proveedores a probar. Se crea automáticamente con un valor predeterminado si no existe.

### Ping a dominios (campo `dominios` en ips.json)

Además de IPs directas, cada objetivo puede incluir dominios, que se miden igual que cualquier servidor:

```json
{
    "nombre": "FACEBOOK",
    "ip4": [],
    "ip6": [],
    "dominios": ["www.facebook.com"]
}
```

- No se fuerza familia de IP (tag `-4`/`-6`): el sistema resuelve el nombre y elige la ruta disponible. En una red sin IPv6 usa la v4; de paso valida el DNS real.
- Los dominios cuentan para el estado global (OK/LENTO/INESTABLE) igual que las IPs, con los mismos umbrales y medianas.
- Si el dominio no resuelve (DNS caído), su fila queda como **SIN RESPUESTA**.
- El `ips.json` embebido en el binario ya trae **FACEBOOK** (www.facebook.com) e **INSTAGRAM** (www.instagram.com) como ejemplo.

**Nota:** si ya tenés `~/.config/koala/ips.json`, `koala instalar` no lo toca (respeta tu configuración). Para tener los dominios: copiá el bloque de ejemplo a tu archivo, o borrá el archivo y ejecutá `koala instalar` para regenerarlo con el nuevo default.

### Configuración de Waybar (`~/.config/waybar/modules/koala.jsonc`)

Definición del módulo personalizado para waybar:

```json
{
    "custom/koala": {
        "format": "{text}",
        "exec": "[absolute path to koala binary] barra",
        "return-type": "json",
        "interval": 5,
        "on-click": "[absolute path to koala binary] medir",
        "on-click-right": "[absolute path to koala binary] gui",
        "tooltip": true
    }
}
```

### Archivo de Estilos (`~/.config/waybar/user-style.css`)

Contiene los estilos CSS para el widget KOALA con indicadores de color:
- `idle` - color gris
- `ok` - color verde
- `slow` - color amarillo
- `bad` - color naranja
- `offline` - color rojo
- `stale` - color atenuado

### Configuración de Waybar (`~/.config/waybar/config.jsonc`)

Añade `custom/koala` a la barra. KOALA se añade al lado izquierdo junto a otros widgets.

## Historial

`koala instalar` activa `koala-medir.timer`, un timer de systemd de usuario que hace **una sola medición** 60 segundos después de iniciar sesión. Después solo se mide a pedido: clic en el widget, abrir `koala gui` o "Re-medir". Todas esas mediciones quedan en el historial.

Para verlo, abrí `koala gui` y elegí **HISTORIAL**. Muestra un día por vez, y se puede filtrar por empresa y moverse entre días con `<` y `>`:

- **Gráfico:** las horas van en el eje horizontal y la latencia en el vertical. Cada hora muestra el promedio de sus mediciones, en verde hasta 50 ms, amarillo hasta 100 ms y rojo por encima. Una hora sin mediciones queda vacía. Al pasar el mouse se ve el detalle.
- **Cortes:** una hora sin ninguna respuesta se marca en rojo tenue en toda la columna, y una hora con algún fallo lleva una franja roja al pie.
- **Tabla:** debajo, las mediciones de ese día.

- **Archivo:** `~/.local/state/koala/history.jsonl`, una medición JSON por línea. Se crea en la primera medición, se conserva entre reinicios y se cambia con `$KOALA_HISTORIAL`.
- **Retención:** al superar 8 MiB (unas 10.000 mediciones) se descartan las más viejas.

```bash
# Quitar la medición al iniciar sesión (el historial se conserva)
systemctl --user disable --now koala-medir.timer
```

## Lógica del estado y del tiempo mostrado

Esta sección explica exactamente cómo se produce el texto del widget (`OK 13ms`, `LENTO 45ms`, `INESTABLE 60%`, `SIN RED`) y qué significa cada dato.

### Qué mide cada ping

Cada destino (IP o dominio) recibe 4 paquetes ICMP (`PINGS_POR_IP = 4`, intervalo de 0,3 s, timeout de 1 s por respuesta). Del resumen del comando `ping` se extraen, sin depender del idioma de la salida (se apoya en los números, no en las palabras):

- **avg** → latencia promedio de esos 4 paquetes. De la línea `rtt min/avg/max/mdev = 12.910/13.259/13.568/0.251 ms` se toma el `13.259`.
- **mdev** → variación de la latencia (jitter) entre esos 4 paquetes.
- **% de pérdida** → del resumen tipo `4 packets transmitted, 4 received, 0% packet loss`.
- Si ningún paquete volvió (100 % de pérdida), ese destino no aporta latencia.

### Estado de cada servidor (filas de la GUI y del tooltip)

| etiqueta | condición (se evalúa en este orden) |
|---|---|
| **SIN RESPUESTA** | ninguna de sus direcciones respondió |
| **INESTABLE** | su peor pérdida > 25 % |
| **LENTO** | su media > 50 ms, o su jitter > 10 ms |
| **OK** | dentro de todos los umbrales |

### Estado global (lo que muestra el widget)

Se evalúa de peor a mejor:

| estado | clase css | condición |
|---|---|---|
| **SIN RED** | `offline` | ningún servidor respondió |
| **INESTABLE** | `bad` | la peor pérdida global > 25 %, **o** 2 o más servidores con 100 % de pérdida |
| **LENTO** | `slow` | latencia global > 50 ms, **o** el jitter de cualquier servidor > 10 ms |
| **OK** | `ok` | todo dentro de los umbrales |

Las constantes están definidas en `src/core/verdict.rs`:

| constante | valor |
|---|---|
| `LIMITE_MS` | 50 ms |
| `LIMITE_JITTER` | 10 ms |
| `LIMITE_PERDIDA` | 25 % |
| `PROVEEDORES_CAIDOS_PARA_BAD` | 2 servidores caídos |

### ¿Qué tiempo muestra el widget y de qué servidor es?

El número de `OK 13ms` es la **latencia global**. No corresponde a un solo servidor fijo: es el valor típico entre todos los servidores que respondieron. Se calcula así:

1. Cada dirección que respondió aporta su **avg** (promedio de sus 4 pings).
2. Por servidor se reduce todo a una **mediana** de esos avg (la "media" del servidor).
3. El widget toma la **mediana de las medias** de los servidores alcanzables → ese número es el que se muestra.

Se usa **mediana** y no promedio a propósito: una IP muy lenta dentro de un proveedor, o un servidor muy lento dentro del total, no distorsionan el número representativo. Por ejemplo, con la métrica de arriba si 8 servidores rondan 30–40 ms y ULTRADNS VIRGINIA mide 130 ms, el widget sigue mostrando ~38 ms, aunque el tooltip/GUI marquen ese servidor como LENTO con su propio valor.

El detalle "a qué servidor corresponde cada latencia" está siempre visible en el **tooltip del widget** (una fila por servidor, con su media en ms) y en la columna **PING** de la GUI.

Casos especiales del número mostrado:
- En `INESTABLE` lo que se muestra es la **peor % de pérdida** (la latencia deja de ser confiable).
- En `SIN RED` no hay número.

### De dónde sale el dato que muestra el widget

El widget no mide en el hilo de la barra: pinta siempre la **última medición** guardada en la caché `~/.cache/koala/state.json`.

- **Clic izquierdo** sobre el widget → mide ahora mismo y refresca.
- Refresco automático: **apagado por defecto** (`core::config::AUTO_REFRESH = false`). Para activarlo sin recompilar: `KOALA_AUTO_REFRESH=1`; con esto re-mide cuando la caché supera los 300 s (`REFRESH_SECS`).
- Con más de 600 s sin medir (`STALE_SECS`) el widget entra en modo **stale**: texto atenuado y cursiva (clase css `stale`), con una nota en el tooltip invitando a re-medir.

## Solución de Problemas

### Problema 1: Duplicación en Waybar

**Síntoma**: Ejecutar `koala instalar` múltiples veces crea múltiples entradas `custom/koala` en waybar.

**Causa**: El antiguo sistema de detección de duplicados usaba una simple búsqueda de cadena `"custom/koala"` en el archivo de configuración, que podía falsear-positive si esa cadena aparecía en otro contexto.

**Solución**: Se mejoró la detección de duplicados para usar análisis JSON preciso. Ejecutar `koala instalar` una vez más para aplicar la solución.

**Pasos para solucionar**:
```bash
# Verificar si hay duplicados
koala barra
# Verificar la configuración de waybar
cat ~/.config/waybar/config.jsonc | grep -o '"custom/koala"' | wc -l

# Si hay duplicados, reiniciar waybar
killall waybar && waybar
```

### Problema 2: El widget no aparece en waybar

Waybar ejecuta `koala barra` cada 5 segundos, así que el widget no necesita un servicio propio.
Si desaparece (HyDE regenera `config.jsonc`), hay que volver a colocarlo:

**Solución**:
```bash
koala instalar
```

`koala instalar` se puede ejecutar en cada inicio de sesión: si no hay nada que cambiar, no escribe
ningún archivo ni recarga waybar.

### Problema 3: Errores en la instalación

**Comprobar el log de errores**:
```bash
# Si usaste el script de instalación
./scripts/install.sh
# Revisar cualquier mensaje de error en la salida

# Intentar instalación manual
cargo install --path .
koala instalar
```

### Problema 4: Widget de waybar no muestra estado

**Verificar**:
```bash
# Logs de KOALA (si está corriendo)
# El widget de waybar debería mostrar el JSON producido por `koala barra`

# Probar KOALA manualmente
koala barra
koala medir
```

## Uso Diario

### Monitorizar Conectividad

1. **Desde terminal**:
   ```bash
   koala medir
   ```

2. **Widget en waybar**: Se actualiza automáticamente cada 5 segundos

3. **Ventana gráfica**:
   ```bash
   koala gui
   ```

## Actualización

Desde la carpeta del proyecto:

```bash
git pull
```

```bash
cargo install --path . --force
```

```bash
koala instalar
```

## Apoyo

Si encuentras bugs o necesitas ayuda:

1. Revisar el archivo de análisis: `analysis_report.md`
2. Verificar los logs de installation: `./scripts/install.sh`
3. Revisar el README para pasos de solución de problemas

KOALA está bajo licencia GPL-3.0. ¡Siéntete libre de modificar y distribuir bajo los términos de la licencia!

## Créditos

- Proyecto original por Joaco200219
- Basado en conceptos de HyDE (Hyprland Desktop Environment)
- Iconografía inspirada en diseños de gatos (K.O.A.L.A. = Kitten Oriented Asynchronous Link Analyzer)