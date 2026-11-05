//! Ejecución de pings y extracción de métricas desde la salida del comando.
//!
//! Mejoras sobre la versión original de KOALA:
//!  - Paralelismo: cada IP se mide en su propio hilo (una medición completa
//!    toma ~2 s en vez de ~30–60 s en serie).
//!  - Parsers tolerantes: funcionan con la salida de `ping` en inglés y en
//!    español, y resuelven el caso de 100 % de pérdida (sin línea de rtt).
//!  - Auditoría: `extraer_metricas` tiene tests con salidas reales como fixtures.

use std::process::Command;
use std::sync::mpsc;

/// Métricas extraídas del ping de una IP.
/// `avg`/`mdev` vienen en `None` si hubo 100 % de pérdida (no hay rtt).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Metricas {
    pub avg: Option<f32>,
    pub mdev: Option<f32>,
    pub packet_loss: u8,
}

/// Resultado del ping a un destino individual (IP o dominio).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResultadoPing {
    pub ip: String,
    pub familia: u8, // 4, 6 o 0 (auto, dominios)
    pub metricas: Option<Metricas>,
    pub indice: usize, // posición original en la lista (para reordenar)
}

/// Ejecuta un ping y extrae métricas. `None` si el comando no se pudo correr.
/// `familia`: 4 = forzar IPv4, 6 = forzar IPv6, 0 = auto (dominios).
fn ejecutar_ping(destino: &str, familia: u8) -> Option<Metricas> {
    use crate::core::config::PINGS_POR_IP;
    let mut args: Vec<String> = vec![
        "-c".into(),
        PINGS_POR_IP.to_string(), // coincide con los umbrales de verdict.rs
        "-i".into(),
        "0.3".into(), // intervalo rápido entre paquetes
        "-W".into(),
        "1".into(), // timeout por respuesta (sin -w: con deadline iputils envía 1 paquete extra)
    ];
    match familia {
        4 => args.push("-4".into()),
        6 => args.push("-6".into()),
        _ => {} // 0 = auto: sin flag, el sistema resuelve (dominios)
    }
    args.push(destino.to_string());
    let output = Command::new("ping").args(&args).output().ok()?;
    let texto = String::from_utf8_lossy(&output.stdout);
    if std::env::var("KOALA_DEBUG").is_ok() {
        eprintln!("── ping {destino} ──\n{texto}");
    }
    extraer_metricas(&texto)
}

/// Extrae las métricas de la salida textual de `ping`.
/// Acepta salidas en cualquier idioma: se apoya en los números, no en las palabras.
pub fn extraer_metricas(texto_salida: &str) -> Option<Metricas> {
    // ── 1) PÉRDIDA: primer número que aparece justo antes de un '%' ──
    let perdida = {
        let mut encontrada = None;
        'buscar: for linea in texto_salida.lines() {
            if !linea.contains('%') {
                continue;
            }
            let antes = match linea.split('%').next() {
                Some(a) => a,
                None => continue,
            };
            if let Some(tok) = antes.split_whitespace().last() {
                if let Ok(n) = tok.parse::<u8>() {
                    encontrada = Some(n);
                    break 'buscar;
                }
            }
        }
        encontrada?
    };

    // ── 2) RTT: línea con '=' y 4 números separados por '/' (min/avg/max/mdev)
    //        Ej.:  "rtt min/avg/max/mdev = 12.4/13.0/14.1/0.6 ms"
    let mut avg = None;
    let mut mdev = None;
    for linea in texto_salida.lines() {
        if !linea.contains('=') || !linea.contains('/') {
            continue;
        }
        let valores = linea.split('=').nth(1);
        let Some(valores) = valores else { continue };
        let limpios = valores.trim().trim_end_matches(" ms");
        let partes: Vec<&str> = limpios.split('/').collect();
        if partes.len() == 4 {
            if let (Ok(a), Ok(md)) = (partes[1].parse::<f32>(), partes[3].parse::<f32>()) {
                avg = Some(a);
                mdev = Some(md);
            }
            break;
        }
    }

    // Sin línea de rtt: o hubo 100 % de pérdida (resultado válido), o la salida
    // es rara (resultado inválido).
    if avg.is_none() {
        if perdida >= 100 {
            return Some(Metricas {
                avg: None,
                mdev: None,
                packet_loss: 100,
            });
        }
        return None;
    }
    if mdev.is_none() {
        return None;
    }

    Some(Metricas {
        avg,
        mdev,
        packet_loss: perdida.min(100),
    })
}

