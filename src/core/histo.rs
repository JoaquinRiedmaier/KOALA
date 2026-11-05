//! Historial persistente de mediciones: una línea JSON por medición (JSON Lines).
//!
//! Vive en `~/.local/state/koala/history.jsonl` y no en `~/.cache`: el caché se puede
//! borrar en cualquier momento. Solo se llama mientras se tiene el lock de `medir`,
//! así que no hay escrituras concurrentes.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::core::config::HISTORIAL_LIMITE_BYTES;
use crate::core::ipfile::hogar;
use crate::core::verdict::{etiqueta_proveedor, Estado, EstadoGlobal};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmpresaHistorial {
    pub nombre: String,
    pub media: Option<f32>,
    pub jitter: Option<f32>,
    pub peor_perdida: u8,
}

/// Proyección de `EstadoGlobal` sin las IPs individuales.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RegistroHistorial {
    pub ts: u64,
    pub estado: Estado,
    pub media_ms: Option<f32>,
    pub peor_perdida: u8,
    pub empresas: Vec<EmpresaHistorial>,
}

/// Redondea a 0,1 ms: más precisión ocupa espacio y no aporta al historial.
fn redondear(valor: Option<f32>) -> Option<f32> {
    valor.map(|v| (v * 10.0).round() / 10.0)
}

impl From<&EstadoGlobal> for RegistroHistorial {
    fn from(e: &EstadoGlobal) -> Self {
        Self {
            ts: e.ts,
            estado: e.estado,
            media_ms: redondear(e.media_ms),
            peor_perdida: e.peor_perdida,
            empresas: e
                .proveedores
                .iter()
                .map(|p| EmpresaHistorial {
                    nombre: p.nombre.clone(),
                    media: redondear(p.media),
                    jitter: redondear(p.jitter),
                    peor_perdida: p.peor_perdida,
                })
                .collect(),
        }
    }
}

impl EmpresaHistorial {
    /// Estado textual de la empresa en esa medición.
    pub fn etiqueta(&self) -> &'static str {
        etiqueta_proveedor(self.media, self.jitter, self.peor_perdida)
    }
}

impl RegistroHistorial {
    pub fn empresa(&self, nombre: &str) -> Option<&EmpresaHistorial> {
        self.empresas.iter().find(|e| e.nombre == nombre)
    }
}

pub fn ruta() -> PathBuf {
    if let Ok(p) = std::env::var("KOALA_HISTORIAL") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let base = match std::env::var("XDG_STATE_HOME") {
        Ok(d) if !d.is_empty() => PathBuf::from(d),
        _ => hogar().join(".local").join("state"),
    };
    base.join("koala").join("history.jsonl")
}

/// Agrega una medición al historial.
pub fn agregar(estado: &EstadoGlobal) -> io::Result<()> {
    agregar_en(&ruta(), &RegistroHistorial::from(estado), HISTORIAL_LIMITE_BYTES)
}

/// Todas las mediciones legibles, de la más vieja a la más nueva.
pub fn cargar() -> Vec<RegistroHistorial> {
    cargar_de(&ruta())
}

/// Nombres de empresa presentes en el historial, en orden de aparición.
pub fn nombres_empresas(registros: &[RegistroHistorial]) -> Vec<String> {
    let mut nombres: Vec<String> = Vec::new();
    for empresa in registros.iter().flat_map(|r| &r.empresas) {
        if !nombres.contains(&empresa.nombre) {
            nombres.push(empresa.nombre.clone());
        }
    }
    nombres
}

/// Mediciones de una hora del día. Con `mediciones == 0` la hora no tiene datos.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResumenHora {
    pub mediciones: u32,
    pub sin_respuesta: u32,
    /// Promedio de las mediciones con respuesta.
    pub media: Option<f32>,
    pub peor: Option<f32>,
}

