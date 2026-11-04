//! `koala instalar`: integra el widget en HyDE (waybar), de forma idempotente y con backups.
//!
//! Qué toca:
//!   1. `~/.config/koala/ips.json`   — copia del ips.json si el usuario no tiene uno
//!   2. `~/.config/waybar/modules/koala.jsonc` — definición del módulo (HyDE la incluye auto)
//!   3. `~/.config/waybar/user-style.css`      — clases de color (bloque entre marcadores)
//!   4. `~/.config/waybar/config.jsonc`        — coloca `custom/koala` en la barra (con .bak)
//!   5. reinicia waybar si está corriendo
//!
//! Nota: HyDE regenera `config.jsonc` al cambiar de tema → si el widget desaparece,
//! ejecutar `koala instalar` de nuevo (los otros archivos sobreviven).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const MARCA_INI: &str = "/* >>> koala (auto) >>> */";
const MARCA_FIN: &str = "/* <<< koala <<< */";

/// Definición del módulo waybar, con rutas absolutas (el PATH del entorno de
/// waybar no suele incluir ~/.cargo/bin).
fn generar_modulo(bin: &str) -> String {
    format!(
        r#"{{
    "custom/koala": {{
        "format": "{{text}}",
        "exec": "{bin} barra",
        "return-type": "json",
        "interval": 5,
        "on-click": "{bin} medir",
        "on-click-right": "{bin} gui",
        "tooltip": true
    }}
}}"#
    )
}

/// Clases css: colores por letra (sin emojis, pedido del usuario) + atenuado para datos viejos.
const CSS: &str = r#"
/* widget KOALA */
#custom-koala {
    padding: 0 10px;
    margin: 0 4px;
    border-radius: 0.5em;
    background-color: rgba(17, 17, 27, 0.45);
    color: #cdd6f4;
    font-weight: 600;
}
#custom-koala.idle {
    color: #7f849c;
}
#custom-koala.ok {
    color: #a6e3a1;
}
#custom-koala.slow {
    color: #f9e2af;
}
#custom-koala.bad {
    color: #fab387;
}
#custom-koala.offline {
    color: #f38ba8;
}
#custom-koala.stale {
    opacity: 0.65;
    font-style: italic;
}
"#;

const MARCA_LUA_INI: &str = "-- >>> koala (auto) >>>";
const MARCA_LUA_FIN: &str = "-- <<< koala <<<";

/// Regla flotante para la ventana GUI (Hyprland vía HyDE/Lua).
/// El matching de HyDE es full-regex: "kOala.*" cubre "kOala — medidor…".
const LUA_REGLA: &str = r#"hl.window_rule({
    name = "koala_gui_float",
    match = { title = "kOala.*" },
    float = true
})"#;

const IPS_BASE: &str = include_str!("../ips.json");

fn casa() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

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

/// Inserta `"custom/koala"` en la lista de módulos del config.jsonc de HyDE.
/// Ancla 1: después de `"custom/caffeine",` (pill izquierdo). Ancla 2: dentro del primer
/// array de `"modules-center"`. Si ya está, no toca nada.
fn colocar_en_config(ruta: &PathBuf) -> Result<String, String> {
    let contenido = fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    if contenido.contains("\"custom/koala\"") {
        return Ok(String::from("ya presente"));
    }
    // backup
    let backup = ruta.with_file_name(format!("{}.koala-bak", ruta.file_name().and_then(|n| n.to_str()).unwrap_or("config.jsonc")));
    let _ = fs::write(&backup, &contenido);

    // re-arma el archivo línea por línea (conserva los terminadores originales)
    let lineas: Vec<&str> = contenido.split_inclusive('\n').collect();
    let mut salida = String::with_capacity(contenido.len() + 40);
    let mut colocado = false;
    for linea in lineas {
        let limpia = linea.trim_end_matches('\n').trim_end_matches('\r');
        let fin = if linea.ends_with("\r\n") { "\r\n" } else { "\n" };
        if !colocado && limpia.contains("\"custom/caffeine\"") {
            let indent = limpia
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect::<String>();
            let mut linea_ok = limpia.to_owned();
            if !linea_ok.ends_with(',') {
                linea_ok.push(',');
            }
            salida.push_str(&format!("{linea_ok}{fin}{indent}\"custom/koala\",{fin}"));
            colocado = true;
        } else {
            salida.push_str(linea);
        }
    }
    fs::write(ruta, salida).map_err(|e| e.to_string())?;
    Ok(String::from("agregado junto a caffeine (pill izquierdo)"))
}

