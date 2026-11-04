# KOALA - Monitor de Conectividad de Internet

## Overview

KOALA es una aplicación de terminal que analiza la conectividad de internet ejecutando ping a múltiples proveedores importantes (v4 e v6). Incluye un widget para waybar (panel de HyDE) y una interfaz gráfica de usuario.

## Características

- **Medición de ping** a múltiples proveedores (Google, Cloudflare, Verizon, etc.)
- **Widget para waybar** con indicadores visuales de estado
- **Interfaz gráfica** para monitoreo en tiempo real
- **Integración con HyDE** (Hyprland Desktop Environment)
- **Configuración idempotente** - se puede ejecutar `koala instalar` múltiples veces sin crear duplicados

## Installation

### 1. Instalación con Cargo (Recommeded)

```bash
# Clone el repositorio (si aún no lo tienes)
git clone https://github.com/tuusuario/koala.git
cd koala

# Instalar usando cargo (con cargo instalado)
cargo install --path .

# Esto creará el binario en ~/.cargo/bin/koala
# El comando 'koala' estará disponible en tu PATH
```

### 2. Script de Instalación Automático

Para una instalación completa con configuración automática:

```bash
# Hacer el script ejecutable
chmod +x scripts/install.sh

# Ejecutar la instalación
./scripts/install.sh
```

Este script hará:
- Instalar KOALA usando cargo
- Configurar inicio automático (systemd o autostart)
- Integrar con waybar usando `koala instalar`
- Reiniciar waybar si está corriendo

### 3. Ejecución Directamente desde Fuente

```bash
# Para desarrollo/testing
cd koala
cargo run
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
- Reinicia waybar si está corriendo

## Configuration

### Archivo de Configuración de IPs (`~/.config/koala/ips.json`)

Contiene las listas de IPs de proveedores a probar. Se crea automáticamente con un valor predeterminado si no existe.

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

### Problema 2: KOALA no se ejecuta automáticamente

**Solución**:
```bash
# Verificar estado del servicio
systemctl --user status koala.service
# Iniciar si no está corriendo
systemctl --user start koala.service
# Habilitar inicio automático
systemctl --user enable koala.service
```

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

### Configuración Inicial

1. Ejecutar la instalación una vez:
   ```bash
   koala instalar
   ```

2. Reiniciar waybar:
   ```bash
   killall waybar && waybar
   ```

3. Verificar el widget y hacer clic derecho para abrir la GUI si es necesario

## Actualización

Para actualizar KOALA:

```bash
cd /home/johnd/Música/SOFTWARE/KOALA
cargo install --path . --force

# Si usaste el script de instalación, solo ejecútalo nuevamente
./scripts/install.sh
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