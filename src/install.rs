//! `koala instalar`: integra el widget en HyDE (waybar), de forma idempotente y con backups.
//!
//! Qué toca:
//!   1. `~/.config/koala/ips.json`   — copia del ips.json si el usuario no tiene uno
//!   2. `~/.config/waybar/modules/koala.jsonc` — definición del módulo (HyDE la incluye auto)
//!   3. `~/.config/waybar/user-style.css`      — clases de color (bloque entre marcadores)
//!   4. `~/.config/waybar/config.jsonc`        — coloca `custom/koala` en la barra (con .bak)
//!   5. recarga waybar, solo si alguno de los archivos anteriores cambió
//!
//! Se puede ejecutar en cada inicio de sesión: si no hay nada que cambiar, no escribe nada
//! ni recarga nada.
//!
//! Nota: HyDE regenera `config.jsonc` al cambiar de tema → si el widget desaparece,
//! ejecutar `koala instalar` de nuevo (los otros archivos sobreviven).

use std::fs;
use std::io::ErrorKind;
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

/// Inserta `bloque` entre marcadores, o lo actualiza si ya está. Idempotente: aplicarlo
/// sobre su propio resultado devuelve exactamente el mismo texto.
fn aplicar_bloque(contenido: &str, marca_ini: &str, marca_fin: &str, bloque: &str) -> Result<String, String> {
    let Some(ini) = contenido.find(marca_ini) else {
        let sep = if contenido.is_empty() || contenido.ends_with('\n') { "" } else { "\n" };
        return Ok(format!("{contenido}{sep}\n{marca_ini}\n{bloque}\n{marca_fin}\n"));
    };
    // El final se busca desde el inicio del bloque: un marcador suelto más arriba no cuenta.
    let Some(largo) = contenido[ini..].find(marca_fin) else {
        return Err("marcador final faltante".into());
    };
    let fin = ini + largo + marca_fin.len();
    Ok(format!("{}{marca_ini}\n{bloque}\n{marca_fin}{}", &contenido[..ini], &contenido[fin..]))
}

/// Escribe `contenido` solo si difiere de lo que hay. Devuelve si el archivo cambió.
fn escribir_si_cambia(ruta: &PathBuf, contenido: &str) -> Result<bool, String> {
    if fs::read_to_string(ruta).is_ok_and(|actual| actual == contenido) {
        return Ok(false);
    }
    fs::write(ruta, contenido).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Aplica el bloque sobre un archivo que puede no existir todavía. Devuelve si cambió.
fn escribir_con_marcadores(ruta: &PathBuf, marca_ini: &str, marca_fin: &str, bloque: &str) -> Result<bool, String> {
    let contenido = match fs::read_to_string(ruta) {
        Ok(c) => c,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", ruta.display())),
    };
    escribir_si_cambia(ruta, &aplicar_bloque(&contenido, marca_ini, marca_fin, bloque)?)
}

/// Inserta `"custom/koala"` después de `"custom/caffeine"` (pill izquierdo) en el config.jsonc
/// de HyDE. Si ya está, no toca nada. Devuelve si el archivo cambió.
fn colocar_en_config(ruta: &PathBuf) -> Result<bool, String> {
    let contenido = fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    if contenido.contains("\"custom/koala\"") {
        return Ok(false);
    }

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
    if !colocado {
        return Err("no se encontró \"custom/caffeine\" en config.jsonc".into());
    }
    let backup = ruta.with_file_name(format!("{}.koala-bak", ruta.file_name().and_then(|n| n.to_str()).unwrap_or("config.jsonc")));
    let _ = fs::write(&backup, &contenido);
    fs::write(ruta, salida).map_err(|e| e.to_string())?;
    Ok(true)
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
            let _ = Command::new("pkill").args(["-SIGUSR2", "-x", "waybar"]).output();
            println!("waybar recargada");
        }
        _ => println!("waybar no estaba corriendo: no se recarga"),
    }
}

