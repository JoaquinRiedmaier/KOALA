//! Veredicto global: agrupa resultados por proveedor y calcula el estado de la conexión.
//! Umbrales = los originales de KOALA: avg > 50 ms, mdev > 10 ms, pérdida > 25 %.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::ipfile::Objetivo;
use crate::core::ping::{medir_lote, Metricas, ResultadoPing};

pub const LIMITE_MS: f32 = 50.0;
pub const LIMITE_JITTER: f32 = 10.0;
pub const LIMITE_PERDIDA: u8 = 25;
/// Si 2 o más proveedores están 100 % caídos => estado "bad".
pub const PROVEEDORES_CAIDOS_PARA_BAD: usize = 2;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Estado {
    Ok,
    Slow,
    Bad,
    Offline,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Proveedor {
    pub nombre: String,
    pub ips: Vec<ResultadoPing>,
    /// media de las IPs que respondieron
    pub media: Option<f32>,
    /// peor variación (mdev) entre las IPs que respondieron
    pub jitter: Option<f32>,
    pub peor_perdida: u8,
    /// false si TODAS sus IPs tuvieron 100 % de pérdida
    pub al_alcance: bool,
}

/// Instantánea completa de una medición (mismo dato que guarda el caché).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EstadoGlobal {
    pub ts: u64,
    pub estado: Estado,
    pub media_ms: Option<f32>,
    pub peor_perdida: u8,
    pub proveedores: Vec<Proveedor>,
}

fn ahora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn mediana(v: &[f32]) -> Option<f32> {
    if v.is_empty() {
        return None;
    }
    let mut v = v.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n % 2 == 1 {
        Some(v[n / 2])
    } else {
        Some((v[n / 2 - 1] + v[n / 2]) / 2.0)
    }
}

/// Mide todos los objetivos y calcula el veredicto global.
pub fn evaluar(objetivos: &[Objetivo]) -> EstadoGlobal {
    // Armar la lista plana de (índice, destino, familia) y los rangos de cada proveedor.
    let mut lista: Vec<(usize, String, u8)> = Vec::new();
    let mut rangos: Vec<(String, usize, usize)> = Vec::new();
    for o in objetivos {
        let inicio = lista.len();
        for ip in &o.ip4 {
            lista.push((lista.len(), ip.clone(), 4));
        }
        for ip in &o.ip6 {
            lista.push((lista.len(), ip.clone(), 6));
        }
        // dominios: familia 0 = auto, el sistema resuelve el nombre al pinguear
        for dominio in &o.dominios {
            lista.push((lista.len(), dominio.clone(), 0));
        }
        let fin = lista.len();
        rangos.push((o.nombre.clone(), inicio, fin));
    }

    let resultados = medir_lote(lista);

    // Agrupar por proveedor.
    let mut proveedores = Vec::with_capacity(rangos.len());
    for (nombre, inicio, fin) in &rangos {
        let ips: Vec<ResultadoPing> = resultados[*inicio..*fin]
            .iter()
            .map(|r| {
                let mut r = r.clone();
                r.indice -= *inicio;
                r
            })
            .collect();
        let buenas: Vec<&Metricas> = ips
            .iter()
            .filter_map(|r| r.metricas.as_ref())
            .filter(|m| m.packet_loss < 100)
            .collect();
        let media = mediana(
            &buenas.iter().filter_map(|m| m.avg).collect::<Vec<f32>>(),
        );
        let jitter = buenas.iter().filter_map(|m| m.mdev).reduce(f32::max);
        let peor_perdida = ips
            .iter()
            .filter_map(|r| r.metricas.as_ref())
            .map(|m| m.packet_loss)
            .max()
            .unwrap_or(100);
        let al_alcance = media.is_some(); // media solo existe si alguna IP respondió
        proveedores.push(Proveedor {
            nombre: nombre.clone(),
            ips,
            media,
            jitter,
            peor_perdida,
            al_alcance,
        });
    }

    let al_alcance: Vec<&Proveedor> = proveedores.iter().filter(|p| p.al_alcance).collect();
    let caidos = proveedores.iter().filter(|p| !p.al_alcance).count();
    let peor_perdida = proveedores
        .iter()
        .map(|p| p.peor_perdida)
        .max()
        .unwrap_or(100);

    let (estado, media_ms) = if al_alcance.is_empty() {
        (Estado::Offline, None)
    } else {
        let media = mediana(&al_alcance.iter().filter_map(|p| p.media).collect::<Vec<f32>>());
        let lento = media.is_some_and(|m| m > LIMITE_MS)
            || al_alcance.iter().any(|p| p.jitter.is_some_and(|j| j > LIMITE_JITTER));
        let estado = if caidos >= PROVEEDORES_CAIDOS_PARA_BAD || peor_perdida > LIMITE_PERDIDA {
            Estado::Bad
        } else if lento {
            Estado::Slow
        } else {
            Estado::Ok
        };
        (estado, media)
    };

    EstadoGlobal {
        ts: ahora(),
        estado,
        media_ms,
        peor_perdida,
        proveedores,
    }
}

