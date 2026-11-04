# KOALA Installation and Auto-Start Analysis

## Current Project Structure

This is a Rust project named "KOALA" that provides:
- Internet connectivity measurement via ping to multiple providers
- Waybar widget integration (via HyDE/Lua)
- GUI application for monitoring
- Configuration management for waybar integration

## Installation Commands

### 1. Install using Cargo (with charges)

```bash
# Navigate to the project directory
cd /home/johnd/Música/SOFTWARE/KOALA

# Install the binary
cargo install --path .

# This will create the binary at ~/.cargo/bin/koala
```

### 2. Alternative: Run directly from source

```bash
# For development/testing
cd /home/johnd/Música/SOFTWARE/KOALA
cargo run
```

## Current Auto-Start Configuration Issue

The project lacks proper auto-start configuration. For automatic startup, you need to:

### Linux Systems (systemd)

```bash
# Create a systemd service file
cat > ~/.config/systemd/user/koala.service << EOF
[Unit]
Description=KOALA Internet Monitor
After=network.target

[Service]
Type=simple
ExecStart=/home/johnd/.cargo/bin/koala barra
Restart=always
RestartSec=30

[Install]
WantedBy=default.target
EOF

# Enable and start the service
systemctl --user enable koala.service
systemctl --user start koala.service
```

### Alternative: Autostart Directory

```bash
# For desktop environments
cat > ~/.config/autostart/koala.desktop << EOF
[Desktop Entry]
Type=Application
Exec=/home/johnd/.cargo/bin/koala barra
Hidden=false
NoDisplay=false
Name=KOALA Internet Monitor
Comment=Monitors internet connectivity
Categories=Network;
EOF
```

## Waybar Duplication Issue Analysis

### Problem
When running `koala instalar`, entries are being duplicated in waybar.

### Root Cause Analysis

Looking at `src/install.rs`, I found several potential issues:

1. **Config.jsonc Detection Logic** (lines 108-110):
   ```rust
   if contenido.contains("\"custom/koala\"") {
       return Ok(String::from("ya presente"));
   }
   ```
   This simple string search can false-positive if `"custom/koala"` appears elsewhere in the file.

2. **CSS Marker Replacement** (lines 87-101):
   ```rust
   fn escribir_con_marcadores(ruta: &PathBuf, marca_ini: &str, marca_fin: &str, bloque: &str) -> Result<(), String> {
       let contenido = fs::read_to_string(ruta).unwrap_or_default();
       let nuevo = if let Some(ini) = contenido.find(marca_ini) {
           // ya existe el bloque: reemplazar solo esa sección
           let Some(fin) = contenido.find(marca_fin) else {
               return Err("marcador final faltante".into());
           };
           let fin = fin + marca_fin.len();
           format!("{}\n{bloque}\n{marca_fin}\n{}", &contenido[..ini], &contenido[fin..])
       } else {
           let sep = if contenido.is_empty() || contenido.ends_with('\n') { "" } else { "\n" };
           format!("{contenido}{sep}\n{marca_ini}\n{bloque}\n{marca_fin}\n")
       };
       fs::write(ruta, nuevo).map_err(|e| e.to_string())
   }
   ```
   This function should prevent duplication by replacing the entire marked section, but there might be edge cases.

### Solutions

#### 1. Fix the Config.jsonc Detection

```rust
// Replace the simple string search with exact JSON parsing
fn config_contains_custom_koala(contenido: &str) -> bool {
    // Parse the JSON and check if "custom/koala" is actually a module
    // A more robust implementation would use a JSON parser
    if let Ok(config) = serde_json::from_str::<serde_json::Value>(&contenido) {
        if let Some(modules) = config.get("bar").and_then(|bar| bar.get("modules")) {
            if let Some(modules_center) = modules.get("center") {
                return modules_center.as_array()
                    .map(|arr| arr.iter().any(|m| m == "custom/koala"))
                    .unwrap_or(false);
            }
        }
    }
    false
}
```

#### 2. Improve the CSS Marker System

```rust
// Current markers are case-sensitive and might conflict
fn escribir_con_marcadores_mejorado(ruta: &PathBuf, bloque: &str) -> Result<(), String> {
    let contenido = fs::read_to_string(ruta).unwrap_or_default();
    let marca_ini = "/* >>> koala (auto) >>> */";
    let marca_fin = "/* <<< koala <<< */";
    
    // Use a more robust marker replacement approach
    let nuevo_contenido = if let (Some(ini), Some(fin)) = (contenido.find(marca_ini), contenido.find(marca_fin)) {
        // Replace the entire section between markers
        let fin = fin + marca_fin.len();
        format!("{}\n{}\n{}", &contenido[..ini], bloque, &contenido[fin..])
    } else {
        // Add new block
        let sep = if contenido.is_empty() || contenido.ends_with('\n') { "" } else { "\n" };
        format!("{contenido}{sep}\n{marca_ini}\n{bloque}\n{marca_fin}\n")
    };
    
    fs::write(ruta, nuevo_contenido).map_err(|e| e.to_string())
}
```

#### 3. Debug and Test the Installation

To debug the duplication issue:

```bash
# Check current waybar config
ls -la ~/.config/waybar/
cat ~/.config/waybar/config.jsonc
cat ~/.config/waybar/modules/koala.jsonc
cat ~/.config/waybar/user-style.css
```

# Test running the install command
koala instalar

# Check if modules are duplicated
grep -o '"custom/koala"' ~/.config/waybar/config.jsonc | wc -l
```

## Recommended Fix

### Immediate Fix

Edit `src/install.rs` to improve the duplicate detection:

1. **Improve config.jsonc parsing**: Replace the simple string search with proper JSON parsing
2. **Better marker system**: Use more unique markers or UUIDs
3. **Add logging**: Add debug output to understand what's happening

### Complete Solution

1. Fix the duplicate detection logic in `colocar_en_config()`
2. Add comprehensive logging throughout the install process
3. Create a test script to verify the installation
4. Document the configuration and auto-start setup

## Next Steps

1. Implement the fixes to `src/install.rs`
2. Test the installation process
3. Set up auto-start configuration
4. Verify waybar integration works correctly without duplication

The core issue is that the current duplicate detection logic is too simplistic and can false-positive. The solution requires proper JSON parsing and more robust marker management.