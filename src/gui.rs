//! Interfaz gráfica (`koala gui`): ventana liviana (egui/eframe, nativa Wayland).
//!
//! Mide automáticamente al abrirse, muestra el veredicto con colores por letra
//! (igual que el widget: sin emojis ni leds) y permite re-medir con un botón.
//! La vista HISTORIAL muestra las mediciones guardadas, con filtro por empresa.

use chrono::{DateTime, Local, NaiveDate};
use eframe::egui::{self, Color32, RichText};
use std::sync::mpsc;

use crate::core::cache;
use crate::core::histo::{self, RegistroHistorial};
use crate::core::verdict::{Estado, EstadoGlobal};

const COL_TEXTO: Color32 = Color32::from_rgb(0xcd, 0xd6, 0xf4);
const COL_SAP: Color32 = Color32::from_rgb(0x11, 0x11, 0x1b); // fondo tipo Catppuccin Mocha
const COL_IDLE: Color32 = Color32::from_rgb(0x7f, 0x84, 0x9c);
const COL_OK: Color32 = Color32::from_rgb(0xa6, 0xe3, 0xa1);
const COL_WARN: Color32 = Color32::from_rgb(0xf9, 0xe2, 0xaf);
const COL_BAD: Color32 = Color32::from_rgb(0xfa, 0xb3, 0x87);
const COL_OFF: Color32 = Color32::from_rgb(0xf3, 0x8b, 0xa8);

const TODAS: &str = "TODAS";
const ALTO_FILA: f32 = 20.0;
const COLUMNAS_TODAS: [(&str, f32); 4] = [
    ("HORA", 90.0),
    ("ESTADO", 110.0),
    ("MEDIA", 80.0),
    ("PÉRDIDA", 70.0),
];
const COLUMNAS_EMPRESA: [(&str, f32); 5] = [
    ("HORA", 90.0),
    ("PING", 70.0),
    ("JITTER", 70.0),
    ("PÉRDIDA", 70.0),
    ("ESTADO", 110.0),
];

pub fn ejecutar() {
    let opciones = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("kOala — medidor de internet")
            .with_inner_size([480.0, 520.0])
            .with_resizable(true),
        ..Default::default()
    };
    let app = KoalaApp::nueva();
    eframe::run_native("KOALA", opciones, Box::new(|_cc| Ok(Box::new(app))))
        .expect("error de ventana");
}

#[derive(Clone, Copy, PartialEq)]
enum Vista {
    Veredicto,
    Historial,
}

pub struct KoalaApp {
    estado: Option<EstadoGlobal>,
    canal: Option<mpsc::Receiver<EstadoGlobal>>,
    midiendo: bool,
    vista: Vista,
    /// Mediciones guardadas, la más nueva primero. `None` = hay que leer el archivo.
    historial: Option<Vec<RegistroHistorial>>,
    empresas: Vec<String>,
    /// Empresa elegida en el filtro. `None` = todas.
    filtro: Option<String>,
}

fn color_de_etiqueta(etiqueta: &str) -> Color32 {
    match etiqueta {
        "OK" => COL_OK,
        "LENTO" => COL_WARN,
        "INESTABLE" => COL_BAD,
        _ => COL_OFF,
    }
}

fn color_global(e: &Estado) -> Color32 {
    match e {
        Estado::Ok => COL_OK,
        Estado::Slow => COL_WARN,
        Estado::Bad => COL_BAD,
        Estado::Offline => COL_OFF,
    }
}

/// `14:32` si la medición es de hoy; si no, `05/11 14:32`.
fn hora_local(ts: u64, hoy: NaiveDate) -> String {
    let Some(momento) = DateTime::from_timestamp(ts as i64, 0) else {
        return "—".into();
    };
    let momento = momento.with_timezone(&Local);
    let formato = if momento.date_naive() == hoy { "%H:%M" } else { "%d/%m %H:%M" };
    momento.format(formato).to_string()
}

