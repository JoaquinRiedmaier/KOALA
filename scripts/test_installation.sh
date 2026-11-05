#!/bin/bash

# Script de prueba para KOALA
# Verifica que la instalación funciona correctamente y no hay duplicación en waybar

set -e

PROJECT_DIR="/home/johnd/Música/SOFTWARE/KOALA"
CARGO_BIN="$HOME/.cargo/bin/koala"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo_color() {
    case $1 in
        "error") echo -e "${RED}[ERROR]${NC} $2" ;;
        "success") echo -e "${GREEN}[SUCCESS]${NC} $2" ;;
        "warning") echo -e "${YELLOW}[WARNING]${NC} $2" ;;
        *) echo -e "[INFO] $2" ;;
    esac
}

# Función para verificar el estado de la instalación
verify_installation() {
    echo "=== Verificación de Instalación ==="
    
    # 1. Verificar si el binario existe
    if [ ! -f "$CARGO_BIN" ]; then
        echo_color "error" "Binario de KOALA no encontrado en $CARGO_BIN"
        return 1
    fi
    echo_color "success" "Binario de KOALA encontrado en $CARGO_BIN"
    
    # 2. Verificar que koala --version funciona
    if $CARGO_BIN --version 2>/dev/null; then
        echo_color "success" "Comando koala funciona"
    else
        echo_color "error" "Comando koala no funciona"
        return 1
    fi
    
    # 3. Verificar archivos de configuración
    local config_dir="$HOME/.config/waybar"
    local module_file="$config_dir/modules/koala.jsonc"
    local css_file="$config_dir/user-style.css"
    local config_file="$config_dir/config.jsonc"
    
    if [ -f "$module_file" ]; then
        echo_color "success" "Archivo de módulo waybar encontrado: $module_file"
    else
        echo_color "warning" "Archivo de módulo waybar no encontrado: $module_file"
    fi
    
    if [ -f "$css_file" ]; then
        echo_color "success" "Archivo de estilos CSS encontrado: $css_file"
        
        # Verificar marcadores únicos en CSS
        local markers_ini=$(grep -o "/* >>> koala (auto) >>> */" "$css_file" | wc -l)
        local markers_fin=$(grep -o "/* <<< koala <<< */" "$css_file" | wc -l)
        
        if [ "$markers_ini" -eq 1 ] && [ "$markers_fin" -eq 1 ]; then
            echo_color "success" "Marcadores CSS únicos (sin duplicación)"
        else
            echo_color "error" "Posible duplicación de marcadores CSS (ini: $markers_ini, fin: $markers_fin)"
        fi
    else
        echo_color "warning" "Archivo de estilos CSS no encontrado: $css_file"
    fi
    
    if [ -f "$config_file" ]; then
        echo_color "success" "Archivo de configuración de waybar encontrado: $config_file"
        
        # Contar ocurrencias de "custom/koala" para verificar duplicación
        local custom_koala_count=$(grep -o '"custom/koala"' "$config_file" | wc -l)
        
        if [ "$custom_koala_count" -eq 1 ]; then
            echo_color "success" "Exactamente un módulo custom/koala en configuración (sin duplicación)"
        elif [ "$custom_koala_count" -gt 1 ]; then
            echo_color "error" "DUPLICACIÓN DETECTADA: $custom_koala_count módulos custom/koala"
        else
            echo_color "warning" "No se encontró ningún módulo custom/koala"
        fi
    else
        echo_color "warning" "Archivo de configuración de waybar no encontrado: $config_file"
    fi
    
    return 0
}

# Función para probar funcionalidad
run_functionality_tests() {
    echo "=== Pruebas de Funcionalidad ==="
    
    # 1. Probar comando barra (JSON de widget)
    echo "Probando comando 'koala barra'..."
    if $CARGO_BIN barra > /tmp/koala_widget.json 2>/dev/null; then
        echo_color "success" "Comando 'koala barra' exitoso"
        
        # Verificar que el JSON es válido
        if python3 -m json.tool /tmp/koala_widget.json > /dev/null 2>&1; then
            echo_color "success" "JSON de widget válido"
        else
            echo_color "warning" "JSON de widget puede ser inválido"
        fi
    else
        echo_color "error" "Comando 'koala barra' falló"
    fi
    
    # 2. Probar comando medir (métrica)
    echo "Probando comando 'koala medir' (puede tardar)..."
    timeout 30 $CARGO_BIN medir > /tmp/koala_measure.log 2>&1
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo_color "success" "Comando 'koala medir' exitoso"
        
        # Verificar que se crearon archivos de caché
        if [ -f "$HOME/.cache/koala/" ]; then
            echo_color "success" "Cache de KOALA creado"
        fi
    else
        echo_color "error" "Comando 'koala medir' falló con código $exit_code"
        echo "Ver logs: cat /tmp/koala_measure.log"
    fi
}

# Función para verificar integración con waybar
verify_waybar_integration() {
    echo "=== Verificación de Integración con Waybar ==="
    
    local config_file="$HOME/.config/waybar/config.jsonc"
    local module_file="$HOME/.config/waybar/modules/koala.jsonc"
    
    if [ ! -f "$config_file" ]; then
        echo_color "error" "Archivo de configuración de waybar no encontrado"
        return
    fi
    
    # Verificar ejecución del comando del módulo
    echo "Verificando ejecución del módulo waybar..."
    
    # Extraer el comando del módulo
    local module_exec=$(grep -A5 -B5 '"custom/koala"' "$config_file" | grep "exec" | head -1)
    if [ -n "$module_exec" ]; then
        echo "Ejecución del módulo: $module_exec"
        
        # Verificar que el comando existe
        local cmd_path=$(echo "$module_exec" | sed 's/.*"\(.*\)".*/\1/')
        if [ -f "$cmd_path" ]; then
            echo_color "success" "Comando del módulo KOALA encontrado en: $cmd_path"
        else
            echo_color "error" "Comando del módulo KOALA no encontrado en: $cmd_path"
        fi
    else
        echo_color "error" "No se pudo encontrar la definición del módulo custom/koala"
    fi
    
    # Verificar integración con HyDE (regla de ventana flotante)
    local hyprland_file="$HOME/.config/hypr/hyprland.lua"
    if [ -f "$hyprland_file" ]; then
        if grep -q "hl.window_rule" "$hyprland_file" && grep -q "kOala.*" "$hyprland_file"; then
            echo_color "success" "Regla de ventana flotante de HyDE encontrada"
        else
            echo_color "warning" "Regla de ventana flotante de HyDE no encontrada"
        fi
    else
        echo_color "warning" "Archivo de Hyprland no encontrado: $hyprland_file"
    fi
}

# Función principal
main() {
    echo "=== Prueba de Instalación de KOALA ==="
    echo ""
    
    # Ejecutar todas las pruebas
    verify_installation
    echo ""
    run_functionality_tests
    echo ""
    verify_waybar_integration
    echo ""
    
    echo "=== Resumen ==="
    echo_color "success" "¡Pruebas completadas!"
    echo ""
    echo "Si encuentras algún error, revisa los logs de salida arriba."
    echo "Para obtener ayuda adicional, ejecuta:"
    echo "  ./scripts/install.sh --debug"
}

# Ejecutar función principal
main "$@"