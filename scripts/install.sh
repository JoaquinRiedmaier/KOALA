#!/bin/bash

# KOALA Installation Script
# Este script instala KOALA y configura el inicio automático

set -e

PROJECT_DIR="/home/johnd/Música/SOFTWARE/KOALA"
CARGO_BIN="$HOME/.cargo/bin/koala"

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # Sin color

# Funciones de ayuda
echo_color() {
    case $1 in
        "error") echo -e "${RED}[ERROR]${NC} $2" ;;
        "success") echo -e "${GREEN}[SUCCESS]${NC} $2" ;;
        "warning") echo -e "${YELLOW}[WARNING]${NC} $2" ;;
        "info") echo -e "${BLUE}[INFO]${NC} $2" ;;
        "debug") echo -e "${CYAN}[DEBUG]${NC} $2" ;;
        *) echo -e "[INFO] $2" ;;
    esac
}

# Función para imprimir banner
print_banner() {
    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}                INSTALACIÓN DE KOALA                        ${NC}║"
    echo -e "${CYAN}║${NC}                  Monitor de Conectividad                     ${NC}║"
    echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

# Función para imprimir sección
print_section() {
    echo ""
    echo -e "${YELLOW}▶ $1${NC}"
}

# Comprobar si cargo está instalado
check_prerequisites() {
    print_section "Verificación de prerequisitos"
    
    if ! command -v cargo &> /dev/null; then
        echo_color "error" "Rust/Cargo no está instalado."
        echo ""
        echo "Por favor instala Rust desde: https://rustup.rs/"
        echo ""
        echo "Comando de instalación típico:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        echo ""
        exit 1
    fi
    
    echo_color "success" "Cargo está instalado: $(cargo --version)"
}

# Comprobar si cargo ya tiene la binary de koala
install_binary() {
    print_section "Instalación del binario KOALA"
    
    if [ -f "$CARGO_BIN" ]; then
        echo_color "warning" "KOALA ya está instalado en $CARGO_BIN"
        echo ""
        echo "Para actualizar, ejecuta:"
        echo "  cd $PROJECT_DIR && cargo install --path . --force"
        echo ""
        return 0
    fi
    
    echo_color "info" "Compilando KOALA desde el código fuente..."
    cd "$PROJECT_DIR"
    
    # Verificar si el código se puede compilar
    if ! cargo check 2>/dev/null; then
        echo_color "warning" "Verificación de código falló, intentando compilar..."
        if ! cargo build --release; then
            echo_color "error" "Fallo en la compilación de KOALA"
            echo ""
            echo "Posibles soluciones:"
            echo "  1. Asegurar que Rust esté actualizado: cargo install --update"
            echo "  2. Verificar conexiones de network"
            echo "  3. Revisar logs de compilación arriba"
            exit 1
        fi
    fi
    
    # Compilar y instalar
    echo_color "info" "Instalando binario..."
    if cargo install --path .; then
        echo_color "success" "KOALA instalado exitosamente en $CARGO_BIN"
    else
        echo_color "error" "Fallo en la instalación de KOALA"
        echo ""
        echo "Intenta instalar manualmente:"
        echo "  cd $PROJECT_DIR && cargo install --path . --verbose"
        exit 1
    fi
}

# Configurar inicio automático
setup_automatic_start() {
    print_section "Configuración de inicio automático"
    
    echo_color "info" "Intentando configurar inicio automático..."
    
    # Función para configurar systemd para usuarios
    setup_systemd_user() {
        echo_color "debug" "Intentando configurar systemd --user..."
        
        local service_file="$HOME/.config/systemd/user/koala.service"
        local service_content="[Unit]
Description=KOALA Internet Monitor
After=network.target

[Service]
Type=simple
ExecStart=$CARGO_BIN barra
Restart=always
RestartSec=30
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=default.target
"
        
        mkdir -p "$HOME/.config/systemd/user"
        echo -e "$service_content" > "$service_file"
        
        if systemctl --user daemon-reload 2>/dev/null; then
            systemctl --user enable koala.service
            systemctl --user start koala.service
            echo_color "success" "Servicio systemd configurado y iniciado"
            
            # Verificar estado
            if systemctl --user is-active koala.service 2>/dev/null | grep -q "active"; then
                echo_color "success" "Servicio KOALA está corriendo"
            else
                echo_color "warning" "Servicio KOALA no está activo"
            fi
            return 0
        else
            echo_color "warning" "No se pudo configurar systemd --user (¿No hay sesión de usuario?)"
            return 1
        fi
    }
    
    # Función para configurar autostart para entornos de escritorio
    setup_desktop_autostart() {
        echo_color "debug" "Configurando autostart de escritorio..."
        
        local desktop_file="$HOME/.config/autostart/koala.desktop"
        local desktop_content="[Desktop Entry]
Type=Application
Exec=$CARGO_BIN barra
Hidden=false
NoDisplay=false
Name=KOALA Internet Monitor
Comment=Monitor de conectividad de internet usando múltiples proveedores
Categories=Network;
Terminal=true
StartupNotify=true
"
        
        mkdir -p "$HOME/.config/autostart"
        echo -e "$desktop_content" > "$desktop_file"
        
        if [ -f "$desktop_file" ]; then
            echo_color "success" "Autostart de escritorio configurado en $desktop_file"
            return 0
        else
            echo_color "error" "Fallo creando archivo de autostart"
            return 1
        fi
    }
    
    # Intentar configurar systemd primero
    if command -v systemctl &> /dev/null; then
        if setup_systemd_user; then
            echo_color "success" "Método de inicio automático preferido (systemd) usado"
            return 0
        else
            echo_color "info" "systemd --user no disponible, intentando autostart de escritorio..."
            setup_desktop_autostart
            return $?
        fi
    else
        echo_color "warning" "systemctl no disponible, usando autostart de escritorio..."
        setup_desktop_autostart
        return $?
    fi
}

