//! Caché del último estado medido + lock de medición en curso.
//! Un solo dato vivo: `~/.cache/koala/state.json` (lo pinta la barra y lo lee la GUI).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::core::verdict::EstadoGlobal;

fn hogar() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

fn dir_cache() -> PathBuf {
    match std::env::var("XDG_CACHE_HOME") {
        Ok(c) if !c.is_empty() => PathBuf::from(c).join("koala"),
        _ => hogar().join(".cache").join("koala"),
    }
}

pub fn ruta_estado() -> PathBuf {
    dir_cache().join("state.json")
}

pub fn ruta_lock() -> PathBuf {
    dir_cache().join("midendo.lock")
}

/// Guarda el estado en el caché (respetando el formato del serde del estado).
pub fn guardar(estado: &EstadoGlobal) -> std::io::Result<()> {
    let dir = dir_cache();
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(estado)?;
    fs::write(dir.join("state.json"), json)
}

/// Carga el último estado medido, si existe.
pub fn cargar() -> Option<EstadoGlobal> {
    let txt = fs::read_to_string(ruta_estado()).ok()?;
    serde_json::from_str(&txt).ok()
}

/// Intenta tomar el lock de medición (archivo con el PID + bandera O_EXCL).
/// Devuelve false si ya hay una medición viva en curso. Limpia locks muertos.
pub fn tomar_lock() -> bool {
    let _ = fs::create_dir_all(dir_cache());
    let ruta = ruta_lock();
    if ruta.exists() {
        if let Ok(pid_txt) = fs::read_to_string(&ruta) {
            if let Ok(pid) = pid_txt.trim().parse::<u32>() {
                if PathBuf::from("/proc").join(pid.to_string()).exists() {
                    return false; // medición viva: no duplicar
                }
            }
        }
        let _ = fs::remove_file(&ruta); // lock huérfano: recuperarlo
    }
    let pid = std::process::id();
    match OpenOptions::new().write(true).create_new(true).open(&ruta) {
        Ok(mut f) => {
            let _ = writeln!(f, "{pid}");
            true
        }
        Err(_) => false,
    }
}

pub fn soltar_lock() {
    let _ = fs::remove_file(ruta_lock());
}