fn actualizar_ips() -> Result<(), String> {
    let destino = casa().join(".config").join("koala");
    let ruta = destino.join("ips.json");
    if ruta.exists() {
        return Ok(()); // respeta la configuración del usuario
    }
    fs::create_dir_all(&destino).map_err(|e| e.to_string())?;
    fs::write(&ruta, IPS_BASE).map_err(|e| e.to_string())
}

fn reiniciar_waybar() {
    let corriendo = Command::new("pgrep").arg("-x").arg("waybar").output().ok();
    match corriendo {
        Some(o) if !o.stdout.is_empty() => {
            let _ = Command::new("sh")
                .arg("-c")
                .arg("killall waybar 2>/dev/null; sleep 0.3; nohup waybar >/dev/null 2>&1 &")
                .spawn();
            println!("waybar reiniciada");
        }
        _ => println!("waybar no estaba corriendo: no se reinicia"),
    }
}

/// Parches el hyprland.lua del usuario (no lo reescribe HyDE) con la regla flotante.
fn colocar_regla_hyprland() -> Result<String, String> {
    let ruta = casa().join(".config").join("hypr").join("hyprland.lua");
    if !ruta.exists() {
        return Err("no existe ~/.config/hypr/hyprland.lua".into());
    }
    let backup = ruta.with_file_name("hyprland.lua.koala-bak");
    let _ = fs::write(&backup, fs::read_to_string(&ruta).unwrap_or_default());
    escribir_con_marcadores(&ruta, MARCA_LUA_INI, MARCA_LUA_FIN, &LUA_REGLA)?;
    // recargar la config si hyprctl está disponible
    let _ = Command::new("hyprctl").arg("reload").output();
    Ok(String::from("regla agregada (ventana flotante kOala)"))
}

pub fn instalar() {
    println!("== KOALA instalar ==");

    // binario en uso (ruta absoluta): waybar no hereda el PATH de ~/.cargo/bin
    let bin = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| {
            let p = casa().join(".cargo").join("bin").join("koala");
            p.to_string_lossy().into_owned()
        });
    println!("binario: {bin}");

    let _ = actualizar_ips().map_err(|e| eprintln!("ips: {e}"));
    println!("1/4 ips.json → ~/.config/koala/ips.json (si no existía)");

    let dir = casa().join(".config").join("waybar");
    let _ = fs::create_dir_all(dir.join("modules"));

    match fs::write(dir.join("modules").join("koala.jsonc"), generar_modulo(&bin)) {
        Ok(_) => println!("2/4 módulo → ~/.config/waybar/modules/koala.jsonc"),
        Err(e) => eprintln!("módulo: {e}"),
    }

    match escribir_con_marcadores(&dir.join("user-style.css"), MARCA_INI, MARCA_FIN, CSS) {
        Ok(_) => println!("3/4 colores → ~/.config/waybar/user-style.css"),
        Err(e) => eprintln!("estilos: {e}"),
    }

    match colocar_en_config(&dir.join("config.jsonc")) {
        Ok(detalle) => println!("4/4 posición en config.jsonc: {detalle} (backup .koala-bak)"),
        Err(e) => eprintln!("colocación: {e}"),
    }

    match colocar_regla_hyprland() {
        Ok(detalle) => println!("gui: hyprland.lua — {detalle}"),
        Err(e) => eprintln!("gui: regla flotante omitida — {e}"),
    }

    reiniciar_waybar();
    println!("Listo. Probar: `koala medir && koala barra`");
}