/// Parches el hyprland.lua del usuario (no lo reescribe HyDE) con la regla flotante.
/// Si la regla ya está igual, no hace backup, no escribe y no recarga Hyprland.
fn colocar_regla_hyprland() -> Result<String, String> {
    let ruta = casa().join(".config").join("hypr").join("hyprland.lua");
    if !ruta.exists() {
        return Err("no existe ~/.config/hypr/hyprland.lua".into());
    }
    let actual = fs::read_to_string(&ruta).map_err(|e| e.to_string())?;
    let nuevo = aplicar_bloque(&actual, MARCA_LUA_INI, MARCA_LUA_FIN, LUA_REGLA)?;
    if nuevo == actual {
        return Ok(String::from("regla ya presente"));
    }
    let _ = fs::write(ruta.with_file_name("hyprland.lua.koala-bak"), &actual);
    fs::write(&ruta, nuevo).map_err(|e| e.to_string())?;
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

    // waybar solo se recarga si algo de lo que lee cambió
    let mut hay_cambios = false;

    match escribir_si_cambia(&dir.join("modules").join("koala.jsonc"), &generar_modulo(&bin)) {
        Ok(cambio) => {
            hay_cambios |= cambio;
            println!("2/4 módulo → ~/.config/waybar/modules/koala.jsonc");
        }
        Err(e) => eprintln!("módulo: {e}"),
    }

    match escribir_con_marcadores(&dir.join("user-style.css"), MARCA_INI, MARCA_FIN, CSS) {
        Ok(cambio) => {
            hay_cambios |= cambio;
            println!("3/4 colores → ~/.config/waybar/user-style.css");
        }
        Err(e) => eprintln!("estilos: {e}"),
    }

    match colocar_en_config(&dir.join("config.jsonc")) {
        Ok(true) => {
            hay_cambios = true;
            println!("4/4 posición en config.jsonc: agregado junto a caffeine (backup .koala-bak)");
        }
        Ok(false) => println!("4/4 posición en config.jsonc: ya presente"),
        Err(e) => eprintln!("colocación: {e}"),
    }

    match colocar_regla_hyprland() {
        Ok(detalle) => println!("gui: hyprland.lua — {detalle}"),
        Err(e) => eprintln!("gui: regla flotante omitida — {e}"),
    }

    if hay_cambios {
        reiniciar_waybar();
    } else {
        println!("sin cambios: waybar no se recarga");
    }
    println!("Listo. Probar: `koala medir && koala barra`");
}

#[cfg(test)]
mod tests {
    use super::*;

    const INI: &str = "/* >>> k >>> */";
    const FIN: &str = "/* <<< k <<< */";

    fn aplicar(contenido: &str) -> String {
        aplicar_bloque(contenido, INI, FIN, "bloque").unwrap()
    }

    #[test]
    fn repetir_no_cambia_el_archivo() {
        let primera = aplicar("/* del usuario */\n");
        let mut actual = primera.clone();
        for _ in 0..30 {
            actual = aplicar(&actual);
        }
        assert_eq!(actual, primera);
        assert_eq!(actual.matches(INI).count(), 1);
        assert_eq!(actual.matches(FIN).count(), 1);
    }

    #[test]
    fn conserva_lo_que_rodea_al_bloque() {
        let archivo = format!("antes\n{INI}\nviejo\n{FIN}\ndespués\n");
        assert_eq!(aplicar(&archivo), format!("antes\n{INI}\nbloque\n{FIN}\ndespués\n"));
    }

    /// Estado que dejaban las versiones con el bug: un final huérfano antes del bloque.
    /// Antes esto duplicaba el archivo en cada ejecución.
    #[test]
    fn un_final_huerfano_no_hace_crecer_el_archivo() {
        let archivo = format!("huérfano\n{FIN}\nmedio\n{INI}\nbloque\n{FIN}\n");
        assert_eq!(aplicar(&archivo), archivo);
    }

    #[test]
    fn falla_si_el_bloque_no_tiene_final() {
        let archivo = format!("{FIN}\n{INI}\nbloque\n");
        assert!(aplicar_bloque(&archivo, INI, FIN, "bloque").is_err());
    }

    #[test]
    fn agrega_el_bloque_en_un_archivo_sin_salto_final() {
        assert_eq!(aplicar("a"), format!("a\n\n{INI}\nbloque\n{FIN}\n"));
    }

    #[test]
    fn las_marcas_reales_son_idempotentes() {
        let css = aplicar_bloque("", MARCA_INI, MARCA_FIN, CSS).unwrap();
        assert_eq!(aplicar_bloque(&css, MARCA_INI, MARCA_FIN, CSS).unwrap(), css);
        let lua = aplicar_bloque("", MARCA_LUA_INI, MARCA_LUA_FIN, LUA_REGLA).unwrap();
        assert_eq!(aplicar_bloque(&lua, MARCA_LUA_INI, MARCA_LUA_FIN, LUA_REGLA).unwrap(), lua);
    }
}
