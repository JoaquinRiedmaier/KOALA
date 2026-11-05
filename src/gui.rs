//! Interfaz gráfica (`koala gui`): ventana liviana (egui/eframe, nativa Wayland).
//!
//! Mide automáticamente al abrirse, muestra el veredicto con colores por letra
//! (igual que el widget: sin emojis ni leds) y permite re-medir con un botón.

use eframe::egui::{self, Color32, RichText};
use std::sync::mpsc;

use crate::core::cache;
use crate::core::verdict::{Estado, EstadoGlobal, Proveedor};

const COL_TEXTO: Color32 = Color32::from_rgb(0xcd, 0xd6, 0xf4);
const COL_SAP: Color32 = Color32::from_rgb(0x11, 0x11, 0x1b); // fondo tipo Catppuccin Mocha
const COL_IDLE: Color32 = Color32::from_rgb(0x7f, 0x84, 0x9c);
const COL_OK: Color32 = Color32::from_rgb(0xa6, 0xe3, 0xa1);
const COL_WARN: Color32 = Color32::from_rgb(0xf9, 0xe2, 0xaf);
const COL_BAD: Color32 = Color32::from_rgb(0xfa, 0xb3, 0x87);
const COL_OFF: Color32 = Color32::from_rgb(0xf3, 0x8b, 0xa8);

pub fn ejecutar() {
    let opciones = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("kOala — medidor de internet")
            .with_inner_size([430.0, 470.0])
            .with_resizable(true),
        ..Default::default()
    };
    let app = KoalaApp::nueva();
    eframe::run_native("KOALA", opciones, Box::new(|_cc| Ok(Box::new(app))))
        .expect("error de ventana");
}

pub struct KoalaApp {
    estado: Option<EstadoGlobal>,
    canal: Option<mpsc::Receiver<EstadoGlobal>>,
    midiendo: bool,
}

impl KoalaApp {
    pub fn nueva() -> Self {
        // arranca midiendo de una (besides leer el caché previo para pintar enseguida)
        let estado = cache::cargar();
        let mut app = Self {
            estado,
            canal: None,
            midiendo: false,
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

    fn color_de_proveedor(&self, p: &Proveedor) -> Color32 {
        match p.etiqueta() {
            "OK" => COL_OK,
            "LENTO" => COL_WARN,
            "INESTABLE" => COL_BAD,
            _ => COL_OFF,
        }
    }

    fn color_global(&self, e: &Estado) -> Color32 {
        match e {
            Estado::Ok => COL_OK,
            Estado::Slow => COL_WARN,
            Estado::Bad => COL_BAD,
            Estado::Offline => COL_OFF,
        }
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
                ui.add_space(4.0);

                // ── Veredicto grande ──
                match &self.estado {
                    Some(e) => {
                        let color = self.color_global(&e.estado);
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
                            ui.label(
                                RichText::new(Self::edad_humana(e.edad())).color(COL_IDLE).weak(),
                            );
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
                                        let color = self.color_de_proveedor(p);
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
            });
    }
}