/// Agrupa mediciones por hora, de la empresa indicada o global (`None`).
/// `hora_de` convierte un `ts` en la hora 0–23 del día que se está mirando.
pub fn resumen_por_hora<'a>(
    registros: impl IntoIterator<Item = &'a RegistroHistorial>,
    empresa: Option<&str>,
    hora_de: impl Fn(u64) -> usize,
) -> [ResumenHora; 24] {
    let mut horas = [ResumenHora::default(); 24];
    let mut sumas = [0.0f32; 24];
    for r in registros {
        let latencia = match empresa {
            None => r.media_ms,
            Some(nombre) => match r.empresa(nombre) {
                Some(e) => e.media,
                None => continue, // esa medición no incluía a la empresa
            },
        };
        let indice = hora_de(r.ts).min(23);
        let hora = &mut horas[indice];
        hora.mediciones += 1;
        match latencia {
            Some(ms) => {
                sumas[indice] += ms;
                hora.peor = Some(hora.peor.map_or(ms, |p| p.max(ms)));
            }
            None => hora.sin_respuesta += 1,
        }
    }
    for (hora, suma) in horas.iter_mut().zip(sumas) {
        let con_respuesta = hora.mediciones - hora.sin_respuesta;
        if con_respuesta > 0 {
            hora.media = Some(suma / con_respuesta as f32);
        }
    }
    horas
}

fn agregar_en(ruta: &Path, registro: &RegistroHistorial, limite: u64) -> io::Result<()> {
    if let Some(dir) = ruta.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut linea = serde_json::to_string(registro)?;
    linea.push('\n');
    // Un apagón pudo dejar una línea sin terminar: sin este salto, la nueva se pegaría a ella.
    if !termina_en_salto(ruta)? {
        linea.insert(0, '\n');
    }
    let mut archivo = OpenOptions::new().create(true).append(true).open(ruta)?;
    archivo.write_all(linea.as_bytes())?;
    if archivo.metadata()?.len() > limite {
        compactar(ruta, limite)?;
    }
    Ok(())
}

fn termina_en_salto(ruta: &Path) -> io::Result<bool> {
    let mut archivo = match File::open(ruta) {
        Ok(a) => a,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(e),
    };
    if archivo.metadata()?.len() == 0 {
        return Ok(true);
    }
    archivo.seek(SeekFrom::End(-1))?;
    let mut ultimo = [0u8];
    archivo.read_exact(&mut ultimo)?;
    Ok(ultimo[0] == b'\n')
}

