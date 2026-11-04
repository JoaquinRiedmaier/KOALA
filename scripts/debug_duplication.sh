#!/bin/bash

# Script de depuración para problemas de duplicación de KOALA en waybar

set -e

PROJECT_DIR="/home/johnd/Música/SOFTWARE/KOALA"
CARGO_BIN="$HOME/.cargo/bin/koala"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo_color() {
    case $1 in
        "error") echo -e "${RED}[ERROR]${NC} $2" ;;
        "success") echo -e "${GREEN}[SUCCESS]${NC} $2" ;;
        "warning") echo -e "${YELLOW}[WARNING]${NC} $2" ;;
        "info") echo -e "${BLUE}[INFO]${NC} $2" ;;
        *) echo -e "[DEBUG] $2" ;;
    esac
}

# Función para limpiar y reinstalar KOALA
reset_installation() {
    echo_color "warning" "Limpiando archivos de configuración de KOALA..."
    
    # Remover archivos de configuración
    rm -f "$HOME/.config/waybar/modules/koala.jsonc"
    rm -f "$HOME/.config/waybar/user-style.css"
    rm -f "$HOME/.config/waybar/config.jsonc"
    rm -f "$HOME/.config/waybar/*.koala-bak"
    rm -rf "$HOME/.config/koala/"
    
    # Remover regla de ventana flotante de HyDE si existe
    local hyprland_file="$HOME/.config/hypr/hyprland.lua"
    if [ -f "$hyprland_file" ]; then
        # Crear backup y remover marcadores
        cp "$hyprland_file" "$hyprland_file.bak.debug"
        sed -i '/\/\/ -- >>> koala (auto) >>>/,/\/\/ -- <<< koala <<</d' "$hyprland_file"
        echo_color "success" "Regla de ventana flotante de HyDE limpiada"
    fi
    
    echo_color "info" "Archivos de configuración limpiados"
}

# Función para verificar estado actual
check_current_state() {
    echo "=== Estado Actual de KOALA ==="
    
    # Verificar if KOALA está instalado
    if [ -f "$CARGO_BIN" ]; then
        echo_color "success" "KOALA instalado en: $CARGO_BIN"
        
        # Verificar versión
        echo "Versión: $($CARGO_BIN --version 2>/dev/null || echo 'No disponible')"
    else
        echo_color "error" "KOALA no está instalado"
        echo "Instale usando: cargo install --path $PROJECT_DIR"
    fi
    
    # Verificar estructura de configuración
    local config_dir="$HOME/.config/waybar"
    if [ -d "$config_dir" ]; then
        echo_color "success" "Directorio de configuración waybar existe: $config_dir"
        
        # Listar archivos
        find "$config_dir" -maxdepth 2 -type f -name "*koala*" | while read file; do
            local rel_path="${file#$config_dir/}"
            echo "  Archivo: $rel_path"
            
            # Contenido específico según archivo
            if [[ "$file" == *"modules/koala.jsonc" ]]; then
                echo "    Contenido:"
                cat "$file" | sed 's/^/      /'
            elif [[ "$file" == *"user-style.css" ]]; then
                echo "    Marcadores encontrados:"
                grep -o "/* >>> koala (auto) >>> */" "$file" | wc -l
                grep -o "/* <<< koala <<< */" "$file" | wc -l
            fi
        done
    else
        echo_color "warning" "Directorio de configuración waybar no existe"
    fi
    
    # Verificar archivo de configuración waybar
    local config_file="$HOME/.config/waybar/config.jsonc"
    if [ -f "$config_file" ]; then
        echo "=== Contenido de config.jsonc ==="
        
        # Contar módulos
        local custom_count=$(grep -o '"custom/koala"' "$config_file" | wc -l)
        echo "Módulo 'custom/koala' encontrado: $custom_count veces"
        
        if [ "$custom_count" -gt 1 ]; then
            echo_color "error" "¡DUPLICACIÓN DETECTADA!"
            echo "Posiciones:"
            grep -n '"custom/koala"' "$config_file"
        else
            echo_color "success" "Un solo módulo 'custom/koala'"
        fi
        
        # Mostrar contexto alrededor del módulo
        if [ "$custom_count" -eq 1 ]; then
            echo ""
            echo "Contexto del módulo:"
            grep -B5 -A5 '"custom/koala"' "$config_file"
        fi
    else
        echo_color "warning" "Archivo de configuración waybar no encontrado: $config_file"
    fi
}

# Función para ejecutar test de widget
run_widget_test() {
    echo "=== Test de Widget KOALA ==="
    
    if [ ! -f "$CARGO_BIN" ]; then
        echo_color "error" "KOALA no está instalado"
        return
    fi
    
    # Generar JSON del widget
    echo "Ejecutando: $CARGO_BIN barra"
    $CARGO_BIN barra > /tmp/koala_widget.json 2>&1
    
    if [ $? -eq 0 ]; then
        echo_color "success" "Widget JSON generado en /tmp/koala_widget.json"
        
        # Verificar si el JSON es válido
        if python3 -m json.tool /tmp/koala_widget.json > /dev/null 2>&1; then
            echo_color "success" "JSON del widget es válido"
            echo ""
            echo "JSON del widget (formateado):"
            python3 -m json.tool /tmp/koala_widget.json
        else
            echo_color "error" "JSON del widget es inválido"
            echo ""
            echo "JSON crudo:"
            cat /tmp/koala_widget.json
        fi
    else
        echo_color "error" "Error generando JSON del widget"
        echo "Ver logs: cat /tmp/koala_widget.json"
    fi
}

