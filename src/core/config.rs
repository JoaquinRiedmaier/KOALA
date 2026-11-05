//! Parámetros de comportamiento. Todo lo que se desea ajustar vive acá.

/// REFRESCO AUTOMÁTICO (apagado por defecto, a pedido del usuario).
///
/// false = el widget solo mide cuando el usuario lo pide (clic izquierdo).
/// true  = cuando el caché vence (`REFRESH_SECS`), `koala bar` lanza en
///         background una medición nueva y refresca el widget solo.
///
/// Para activarlo: cambiar la constante y `cargo install --path .`,
/// o sin recompilar usar la variable de entorno `KOALA_AUTO_REFRESH=1`.
pub const AUTO_REFRESH: bool = false;

/// Con refresco automático activado: re-medir cuando el caché es más viejo que esto (segundos).
pub const REFRESH_SECS: u64 = 300;

/// Antes de esta edad el dato se marca visualmente como "viejo" (clase css `stale`).
pub const STALE_SECS: u64 = 600;

/// Paquetes ping por IP (los umbrales de `verdict` están pensados para 4).
pub const PINGS_POR_IP: u32 = 4;

/// Tamaño máximo del historial. Al superarlo se conserva el 75 % más reciente.
pub const HISTORIAL_LIMITE_BYTES: u64 = 8 * 1024 * 1024;

/// ¿Refresco automático? constant + override en runtime con `KOALA_AUTO_REFRESH=1`.
pub fn auto_refresh_activado() -> bool {
    AUTO_REFRESH
        || std::env::var("KOALA_AUTO_REFRESH")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}