/// Conserva el 75 % más reciente del límite, cortando en un límite de línea.
/// Escribe en un temporal y renombra: un apagón no puede dejar el historial a medias.
fn compactar(ruta: &Path, limite: u64) -> io::Result<()> {
    let datos = fs::read(ruta)?;
    let conservar = ((limite / 4 * 3) as usize).min(datos.len());
    let corte = datos.len() - conservar;
    let inicio = datos[corte..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(datos.len(), |i| corte + i + 1);
    let temporal = ruta.with_extension("jsonl.tmp");
    fs::write(&temporal, &datos[inicio..])?;
    fs::rename(&temporal, ruta)
}

/// Se lee como bytes: con `read_to_string`, un byte inválido descartaría todo el archivo.
fn cargar_de(ruta: &Path) -> Vec<RegistroHistorial> {
    let Ok(datos) = fs::read(ruta) else {
        return Vec::new();
    };
    String::from_utf8_lossy(&datos)
        .lines()
        .filter_map(|linea| serde_json::from_str(linea).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::verdict::Proveedor;
    use std::sync::atomic::{AtomicU32, Ordering};

    static SIGUIENTE: AtomicU32 = AtomicU32::new(0);

    /// Archivo de prueba único, en un directorio que todavía no existe.
    struct Temporal(PathBuf);

    impl Temporal {
        fn nuevo() -> Self {
            let id = SIGUIENTE.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("koala-histo-{}-{id}", std::process::id()));
            Self(dir.join("history.jsonl"))
        }
    }

    impl Drop for Temporal {
        fn drop(&mut self) {
            if let Some(dir) = self.0.parent() {
                let _ = fs::remove_dir_all(dir);
            }
        }
    }

    fn registro(ts: u64) -> RegistroHistorial {
        RegistroHistorial {
            ts,
            estado: Estado::Ok,
            media_ms: Some(20.0),
            peor_perdida: 0,
            empresas: vec![EmpresaHistorial {
                nombre: "GOOGLE".into(),
                media: Some(20.0),
                jitter: Some(1.0),
                peor_perdida: 0,
            }],
        }
    }

    fn marcas(ruta: &Path) -> Vec<u64> {
        cargar_de(ruta).iter().map(|r| r.ts).collect()
    }

    #[test]
    fn crea_el_directorio_y_agrega_al_final() {
        let t = Temporal::nuevo();
        agregar_en(&t.0, &registro(1), 1_000_000).unwrap();
        agregar_en(&t.0, &registro(2), 1_000_000).unwrap();
        assert_eq!(marcas(&t.0), [1, 2]);
    }

    #[test]
    fn archivo_inexistente_es_historial_vacio() {
        let t = Temporal::nuevo();
        assert!(cargar_de(&t.0).is_empty());
    }

    #[test]
    fn compacta_al_superar_el_limite_y_conserva_lo_reciente() {
        let t = Temporal::nuevo();
        let limite = 2_000;
        for ts in 0..200 {
            agregar_en(&t.0, &registro(ts), limite).unwrap();
        }
        assert!(fs::metadata(&t.0).unwrap().len() <= limite);
        let ts = marcas(&t.0);
        assert_eq!(ts.last(), Some(&199));
        // sin huecos ni registros partidos: lo que queda es una cola continua
        assert!(ts.windows(2).all(|par| par[1] == par[0] + 1));
        assert!(ts.len() > 1);
        assert!(!t.0.with_extension("jsonl.tmp").exists());
    }

    #[test]
    fn ignora_lineas_corruptas_y_bytes_invalidos() {
        let t = Temporal::nuevo();
        agregar_en(&t.0, &registro(1), 1_000_000).unwrap();
        let mut archivo = OpenOptions::new().append(true).open(&t.0).unwrap();
        archivo.write_all(b"{\"ts\":2,\"esta\n\xff\xfe\n").unwrap();
        drop(archivo);
        agregar_en(&t.0, &registro(3), 1_000_000).unwrap();
        assert_eq!(marcas(&t.0), [1, 3]);
    }

    #[test]
    fn una_linea_sin_terminar_no_arruina_la_siguiente() {
        let t = Temporal::nuevo();
        agregar_en(&t.0, &registro(1), 1_000_000).unwrap();
        let mut archivo = OpenOptions::new().append(true).open(&t.0).unwrap();
        archivo.write_all(b"{\"ts\":2,\"estado").unwrap();
        drop(archivo);
        agregar_en(&t.0, &registro(3), 1_000_000).unwrap();
        assert_eq!(marcas(&t.0), [1, 3]);
    }

    #[test]
    fn la_proyeccion_descarta_ips_y_conserva_el_estado_de_cada_empresa() {
        let proveedor = |nombre: &str, media, jitter, peor_perdida| Proveedor {
            nombre: String::from(nombre),
            ips: Vec::new(),
            media,
            jitter,
            peor_perdida,
            al_alcance: media.is_some(),
        };
        let proveedores = vec![
            proveedor("OK", Some(25.84), Some(1.0), 0),
            proveedor("LENTO", Some(80.0), Some(1.0), 0),
            proveedor("INESTABLE", Some(20.0), Some(1.0), 50),
            proveedor("CAIDO", None, None, 100),
        ];
        let esperadas: Vec<&str> = proveedores.iter().map(Proveedor::etiqueta).collect();
        let estado = EstadoGlobal {
            ts: 7,
            estado: Estado::Bad,
            media_ms: Some(36.2195),
            peor_perdida: 100,
            proveedores,
        };

        let r = RegistroHistorial::from(&estado);

        assert_eq!(r.ts, 7);
        assert_eq!(r.media_ms, Some(36.2));
        assert_eq!(r.empresa("OK").unwrap().media, Some(25.8));
        assert!(r.empresa("OTRA").is_none());
        let etiquetas: Vec<&str> = r.empresas.iter().map(EmpresaHistorial::etiqueta).collect();
        assert_eq!(etiquetas, esperadas);
    }

    #[test]
    fn un_offline_se_guarda_y_se_vuelve_a_leer() {
        let t = Temporal::nuevo();
        let mut r = registro(1);
        r.estado = Estado::Offline;
        r.media_ms = None;
        r.empresas[0].media = None;
        r.empresas[0].jitter = None;
        r.empresas[0].peor_perdida = 100;
        agregar_en(&t.0, &r, 1_000_000).unwrap();
        let leido = &cargar_de(&t.0)[0];
        assert_eq!(leido.estado, Estado::Offline);
        assert_eq!(leido.media_ms, None);
        assert_eq!(leido.empresas[0].etiqueta(), "SIN RESPUESTA");
    }

    /// El `ts` de estos registros es directamente la hora.
    fn por_hora(registros: &[RegistroHistorial], empresa: Option<&str>) -> [ResumenHora; 24] {
        resumen_por_hora(registros, empresa, |ts| ts as usize)
    }

    fn con_latencia(hora: u64, ms: Option<f32>) -> RegistroHistorial {
        let mut r = registro(hora);
        r.media_ms = ms;
        r.empresas[0].media = ms;
        r
    }

    #[test]
    fn promedia_las_mediciones_de_cada_hora() {
        let horas = por_hora(&[con_latencia(19, Some(30.0)), con_latencia(19, Some(50.0))], None);
        assert_eq!(horas[19].mediciones, 2);
        assert_eq!(horas[19].media, Some(40.0));
        assert_eq!(horas[19].peor, Some(50.0));
    }

    #[test]
    fn una_hora_sin_mediciones_queda_vacia() {
        let horas = por_hora(&[con_latencia(19, Some(30.0))], None);
        assert_eq!(horas[18], ResumenHora::default());
    }

    #[test]
    fn un_corte_cuenta_como_medicion_pero_no_baja_el_promedio() {
        let horas = por_hora(&[con_latencia(8, Some(30.0)), con_latencia(8, None)], None);
        assert_eq!(horas[8].mediciones, 2);
        assert_eq!(horas[8].sin_respuesta, 1);
        assert_eq!(horas[8].media, Some(30.0));
    }

    #[test]
    fn una_hora_toda_sin_respuesta_no_tiene_latencia() {
        let horas = por_hora(&[con_latencia(8, None)], None);
        assert_eq!((horas[8].mediciones, horas[8].sin_respuesta), (1, 1));
        assert_eq!(horas[8].media, None);
    }

    #[test]
    fn el_resumen_por_empresa_usa_sus_datos_y_salta_las_mediciones_sin_ella() {
        let mut r = con_latencia(10, Some(99.0));
        r.empresas[0].media = Some(20.0);
        let horas = por_hora(&[r.clone()], Some("GOOGLE"));
        assert_eq!(horas[10].media, Some(20.0));
        assert_eq!(por_hora(&[r], Some("FACEBOOK"))[10], ResumenHora::default());
    }

    #[test]
    fn nombres_sin_repetir_en_orden_de_aparicion() {
        let mut viejo = registro(1);
        viejo.empresas[0].nombre = "CLOUDFLARE".into();
        let registros = [viejo, registro(2), registro(3)];
        assert_eq!(nombres_empresas(&registros), ["CLOUDFLARE", "GOOGLE"]);
    }
}
