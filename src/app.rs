//! Manejadores de los subcomandos: `medir`, `barra`.

use std::process::{Command, Stdio};

use crate::core::cache;
use crate::core::config;
use crate::core::ipfile;
use crate::core::verdict::{evaluar, EstadoGlobal};

/// `koala medir`: mide TODO, escribe el caché. La barra se refresca sola en su próximo tick.
pub fn comando_medir() {
    match medir() {
        Some(e) => println!("KOALA: {} — {}", e.texto(), formateo_humano_edad(e.edad())),
        None => println!("KOALA: ya hay una medición en curso o falló el arranque."),
    }
}

/// Corre una medición completa y guarda el caché. Devuelve el estado calculado.
/// Bloquea ~2–4 s (los pings corren en paralelo). Respeta el lock.
pub fn medir() -> Option<EstadoGlobal> {
    if !cache::tomar_lock() {
        return cache::cargar(); // otra medición ya corre: devolver lo último
    }
    let objetivos = ipfile::objetivos();
    let estado = evaluar(&objetivos);
    let ok = cache::guardar(&estado).is_ok();
    cache::soltar_lock();
    if ok {
        Some(estado)
    } else {
        eprintln!("KOALA: no se pudo guardar el caché");
        None
    }
}

/// `koala barra`: imprime 1 línea JSON para waybar (return-type json), a partir del caché.
/// No mide en el hilo del widget: siempre responde al instante con lo último conocido.
///
/// Si el refresco automático está activado (`core::config::AUTO_REFRESH` o
/// `KOALA_AUTO_REFRESH=1`) y el caché venció, lanza `koala medir` en background.
pub fn comando_barra() {
    let (texto, clases, tooltip) = match cache::cargar() {
        Some(e) => {
            let edad = e.edad();
            tal_ves_refrescar(&e);
            let mut clases = vec![e.clase()];
            if edad > config::STALE_SECS {
                clases.push("stale");
            }
            (e.texto(), clases, append_edad_vieja(e.tooltip(), edad))
        }
        // sin caché inicial: se invita a medir con un clic
        None => (
            "KOALA".into(),
            vec!["idle"],
            "Todavía no hay medición.\nClic: medir ahora · Clic der: interfaz".into(),
        ),
    };
    let json = serde_json::json!({
        "text": texto,
        "class": clases,
        "tooltip": tooltip,
    });
    println!("{json}");
}

/// Lanza `koala medir` desacoplado si el caché venció (modo refresco automático).
fn tal_ves_refrescar(e: &EstadoGlobal) {
    if !config::auto_refresh_activado() || e.edad() < config::REFRESH_SECS {
        return;
    }
    // el lock de "medir" evita duplicar medición si un spawn anterior aún corre
    if cache::ruta_lock().exists() {
        return;
    }
    let cmd = match ruta_koala() {
        Some(p) => format!("{p} medir"),
        None => "koala medir".into(),
    };
    let _ = Command::new("sh")
        .args(["-c", &format!("nohup {cmd} >/dev/null 2>&1 &")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Busca `koala` en $PATH o en ~/.cargo/bin (waybar no hereda ese PATH).
fn ruta_koala() -> Option<String> {
    let es_ejecutable = |p: &std::path::Path| -> bool {
        let Ok(md) = std::fs::metadata(p) else { return false };
        use std::os::unix::fs::PermissionsExt;
        md.is_file() && md.permissions().mode() & 0o111 != 0
    };
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let p = std::path::PathBuf::from(dir).join("koala");
            if es_ejecutable(&p) {
                return Some(p.to_string_lossy().into_owned());
            }
        }
    }
    let cargo = crate::core::ipfile::hogar().join(".cargo").join("bin").join("koala");
    if es_ejecutable(&cargo) {
        return Some(cargo.to_string_lossy().into_owned());
    }
    None
}

fn formateo_humano_edad(seg: u64) -> String {
    if seg < 60 {
        format!("hace {seg} s")
    } else {
        format!("hace {} min", seg / 60)
    }
}

/// Nota adicional en el tooltip si el dato ya está viejo (clase 'stale' activa).
fn append_edad_vieja(tip: String, edad: u64) -> String {
    if edad > config::STALE_SECS {
        format!(
            "{tip}\n(dato viejo — {} — hacé clic para actualizar)",
            formateo_humano_edad(edad)
        )
    } else {
        tip
    }
}