/// Mide en paralelo todos los destinos `(orden, destino, familia)` de la lista
/// (IPs o dominios). Los resultados vuelven ordenados por su `indice` original.
pub fn medir_lote(lista: Vec<(usize, String, u8)>) -> Vec<ResultadoPing> {
    let (tx, rx) = mpsc::channel::<ResultadoPing>();
    std::thread::scope(|s| {
        for (indice, ip, familia) in &lista {
            let tx = tx.clone();
            let ip = ip.clone();
            s.spawn(move || {
                let metricas = ejecutar_ping(&ip, *familia);
                let _ = tx.send(ResultadoPing {
                    ip,
                    familia: *familia,
                    metricas,
                    indice: *indice,
                });
            });
        }
        drop(tx); // en scope el cierre espera los hilos; soltar el canal no interrumpe nada
    });
    let mut resultados: Vec<ResultadoPing> = rx.into_iter().collect();
    resultados.sort_by_key(|r| r.indice);
    resultados
}

#[cfg(test)]
mod tests {
    use super::extraer_metricas;

    const SALIDA_EN: &str = "PING 8.8.8.8 (8.8.8.8) 56(84) bytes of data.\n\
        64 bytes from 8.8.8.8: icmp_seq=1 ttl=118 time=13.4 ms\n\
        64 bytes from 8.8.8.8: icmp_seq=2 ttl=118 time=12.9 ms\n\
        64 bytes from 8.8.8.8: icmp_seq=3 ttl=118 time=13.1 ms\n\
        64 bytes from 8.8.8.8: icmp_seq=4 ttl=118 time=13.6 ms\n\
        \n\
        --- 8.8.8.8 ping statistics ---\n\
        4 packets transmitted, 4 received, 0% packet loss, time 1931ms\n\
        rtt min/avg/max/mdev = 12.910/13.259/13.568/0.251 ms";

    const SALIDA_ES: &str = "PING 1.1.1.1 (1.1.1.1) 56(84) bytes de datos.\n\
        64 bytes desde 1.1.1.1: icmp_seq=1 ttl=57 tiempo=5.20 ms\n\
        64 bytes desde 1.1.1.1: icmp_seq=2 ttl=57 tiempo=6.11 ms\n\
        64 bytes desde 1.1.1.1: icmp_seq=3 ttl=57 tiempo=5.98 ms\n\
        \n\
        --- estadísticas del ping de 1.1.1.1 ---\n\
        4 paquetes transmitidos, 3 recibidos, 25% paquetes perdidos\n\
        rtt min/med/max/média = 5.203/5.764/6.108/0.403 ms";

    const SALIDA_PERDIDA_TOTAL: &str = "PING 4.2.2.2 (4.2.2.2) 56(84) bytes of data.\n\
        \n\
        --- 4.2.2.2 ping statistics ---\n\
        4 packets transmitted, 0 received, 100% packet loss, time 1906ms";

    // Salida típica del ping a un dominio: el header muestra el nombre resuelto
    // entre paréntesis; el resto es idéntico al de una IP (mismo parser).
    const SALIDA_DOMINIO: &str = "PING www.facebook.com (163.70.8.8) 56(84) bytes of data.\n\
        64 bytes from www.facebook.com (163.70.8.8): icmp_seq=1 ttl=114 time=31.2 ms\n\
        64 bytes from www.facebook.com (163.70.8.8): icmp_seq=2 ttl=114 time=30.4 ms\n\
        64 bytes from www.facebook.com (163.70.8.8): icmp_seq=3 ttl=114 time=30.9 ms\n\
        64 bytes from www.facebook.com (163.70.8.8): icmp_seq=4 ttl=114 time=30.6 ms\n\
        \n\
        --- www.facebook.com ping statistics ---\n\
        4 packets transmitted, 4 received, 0% packet loss, time 3004ms\n\
        rtt min/avg/max/mdev = 30.404/30.778/31.240/0.318 ms";

    #[test]
    fn parsea_dominio() {
        let m = extraer_metricas(SALIDA_DOMINIO).expect("debería parsear");
        assert_eq!(m.packet_loss, 0);
        assert!((m.avg.unwrap() - 30.778).abs() < 0.01);
        assert!((m.mdev.unwrap() - 0.318).abs() < 0.01);
    }

    #[test]
    fn parsea_ingles() {
        let m = extraer_metricas(SALIDA_EN).expect("debería parsear");
        assert_eq!(m.packet_loss, 0);
        assert!((m.avg.unwrap() - 13.259).abs() < 0.01);
        assert!((m.mdev.unwrap() - 0.251).abs() < 0.01);
    }

    #[test]
    fn parsea_espanol() {
        let m = extraer_metricas(SALIDA_ES).expect("debería parsear");
        assert_eq!(m.packet_loss, 25);
        assert!((m.avg.unwrap() - 5.764).abs() < 0.01);
        assert!((m.mdev.unwrap() - 0.403).abs() < 0.01);
    }

    #[test]
    fn parsea_perdida_total_sin_rtt() {
        let m = extraer_metricas(SALIDA_PERDIDA_TOTAL).expect("debería parsear");
        assert_eq!(m.packet_loss, 100);
        assert!(m.avg.is_none(), "sin rtt, avg debe ser None");
        assert!(m.mdev.is_none());
    }

    #[test]
    fn rechaza_salida_basura() {
        assert!(extraer_metricas("nada útil acá").is_none());
        assert!(extraer_metricas("").is_none());
    }
}