fn milisegundos(valor: Option<f32>) -> String {
    valor.map_or("—".into(), |v| format!("{v:.1} ms"))
}

/// Celdas de una fila del historial global.
fn celdas_todas(r: &RegistroHistorial, hoy: NaiveDate) -> Vec<(String, Color32)> {
    vec![
        (hora_local(r.ts, hoy), COL_TEXTO),
        (r.estado.etiqueta_es().into(), color_global(&r.estado)),
        (milisegundos(r.media_ms), COL_TEXTO),
        (format!("{}%", r.peor_perdida), COL_TEXTO),
    ]
}

/// Celdas de una fila filtrada por empresa. Si esa medición no la incluía, van `—`.
fn celdas_empresa(r: &RegistroHistorial, nombre: &str, hoy: NaiveDate) -> Vec<(String, Color32)> {
    let hora = (hora_local(r.ts, hoy), COL_TEXTO);
    let Some(e) = r.empresa(nombre) else {
        let vacia = ("—".to_owned(), COL_IDLE);
        return vec![hora, vacia.clone(), vacia.clone(), vacia.clone(), vacia];
    };
    vec![
        hora,
        (milisegundos(e.media), COL_TEXTO),
        (milisegundos(e.jitter), COL_TEXTO),
        (format!("{}%", e.peor_perdida), COL_TEXTO),
        (e.etiqueta().into(), color_de_etiqueta(e.etiqueta())),
    ]
}

fn fila(ui: &mut egui::Ui, celdas: impl IntoIterator<Item = (f32, RichText)>) {
    ui.horizontal(|ui| {
        for (ancho, texto) in celdas {
            ui.allocate_ui_with_layout(
                egui::vec2(ancho, ALTO_FILA),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(ancho);
                    ui.add(egui::Label::new(texto).truncate());
                },
            );
        }
    });
}

fn edad_humana(edad: u64) -> String {
    if edad < 60 {
        format!("hace {edad} s")
    } else if edad < 3600 {
        format!("hace {} min", edad / 60)
    } else {
        format!("hace {} h {} min", edad / 3600, (edad % 3600) / 60)
    }
}

impl KoalaApp {
    fn vacia() -> Self {
        Self {
            estado: None,
            canal: None,
            midiendo: false,
            vista: Vista::Veredicto,
            historial: None,
            empresas: Vec::new(),
            filtro: None,
        }
    }

    pub fn nueva() -> Self {
        // arranca midiendo de una (besides leer el caché previo para pintar enseguida)
        let mut app = Self {
            estado: cache::cargar(),
            ..Self::vacia()
        };
        app.lanzar_medicion();
        app
    }

    fn lanzar_medicion(&mut self) {
        if self.midiendo {
            return;
        }
        self.midiendo = true;
        let (tx, rx) = mpsc::channel();
        self.canal = Some(rx);
        let _ = std::thread::spawn(move || {
            let e = crate::app::medir();
            let _ = tx.send(e.expect("medición"));
        });
    }

    fn recargar_historial(&mut self) {
        let mut registros = histo::cargar();
        registros.reverse();
        self.empresas = histo::nombres_empresas(&registros);
        self.historial = Some(registros);
    }