# Función para verificar problemas de configuración
check_config_problems() {
    echo "=== Verificación de Problemas de Configuración ==="
    
    # 1. Verificar configuración idempotente del módulo
    local config_file="$HOME/.config/waybar/config.jsonc"
    if [ -f "$config_file" ]; then
        echo "Verificando configuración idempotente..."
        
        # Utilizar la nueva función de detección precisa si está disponible
        if command -v python3 &> /dev/null; then
            # Intentar usar una versión mejorada de la función en Rust
            echo "Ejecutando detección de duplicación mejorada..."
            
            # Crear un script temporal para verificar usando la lógica mejorada
            cat > /tmp/check_duplication.py << 'EOF'
import json
import sys
import os

config_path = os.path.expanduser('~/.config/waybar/config.jsonc')

def config_contiene_custom_koala(contenido):
    try:
        config = json.loads(contenido)
        if 'bar' in config and 'modules' in config['bar']:
            modules = config['bar']['modules']
            if 'center' in modules:
                if isinstance(modules['center'], list):
                    return 'custom/koala' in modules['center']
            if 'left' in modules:
                if isinstance(modules['left'], list):
                    return 'custom/koala' in modules['left']
            if 'right' in modules:
                if isinstance(modules['right'], list):
                    return 'custom/koala' in modules['right']
    except json.JSONDecodeError:
        pass
    return '"custom/koala"' in contenido

with open(config_path, 'r') as f:
    contenido = f.read()
    
if config_contiene_custom_koala(contenido):
    print("DETECCIÓN PRECISA: Módulo 'custom/koala' encontrado")
    
    # Contar todas las ocurrencias
    import re
    count = len(re.findall(r'"custom/koala"', contenido))
    print(f"Total de ocurrencias: {count}")
    
    if count > 1:
        print("ALERTA: Posibles duplicados detectados")
    else:
        print("VERIFICACIÓN: Exactamente una ocurrencia")
else:
    print("DETECCIÓN PRECISA: Módulo 'custom/koala' NO encontrado")
EOF
            
            python3 /tmp/check_duplication.py
        fi
    fi
    
    # 2. Verificar consistencia del comando del módulo
    echo ""
    echo "Verificando consistencia del comando del módulo..."
    
    local module_file="$HOME/.config/waybar/modules/koala.jsonc"
    if [ -f "$module_file" ]; then
        # Extraer el comando exec
        local exec_cmd=$(grep -A2 '"exec"' "$module_file" | sed 's/.*"\(.*\)".*/\1/')
        if [ -n "$exec_cmd" ]; then
            echo "Comando exec del módulo: $exec_cmd"
            
            # Verificar si el binario existe
            local bin_path="${exec_cmd%% *}"  # Obtener la primera palabra (el binario)
            if [ -f "$bin_path" ]; then
                echo_color "success" "Binario encontrado: $bin_path"
            else
                echo_color "error" "Binario NO encontrado: $bin_path"
            fi
        fi
    fi
}

# Función para depurar instalación
run_debug_install() {
    echo "=== Depuración de Instalación ==="
    
    if [ ! -f "$CARGO_BIN" ]; then
        echo_color "error" "KOALA no está instalado, instalando..."
        cd "$PROJECT_DIR"
        cargo install --path . || {
            echo_color "error" "Fallo en la instalación"
            return
        }
    fi
    
    echo "Ejecutando: $CARGO_BIN instalar"
    $CARGO_BIN instalar
    
    if [ $? -eq 0 ]; then
        echo_color "success" "Instalación completada"
    else
        echo_color "error" "Fallo en la instalación"
    fi
}

# Función principal
main() {
    echo "=== Depuración de KOALA para Problemas de Duplicación ==="
    echo ""
    
    # Parsear argumentos
    local action="check"
    while [[ $# -gt 0 ]]; do
        case $1 in
            "--clean")
                reset_installation
                shift
                ;;
            "--install")
                action="install"
                shift
                ;;
            "--test")
                action="test"
                shift
                ;;
            "--help"|"-h")
                echo "Uso: $0 [OPCIÓN]"
                echo ""
                echo "Opciones:"
                echo "  --clean    Limpiar configuración y volver a instalar"
                echo "  --install  Ejecutar la instalación para depurar"
                echo "  --test     Ejecutar pruebas de widget y funcionalidad"
                echo "  (sin opción)  Verificar estado actual"
                exit 0
                ;;
            *)
                echo_color "warning" "Opción desconocida: $1"
                shift
                ;;
        esac
    done
    
    # Ejecutar acciones según lo especificado
    case $action in
        "clean")
            reset_installation
            check_current_state
            ;;
        "install")
            run_debug_install
            check_current_state
            ;;
        "test")
            run_widget_test
            check_config_problems
            ;;
        *)
            check_current_state
            echo ""
            read -p "¿Ejecutar pruebas completas? (s/n): " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Ss]$ ]]; then
                run_widget_test
                echo ""
                check_config_problems
                echo ""
                read -p "¿Limpiar e instalar nuevamente? (s/n): " -n 1 -r
                echo
                if [[ $REPLY =~ ^[Ss]$ ]]; then
                    reset_installation
                    run_debug_install
                fi
            fi
            ;;
    esac
    
    echo ""
    echo_color "info" "¡Depuración completada!"
}

# Ejecutar función principal
main "$@"