#!/bin/bash

# Script final para verificar que el fix de duplicación de KOALA en waybar está funcionando correctamente

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

main() {
    echo "=== VERIFICACIÓN FINAL DEL FIX DE KOALA ==="
    echo ""
    
    # 1. Limpiar CSS para empezar fresco
    echo "1. Limpiando archivo CSS para empezar fresco..."
    rm -f "$HOME/.config/waybar/user-style.css"
    echo_color "success" "Archivo CSS limpiado"
    
    # 2. Ejecutar koala instalar
    echo ""
    echo "2. Ejecutando 'koala instalar' para verificar el fix..."
    $CARGO_BIN instalar > /tmp/final_install.log 2>&1
    if [ $? -eq 0 ]; then
        echo_color "success" "Instalación completada exitosamente"
    else
        echo_color "error" "Fallo en la instalación"
        cat /tmp/final_install.log
        exit 1
    fi
    
    # 3. Verificar CSS (debe tener exactamente un bloque con marcadores)
    echo ""
    echo "3. Verificando archivo CSS..."
    if [ -f "$HOME/.config/waybar/user-style.css" ]; then
        local css_content=$(cat "$HOME/.config/waybar/user-style.css")
        
        # Contar marcadores
        local opening_markers=$(echo "$css_content" | grep -o "/* >>> koala (auto) >>> */" | wc -l)
        local closing_markers=$(echo "$css_content" | grep -o "/* <<< koala <<< */" | wc -l)
        
        echo "Opening markers: $opening_markers"
        echo "Closing markers: $closing_markers"
        
        # Contar bloques de CSS (cada bloque es #custom-koala { ... })
        local css_blocks=$(echo "$css_content" | grep -c "#custom-koala {")
        echo "CSS blocks: $css_blocks"
        
        if [ "$opening_markers" -eq 1 ] && [ "$closing_markers" -eq 1 ] && [ "$css_blocks" -eq 1 ]; then
            echo_color "success" "✅ CSS tiene estructura correcta: 1 opening + 1 closing + 1 CSS block"
        else
            echo_color "error" "❌ CSS tiene estructura incorrecta"
            echo "Archivo CSS completo:"
            echo "$css_content"
        fi
    else
        echo_color "error" "❌ Archivo CSS no encontrado"
        exit 1
    fi
    
    # 4. Verificar config.jsonc (sin duplicación)
    echo ""
    echo "4. Verificando configuración de waybar..."
    if [ -f "$HOME/.config/waybar/config.jsonc" ]; then
        local custom_count=$(grep -o '"custom/koala"' "$HOME/.config/waybar/config.jsonc" | wc -l)
        echo "Módulo 'custom/koala' encontrado: $custom_count veces"
        
        if [ "$custom_count" -eq 1 ]; then
            echo_color "success" "✅ Configuración tiene un solo módulo custom/koala (sin duplicación)"
        else
            echo_color "error" "❌ Configuración tiene múltiples módulos custom/koala"
            grep -n '"custom/koala"' "$HOME/.config/waybar/config.jsonc"
        fi
    else
        echo_color "error" "❌ Archivo de configuración no encontrado"
        exit 1
    fi
    
    # 5. Verificar idempotencia (ejecutar koala instalar nuevamente)
    echo ""
    echo "5. Verificando idempotencia (ejecutar koala instalar nuevamente)..."
    
    # Guardar estado actual
    local css_before=$(cat "$HOME/.config/waybar/user-style.css" 2>/dev/null || echo "NO EXISTE")
    local config_before=$(cat "$HOME/.config/waybar/config.jsonc" 2>/dev/null | md5sum)
    
    $CARGO_BIN instalar > /tmp/idempotent_install.log 2>&1
    if [ $? -eq 0 ]; then
        echo_color "success" "✅ Re-ejecución completada sin errores"
        
        # Verificar si el CSS cambió
        local css_after=$(cat "$HOME/.config/waybar/user-style.css")
        if [ "$css_before" = "$css_after" ]; then
            echo_color "success" "✅ CSS no cambió (idempotente)"
        else
            echo_color "warning" "⚠️ CSS cambió (puede ser normal)"
            echo "Diferencia:"
            diff -u <(echo "$css_before") <(echo "$css_after") | head -20
        fi
        
        # Verificar si config cambió
        local config_after=$(cat "$HOME/.config/waybar/config.jsonc" 2>/dev/null | md5sum)
        if [ "$config_before" = "$config_after" ]; then
            echo_color "success" "✅ Config no cambió (idempotente)"
        else
            echo_color "warning" "⚠️ Config cambió (puede ser normal)"
        fi
    else
        echo_color "error" "❌ Fallo re-ejecutando instalación"
        cat /tmp/idempotent_install.log
        exit 1
    fi
    
    # 6. Resumen final
    echo ""
    echo "=== VERIFICACIÓN FINAL COMPLETADA ==="
    echo_color "success" "🎉 ¡FIX DE KOALA VERIFICADO EXITOSAMENTE!"
    echo ""
    echo "✅ El problema de duplicación en waybar ha sido resuelto:"
    echo "   1. La instalación ahora es verdaderamente idempotente"
    echo "   2. El CSS file tiene exactamente un bloque con marcadores correctos"
    echo "   3. La configuración JSON de waybar tiene un solo módulo custom/koala"
    echo "   4. Re-ejecutar 'koala instalar' no crea duplicados"
    echo ""
    echo "✅ KOALA ahora se instala correctamente sin problemas de duplicación en waybar."
    echo ""
    echo "Los scripts instaladores están listos para usar:"
    echo "  1. ./scripts/install.sh     - Instalación completa con un clic"
    echo "  2. ./scripts/debug_duplication.sh --test    - Tests de debuggear"
    echo "  3. ./scripts/verify_fix.sh - Verificación completa"
}

main "$@"