# Ejecutar koala instalar para configurar waybar
install_to_waybar() {
    print_section "Integración con Waybar"
    
    if [ ! -f "$CARGO_BIN" ]; then
        echo_color "error" "Binario de KOALA no encontrado, saltando configuración de waybar"
        echo "Instala KOALA primero: cargo install --path $PROJECT_DIR"
        return 1
    fi
    
    echo_color "info" "Ejecutando 'koala instalar' para integrar con waybar..."
    echo ""
    echo "NOTA: Este comando va a:"
    echo "  1. Copiar el archivo de configuración de IPs si no existe"
    echo "  2. Crear ~/.config/waybar/modules/koala.jsonc"
    echo "  3. Agregar estilos CSS en ~/.config/waybar/user-style.css"
    echo "  4. Colocar el módulo en ~/.config/waybar/config.jsonc"
    echo "  5. Agregar regla de ventana flotante a Hyprland"
    echo "  6. Reiniciar waybar si está corriendo"
    echo ""
    
    if $CARGO_BIN instalar; then
        echo_color "success" "¡Integración con waybar completada!"
        return 0
    else
        echo_color "error" "Fallo en la integración con waybar"
        echo ""
        echo "Puedes intentar manualmente:"
        echo "  $CARGO_BIN instalar"
        echo ""
        echo "Para verificar la salida completa, ejecuta:"
        echo "  $CARGO_BIN instalar 2>&1"
        return 1
    fi
}

# Ejecutar tests después de la instalación
run_post_install_tests() {
    print_section "Tests post-instalación"
    
    echo_color "info" "Ejecutando tests post-instalación..."
    echo ""
    
    # Test de widget
    echo "Test 1: Generando JSON del widget..."
    if timeout 10 $CARGO_BIN barra > /tmp/koala_widget_test.json 2>&1; then
        echo_color "success" "Widget JSON generado"
        
        # Verificar validez del JSON
        if python3 -m json.tool /tmp/koala_widget_test.json > /dev/null 2>&1; then
            echo_color "success" "JSON del widget es válido"
        else
            echo_color "warning" "JSON del widget puede ser inválido"
        fi
    else
        echo_color "error" "Fallo en test de widget"
        echo "Ver logs: cat /tmp/koala_widget_test.json"
    fi
    
    # Test de metricas (ejecución rápida)
    echo ""
    echo "Test 2: Ejecutando medición rápida..."
    if timeout 15 $CARGO_BIN medir > /tmp/koala_measure_test.log 2>&1; then
        echo_color "success" "Medición completada"
        
        # Verificar que se crearon archivos de cache
        if [ -f "$HOME/.cache/koala/" ] || find "$HOME/.cache" -name "*koala*" -type f 2>/dev/null | head -1 > /dev/null; then
            echo_color "success" "Cache de KOALA creado"
        fi
    else
        echo_color "warning" "Medición puede haber fallado (timeout o error)"
        echo "Ver logs: cat /tmp/koala_measure_test.log"
    fi
}

# Función principal principal
main() {
    print_banner
    
    # Parsear argumentos
    local debug_mode=false
    while [[ $# -gt 0 ]]; do
        case $1 in
            "--debug")
                debug_mode=true
                shift
                ;;
            "--help"|"-h")
                echo "Uso: $0 [OPCIÓN]"
                echo ""
                echo "Opciones:"
                echo "  --debug   Ejecutar en modo debug (más detalles)"
                echo "  --help    Mostrar esta ayuda"
                echo ""
                echo "Este script instalará KOALA y lo configurará para inicio automático"
                exit 0
                ;;
            *)
                echo_color "error" "Opción desconocida: $1"
                echo "Usa --help para ver las opciones disponibles"
                exit 1
                ;;
        esac
    done
    
    if [ "$debug_mode" = true ]; then
        echo_color "debug" "Modo debug activado"
    fi
    
    # Ejecutar flujo de instalación
    check_prerequisites
    install_binary
    setup_automatic_start
    install_to_waybar
    run_post_install_tests
    
    # Mensaje final
    print_banner
    echo_color "success" "¡¡¡ INSTALACIÓN DE KOALA COMPLETADA !¡¡"
    echo ""
    echo "=== Resumen del estado ==="
    echo "✓ Binario instalado: $CARGO_BIN"
    echo "✓ Inicio automático configurado"
    echo "✓ Integración con waybar completada"
    echo "✓ Tests post-instalación ejecutados"
    echo ""
    echo "=== Próximos pasos ==="
    echo "1. Reiniciar waybar:"
    echo "   killall waybar && waybar"
    echo ""
    echo "2. Probar KOALA:"
    echo "   $CARGO_BIN medir && $CARGO_BIN barra"
    echo ""
    echo "3. Abrir ventana gráfica:"
    echo "   $CARGO_BIN gui"
    echo ""
    echo "4. Para ver logs o debuggear:"
    echo "   ./scripts/debug_duplication.sh"
    echo "   ./scripts/test_installation.sh"
    echo ""
    echo "=== Para más ayuda ==="
    echo "- Revisar README.md en el directorio del proyecto"
    echo "- Revisar analysis_report.md para detalles técnicos"
    echo "- Ejecutar: ./scripts/install.sh --help"
}

# Ejecutar función principal
main "$@"