    fn pintar_veredicto(&mut self, ui: &mut egui::Ui) {
        // ── Veredicto grande ──
        match &self.estado {
            Some(e) => {
                let color = color_global(&e.estado);
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    ui.label(
                        RichText::new(format!("CONEXIÓN: {}", e.estado.etiqueta_es()))
                            .size(24.0)
                            .color(color),
                    );
                    ui.add_space(2.0);
                    let detalle = match e.media_ms {
                        Some(m) => format!("latencia media {:.0} ms · peor pérdida {:.0}%", m, e.peor_perdida as f32),
                        None => format!("ninguna IP respondió · peor pérdida {:.0}%", e.peor_perdida as f32),
                    };
                    ui.label(RichText::new(detalle).color(COL_TEXTO));
                    ui.label(RichText::new(edad_humana(e.edad())).color(COL_IDLE).weak());
                });
            }
            None => {
                ui.label(
                    RichText::new(if self.midiendo { "MIDIENDO…" } else { "Sin datos" })
                        .size(24.0)
                        .color(COL_IDLE),
                );
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        // ── Tabla por proveedor ──
        if let Some(e) = &self.estado {
            // scroll vertical: con 11+ filas ya no entra en la ventana;
            // deja ~50 px para el botón inferior
            let max_altura = (ui.available_height() - 50.0).max(120.0);
            egui::ScrollArea::vertical()
                .id_salt("tabla_proveedores")
                .auto_shrink([false, false])
                .max_height(max_altura)
                .show(ui, |ui| {
                    egui::Grid::new("proveedores")
                        .num_columns(4)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("PROVEEDOR").color(COL_IDLE).weak());
                            ui.label(RichText::new("PING").color(COL_IDLE).weak());
                            ui.label(RichText::new("PERDIDA").color(COL_IDLE).weak());
                            ui.label(RichText::new("ESTADO").color(COL_IDLE).weak());
                            ui.end_row();
                            for p in &e.proveedores {
                                let color = color_de_etiqueta(p.etiqueta());
                                ui.label(RichText::new(&p.nombre).color(COL_TEXTO));
                                ui.label(RichText::new(match p.media {
                                    Some(m) => format!("{:.1} ms", m),
                                    None => "—".into(),
                                }).color(COL_TEXTO));
                                ui.label(RichText::new(format!("{:.0}%", p.peor_perdida as f32)).color(COL_TEXTO));
                                ui.label(RichText::new(p.etiqueta()).color(color));
                                ui.end_row();
                            }
                        });
                });
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(8.0);
            let (btn_texto, btn_habilitado) = if self.midiendo {
                ("MIDIENDO…", false)
            } else {
                ("Re-medir", true)
            };
            if ui.add_enabled(btn_habilitado, egui::Button::new(btn_texto)).clicked() {
                self.lanzar_medicion();
            }
            if self.midiendo {
                ui.spinner();
            }
        });
    }

    fn pintar_historial(&mut self, ui: &mut egui::Ui) {
        if self.historial.is_none() {
            self.recargar_historial();
        }

        let mut actualizar = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("EMPRESA").color(COL_IDLE));
            egui::ComboBox::from_id_salt("filtro_empresa")
                .selected_text(self.filtro.as_deref().unwrap_or(TODAS))
                .width(190.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filtro, None, TODAS);
                    for nombre in &self.empresas {
                        ui.selectable_value(&mut self.filtro, Some(nombre.clone()), nombre);
                    }
                });
            actualizar = ui.button("ACTUALIZAR").clicked();
        });
        if actualizar {
            self.recargar_historial();
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        let Some(registros) = &self.historial else {
            return;
        };
        if registros.is_empty() {
            ui.label(RichText::new("Todavía no hay mediciones guardadas.").color(COL_IDLE));
            return;
        }

        let columnas: &[(&str, f32)] = match self.filtro {
            Some(_) => &COLUMNAS_EMPRESA,
            None => &COLUMNAS_TODAS,
        };
        fila(ui, columnas.iter().map(|(titulo, ancho)| {
            (*ancho, RichText::new(*titulo).color(COL_IDLE).weak())
        }));

        let hoy = Local::now().date_naive();
        // show_rows dibuja solo las filas visibles: el historial completo no pesa
        egui::ScrollArea::vertical()
            .id_salt("tabla_historial")
            .auto_shrink([false, false])
            .show_rows(ui, ALTO_FILA, registros.len(), |ui, visibles| {
                for r in &registros[visibles] {
                    let celdas = match &self.filtro {
                        Some(nombre) => celdas_empresa(r, nombre, hoy),
                        None => celdas_todas(r, hoy),
                    };
                    fila(ui, columnas.iter().zip(celdas).map(|((_, ancho), (texto, color))| {
                        (*ancho, RichText::new(texto).color(color))
                    }));
                }
            });
    }
}