impl Estado {
    pub fn etiqueta_es(&self) -> &'static str {
        match self {
            Estado::Ok => "OK",
            Estado::Slow => "LENTO",
            Estado::Bad => "INESTABLE",
            Estado::Offline => "SIN RED",
        }
    }
}

/// Estado textual de un proveedor a partir de sus métricas. Sin `media`, ninguna IP respondió.
pub fn etiqueta_proveedor(media: Option<f32>, jitter: Option<f32>, peor_perdida: u8) -> &'static str {
    if media.is_none() {
        "SIN RESPUESTA"
    } else if peor_perdida > LIMITE_PERDIDA {
        "INESTABLE"
    } else if media.is_some_and(|m| m > LIMITE_MS) || jitter.is_some_and(|j| j > LIMITE_JITTER) {
        "LENTO"
    } else {
        "OK"
    }
}

impl Proveedor {
    /// Estado textual de un proveedor, para GUI y tooltip.
    pub fn etiqueta(&self) -> &'static str {
        etiqueta_proveedor(self.media, self.jitter, self.peor_perdida)
    }
}

impl EstadoGlobal {
    /// Segundos transcurridos desde esta medición.
    pub fn edad(&self) -> u64 {
        ahora().saturating_sub(self.ts)
    }

    /// Clase css para waybar ("ok" | "slow" | "bad" | "offline").
    pub fn clase(&self) -> &'static str {
        match self.estado {
            Estado::Ok => "ok",
            Estado::Slow => "slow",
            Estado::Bad => "bad",
            Estado::Offline => "offline",
        }
    }

    /// Texto del widget: escueto, sin emojis ni leds (una palabra + dato).
    pub fn texto(&self) -> String {
        match self.estado {
            Estado::Ok => format!("OK {:.0}ms", self.media_ms.unwrap_or(0.0)),
            Estado::Slow => format!("LENTO {:.0}ms", self.media_ms.unwrap_or(0.0)),
            Estado::Bad => format!("INESTABLE {:.0}%", self.peor_perdida as f32),
            Estado::Offline => "SIN RED".into(),
        }
    }

    /// Tooltip multilinea: detalle por proveedor + edad + ayuda de clics.
    pub fn tooltip(&self) -> String {
        let mins = self.edad() / 60;
        let mut t = format!("KOALA — {} (hace {mins} min)", self.estado.etiqueta_es());
        for p in &self.proveedores {
            let fila = if !p.al_alcance {
                format!("{:<22} SIN RESPUESTA", p.nombre)
            } else {
                let ms = p.media.unwrap_or(0.0);
                let extra = if p.etiqueta() == "OK" {
                    String::new()
                } else {
                    format!(" {}", p.etiqueta())
                };
                let perdida = if p.peor_perdida > 0 {
                    format!("  {:.0}% perd.", p.peor_perdida as f32)
                } else {
                    String::new()
                };
                format!("{:<22} {:.1}ms{}{}", p.nombre, ms, perdida, extra)
            };
            t.push('\n');
            t.push_str(&fila);
        }
        t.push_str("\n─────\nClic: medir ahora · Clic der: interfaz");
        t
    }
}
