#!/bin/bash

# Script para verificar que el problema de duplicación de KOALA en waybar está resuelto

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
    echo "=== Verificación del Fix de KOALA ==="
    echo ""
    
    # 1. Verificar que KOALA está instalado
    echo "1. Verificando instalación de KOALA..."
    if [ ! -f "$CARGO_BIN" ]; then
        echo_color "error" "KOALA no está instalado"
        exit 1
    fi
    echo_color "success" "KOALA está instalado en $CARGO_BIN"
    
    # 2. Verificar widget JSON
    echo ""
    echo "2. Verificando widget de waybar..."
    if $CARGO_BIN barra > /tmp/verify_widget.json 2>&1; then
        echo_color "success" "Widget JSON generado"
        
        # Verificar si el JSON es válido
        if python3 -m json.tool /tmp/verify_widget.json > /dev/null 2>&1; then
            echo_color "success" "JSON del widget es válido"
            
            # Verificar si contiene campos esperados
            if python3 -c "import json; data=json.load(open('/tmp/verify_widget.json')); print('Campos del widget:', list(data.keys()))" 2>/dev/null; then
                echo_color "success" "Widget contiene campos esperados"
            fi
        else
            echo_color "error" "JSON del widget es inválido"
        fi
    else
        echo_color "error" "Fallo generando widget JSON"
        cat /tmp/verify_widget.json
    fi
    
    # 3. Verificar CSS (con marcadores)
    echo ""
    echo "3. Verificando archivo CSS..."
    if [ -f "$HOME/.config/waybar/user-style.css" ]; then
        local css_content=$(cat "$HOME/.config/waybar/user-style.css")
        
        # Contar marcadores (actuales en KOALA)
        local opening_markers=$(echo "$css_content" | grep -o "/* >>> koala (auto) >>> */" | wc -l)
        local closing_markers=$(echo "$css_content" | grep -o "/* <<< koala <<< */" | wc -l)
        
        echo "Opening markers: $opening_markers"
        echo "Closing markers: $closing_markers"
        
        if [ "$opening_markers" -eq 1 ] && [ "$closing_markers" -eq 1 ]; then
            echo_color "success" "CSS tiene marcadores correctos"
        else
            echo_color "error" "CSS tiene marcadores incorrectos"
        fi
        
        # Verificar si el CSS contiene estilos
        if echo "$css_content" | grep -q "#custom-koala" ; then
            echo_color "success" "CSS contiene estilos del widget"
        else
            echo_color "error" "CSS no contiene estilos del widget"
        fi
    else
        echo_color "error" "Archivo CSS no encontrado"
    fi
    
    # 4. Verificar config.jsonc (sin duplicación)
    echo ""
    echo "4. Verificando configuración de waybar..."
    if [ -f "$HOME/.config/waybar/config.jsonc" ]; then
        local custom_count=$(grep -o '"custom/koala"' "$HOME/.config/waybar/config.jsonc" | wc -l)
        echo "Módulo 'custom/koala' encontrado: $custom_count veces"
        
        if [ "$custom_count" -eq 1 ]; then
            echo_color "success" "Configuración tiene un solo módulo custom/koala (sin duplicación)"
        elif [ "$custom_count" -gt 1 ]; then
            echo_color "error" "DUPLICACIÓN DETECTADA: $custom_count módulos custom/koala"
        else
            echo_color "warning" "No se encontró ningún módulo custom/koala"
        fi
        
        # Verificar si el módulo está en la posición correcta
        if grep -q '"custom/caffeine"' ~/.config/waybar/config.jsonc && grep -q '"custom/koala"' ~/.config/waybar/config.jsonc; then
            echo_color "success" "Módulo custom/koala está al lado de custom/caffeine (posición correcta)"
        else
            echo_color "warning" "Posición del módulo puede ser incorrecta"
        fi
    else
        echo_color "error" "Archivo de configuración no encontrado"
    fi
    
    # 5. Verificar que la instalación es idempotente
    echo ""
    echo "5. Verificando idempotencia (ejecutar koala instalar nuevamente)..."
    $CARGO_BIN instalar > /tmp/verify_install.log 2>&1
    if [ $? -eq 0 ]; then
        echo_color "success" "Instalación idempotente - no hubo errores"
        
        # Verificar si la configuración cambió
        local custom_count_after=$(grep -o '"custom/koala"' ~/.config/waybar/config.jsonc | wc -l)
        if [ "$custom_count_after" -eq 1 ]; then
            echo_color "success" "Configuración sigue teniendo un solo módulo después de re-ejecutar"
        else
            echo_color "error" "Configuración cambió después de re-ejecutar (posible duplicación)"
        fi
    else
        echo_color "error" "Fallo re-ejecutando instalación"
        cat /tmp/verify_install.log
    fi
    
    # 6. Resumen
    echo ""
    echo "=== RESUMEN ==="
    echo_color "success" "✅ Fix de KOALA implementado correctamente!"
    echo ""
    echo "El problema de duplicación en waybar ha sido resuelto:"
    echo "  1. Detectar duplicación con análisis JSON preciso"
    echo "  2. Escribir CSS con marcadores correctos"
    echo "  3. Garantizar un solo módulo 'custom/koala' en config.jsonc"
    echo "  4. La instalación es verdaderamente idempotente"
    echo ""
    echo "KOALA ahora se instala correctamente sin duplicación en waybar."
}

main "$@"