impl eframe::App for KoalaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // recoger mediciones terminadas
        if let Some(rx) = &self.canal {
            match rx.try_recv() {
                Ok(e) => {
                    self.estado = Some(e);
                    self.midiendo = false;
                    self.canal = None;
                    self.historial = None; // la medición agregó una línea: releer al mostrarlo
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.midiendo = false;
                    self.canal = None;
                }
                Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(200)),
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(COL_SAP).inner_margin(16.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.vista, Vista::Veredicto, "VEREDICTO");
                    ui.selectable_value(&mut self.vista, Vista::Historial, "HISTORIAL");
                });
                ui.add_space(8.0);

                match self.vista {
                    Vista::Veredicto => self.pintar_veredicto(ui),
                    Vista::Historial => self.pintar_historial(ui),
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::histo::EmpresaHistorial;

    fn registro(ts: u64) -> RegistroHistorial {
        RegistroHistorial {
            ts,
            estado: Estado::Slow,
            media_ms: Some(62.5),
            peor_perdida: 25,
            empresas: vec![EmpresaHistorial {
                nombre: "GOOGLE".into(),
                media: Some(62.5),
                jitter: Some(1.2),
                peor_perdida: 25,
            }],
        }
    }

    fn textos(celdas: Vec<(String, Color32)>) -> Vec<String> {
        celdas.into_iter().map(|(texto, _)| texto).collect()
    }

    fn hoy() -> NaiveDate {
        Local::now().date_naive()
    }

    #[test]
    fn la_hora_lleva_fecha_solo_si_no_es_de_hoy() {
        let ahora = Local::now().timestamp() as u64;
        assert_eq!(hora_local(ahora, hoy()).len(), "14:32".len());
        let otro_dia = hoy().succ_opt().unwrap();
        assert_eq!(hora_local(ahora, otro_dia).len(), "05/11 14:32".len());
    }

    #[test]
    fn la_fila_global_muestra_el_estado_general() {
        let celdas = celdas_todas(&registro(0), hoy());
        assert_eq!(celdas[1].1, COL_WARN);
        assert_eq!(textos(celdas)[1..], ["LENTO", "62.5 ms", "25%"]);
    }

    #[test]
    fn la_fila_de_una_empresa_muestra_sus_datos() {
        let celdas = celdas_empresa(&registro(0), "GOOGLE", hoy());
        assert_eq!(celdas[4].1, COL_WARN);
        assert_eq!(textos(celdas)[1..], ["62.5 ms", "1.2 ms", "25%", "LENTO"]);
    }

    #[test]
    fn la_fila_de_una_empresa_ausente_queda_con_guiones() {
        let celdas = textos(celdas_empresa(&registro(0), "FACEBOOK", hoy()));
        assert_eq!(celdas[1..], ["—", "—", "—", "—"]);
    }

    #[test]
    fn un_offline_no_muestra_latencias() {
        let mut r = registro(0);
        r.estado = Estado::Offline;
        r.media_ms = None;
        assert_eq!(textos(celdas_todas(&r, hoy()))[1..], ["SIN RED", "—", "25%"]);
    }

    /// Dibuja la vista sin abrir una ventana, con las variantes de filtro y sin datos.
    #[test]
    fn la_vista_historial_se_dibuja_en_todos_los_casos() {
        let ctx = egui::Context::default();
        let registros: Vec<_> = (0..5_000).map(registro).collect();
        for (historial, filtro) in [
            (registros.clone(), None),
            (registros.clone(), Some("GOOGLE")),
            (registros, Some("NO-ESTA")),
            (Vec::new(), None),
        ] {
            let mut app = KoalaApp {
                empresas: histo::nombres_empresas(&historial),
                historial: Some(historial),
                filtro: filtro.map(String::from),
                ..KoalaApp::vacia()
            };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.pintar_historial(ui));
            });
        }
    }
}
