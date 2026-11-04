//! Lectura de los objetivos (ips.json) desde su ubicación esperada.
//!
//! Orden de búsqueda (el primero que exista gana):
//!   1. `$KOALA_IPS`
//!   2. `~/.config/koala/ips.json`   (esto es lo que crea `koala install`)
//!   3. `./ips.json`                 (modo desarrollo, `cargo run` desde el repo)
//!   4. ips.json embebido en el binario (última reserva)

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Debug, Clone)]
pub struct Objetivo {
    pub nombre: String,
    pub ip4: Vec<String>,
    pub ip6: Vec<String>,
}

/// ips.json embebido: el binario funciona aunque el usuario no tenga configuración propia.
pub const IPS_EMBEBIDO: &str = include_str!("../../ips.json");

pub fn hogar() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

pub fn ruta_config_ips() -> PathBuf {
    hogar().join(".config").join("koala").join("ips.json")
}

pub fn objetivos() -> Vec<Objetivo> {
    // 1) variable de entorno
    if let Ok(p) = std::env::var("KOALA_IPS") {
        if let Ok(txt) = fs::read_to_string(&p) {
            if let Ok(v) = serde_json::from_str(&txt) {
                return v;
            }
        }
    }
    // 2) config usuario
    let cfg = ruta_config_ips();
    if let Ok(txt) = fs::read_to_string(&cfg) {
        if let Ok(v) = serde_json::from_str(&txt) {
            return v;
        }
    }
    // 3) repositorio (cargo run)
    if let Ok(txt) = fs::read_to_string("ips.json") {
        if let Ok(v) = serde_json::from_str(&txt) {
            return v;
        }
    }
    // 4) embebido
    serde_json::from_str(IPS_EMBEBIDO).expect("ips.json embebido inválido")
}
