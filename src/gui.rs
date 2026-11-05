//! Interfaz gráfica (`koala gui`): ventana liviana (egui/eframe, nativa Wayland).
//!
//! Mide automáticamente al abrirse, muestra el veredicto con colores por letra
//! (igual que el widget: sin emojis ni leds) y permite re-medir con un botón.
//! La vista HISTORIAL muestra las mediciones guardadas de un día: un gráfico por hora
//! y su tabla, con filtro por empresa.

use chrono::{DateTime, Local, NaiveDate, Timelike};
use eframe::egui::{self, pos2, vec2, Color32, Rect, RichText, Stroke};
use std::ops::Range;
use std::sync::mpsc;

use crate::core::cache;
use crate::core::histo::{self, RegistroHistorial, ResumenHora};
use crate::core::verdict::{Estado, EstadoGlobal, LIMITE_MS, LIMITE_MS_ALTO};

const COL_TEXTO: Color32 = Color32::from_rgb(0xcd, 0xd6, 0xf4);
const COL_SAP: Color32 = Color32::from_rgb(0x11, 0x11, 0x1b); // fondo tipo Catppuccin Mocha
const COL_IDLE: Color32 = Color32::from_rgb(0x7f, 0x84, 0x9c);
const COL_OK: Color32 = Color32::from_rgb(0xa6, 0xe3, 0xa1);
const COL_WARN: Color32 = Color32::from_rgb(0xf9, 0xe2, 0xaf);
const COL_BAD: Color32 = Color32::from_rgb(0xfa, 0xb3, 0x87);
const COL_OFF: Color32 = Color32::from_rgb(0xf3, 0x8b, 0xa8);

const TODAS: &str = "TODAS";
const ALTO_FILA: f32 = 20.0;
const ALTO_GRAFICO: f32 = 180.0;
const MARGEN_EJE_Y: f32 = 48.0;
const MARGEN_EJE_X: f32 = 18.0;
/// Opacidad del área: deja ver la grilla y hace resaltar el borde superior.
const OPACIDAD_AREA: f32 = 0.8;
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
            .with_inner_size([480.0, 640.0])
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
    /// Días con mediciones, el más nuevo primero.
    dias: Vec<DiaHistorial>,
    /// Índice en `dias` del día que se está mirando.
    dia: usize,
    empresas: Vec<String>,
    /// Empresa elegida en el filtro. `None` = todas.
    filtro: Option<String>,
}

/// Mediciones de un día: `filas` es su rango dentro del historial.
struct DiaHistorial {
    fecha: NaiveDate,
    filas: Range<usize>,
}

fn momento_local(ts: u64) -> DateTime<Local> {
    DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Local)
}

/// Agrupa por día local. `registros` tiene que venir de más nuevo a más viejo.
fn agrupar_por_dia(registros: &[RegistroHistorial]) -> Vec<DiaHistorial> {
    let mut dias: Vec<DiaHistorial> = Vec::new();
    for (i, r) in registros.iter().enumerate() {
        let fecha = momento_local(r.ts).date_naive();
        match dias.last_mut() {
            Some(dia) if dia.fecha == fecha => dia.filas.end = i + 1,
            _ => dias.push(DiaHistorial { fecha, filas: i..i + 1 }),
        }
    }
    dias
}

/// Verde hasta el límite del veredicto, amarillo hasta el doble y rojo por encima.
fn color_latencia(ms: f32) -> Color32 {
    if ms <= LIMITE_MS {
        COL_OK
    } else if ms <= LIMITE_MS_ALTO {
        COL_WARN
    } else {
        COL_OFF
    }
}

/// Tope del eje vertical. Nunca baja de 100 ms: así un valor chico se ve chico.
fn tope_eje(horas: &[ResumenHora; 24]) -> f32 {
    let mayor = horas.iter().filter_map(|h| h.media).fold(0.0, f32::max);
    [100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0]
        .into_iter()
        .find(|tope| mayor <= *tope)
        .unwrap_or(mayor)
}

fn hora_en_x(x: f32, inicio: f32, ancho: f32) -> usize {
    (((x - inicio) / ancho * 24.0).floor().max(0.0) as usize).min(23)
}

fn texto_hora(hora: usize, r: &ResumenHora) -> String {
    let rango = format!("{hora:02}:00 a {:02}:00", hora + 1);
    if r.mediciones == 0 {
        return format!("{rango}\nSin mediciones");
    }
    let cantidad = match r.mediciones {
        1 => "1 medición".to_owned(),
        n => format!("{n} mediciones"),
    };
    let latencia = match (r.media, r.peor) {
        (Some(media), Some(peor)) => format!("media {media:.1} ms · peor {peor:.1} ms"),
        _ => "Sin respuesta".to_owned(),
    };
    match (r.media, r.sin_respuesta) {
        (Some(_), n) if n > 0 => format!("{rango}\n{latencia}\n{cantidad} · {n} sin respuesta"),
        _ => format!("{rango}\n{latencia}\n{cantidad}"),
    }
}

fn texto_eje(pintor: &egui::Painter, posicion: egui::Pos2, alineacion: egui::Align2, texto: &str) {
    pintor.text(posicion, alineacion, texto, egui::FontId::proportional(11.0), COL_IDLE);
}

/// Área escalonada por hora. Una hora sin mediciones queda vacía: unir las vecinas
/// mostraría datos que nadie midió.
fn pintar_grafico(ui: &mut egui::Ui, horas: &[ResumenHora; 24]) {
    let tamano = vec2(ui.available_width(), ALTO_GRAFICO);
    let (rect, respuesta) = ui.allocate_exact_size(tamano, egui::Sense::hover());
    let pintor = ui.painter_at(rect);
    let plot = Rect::from_min_max(
        pos2(rect.left() + MARGEN_EJE_Y, rect.top() + 8.0),
        pos2(rect.right() - 6.0, rect.bottom() - MARGEN_EJE_X),
    );
    let tope = tope_eje(horas);
    let y_de = |ms: f32| plot.bottom() - ms.min(tope) / tope * plot.height();
    let ancho_hora = plot.width() / 24.0;
    let x_de = |hora: usize| plot.left() + hora as f32 * ancho_hora;
    let columna = |hora: usize| {
        Rect::from_x_y_ranges(x_de(hora)..=x_de(hora + 1), plot.y_range())
    };

    // ── Ejes: los valores marcan los cambios de color ──
    let mut ultimo_y = f32::INFINITY;
    for ms in [LIMITE_MS, LIMITE_MS_ALTO, tope] {
        let y = y_de(ms);
        if ultimo_y - y < 14.0 {
            continue; // con un tope alto, 50 y 100 se pisarían
        }
        ultimo_y = y;
        pintor.hline(plot.x_range(), y, Stroke::new(1.0_f32, COL_IDLE.gamma_multiply(0.25)));
        texto_eje(&pintor, pos2(plot.left() - 6.0, y), egui::Align2::RIGHT_CENTER, &format!("{ms:.0} ms"));
    }
    pintor.hline(plot.x_range(), plot.bottom(), Stroke::new(1.0_f32, COL_IDLE.gamma_multiply(0.6)));
    texto_eje(&pintor, pos2(plot.left() - 6.0, plot.bottom()), egui::Align2::RIGHT_CENTER, "0");
    for hora in (0..24).step_by(3) {
        let posicion = pos2(x_de(hora), plot.bottom() + 3.0);
        texto_eje(&pintor, posicion, egui::Align2::CENTER_TOP, &format!("{hora:02}"));
    }

    // ── Área ──
    for (hora, r) in horas.iter().enumerate() {
        let col = columna(hora);
        if let Some(ms) = r.media {
            let color = color_latencia(ms);
            let cima = y_de(ms).min(plot.bottom() - 2.0);
            let area = Rect::from_x_y_ranges(col.x_range(), cima..=plot.bottom());
            pintor.rect_filled(area, 0.0, color.gamma_multiply(OPACIDAD_AREA));
            pintor.hline(col.x_range(), cima, Stroke::new(2.0_f32, color));
        } else if r.mediciones > 0 {
            // un corte de toda la hora ocupa la columna: no tiene que parecer una hora vacía
            pintor.rect_filled(col, 0.0, COL_OFF.gamma_multiply(0.25));
        }
        if r.sin_respuesta > 0 {
            let marca = Rect::from_x_y_ranges(col.x_range(), plot.bottom() - 3.0..=plot.bottom());
            pintor.rect_filled(marca, 0.0, COL_OFF);
        }
    }

    // ── Detalle al pasar el mouse ──
    if let Some(posicion) = respuesta.hover_pos() {
        if plot.x_range().contains(posicion.x) {
            let hora = hora_en_x(posicion.x, plot.left(), plot.width());
            pintor.rect_filled(columna(hora), 0.0, Color32::from_white_alpha(12));
            respuesta.on_hover_text_at_pointer(texto_hora(hora, &horas[hora]));
        }
    }
}

fn pintar_leyenda(ui: &mut egui::Ui) {
    let hasta = format!("hasta {LIMITE_MS:.0} ms");
    let medio = format!("{LIMITE_MS:.0} a {LIMITE_MS_ALTO:.0} ms");
    let alto = format!("más de {LIMITE_MS_ALTO:.0} ms");
    ui.horizontal(|ui| {
        for (color, texto) in [
            (COL_OK, hasta.as_str()),
            (COL_WARN, medio.as_str()),
            (COL_OFF, alto.as_str()),
            (COL_OFF, "sin respuesta"),
        ] {
            let (muestra, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
            if texto == "sin respuesta" {
                // igual que en el gráfico: una franja al pie de la hora
                let franja = Rect::from_x_y_ranges(muestra.x_range(), muestra.bottom() - 3.0..=muestra.bottom());
                ui.painter().rect_filled(franja, 0.0, color);
            } else {
                ui.painter().rect_filled(muestra, 2.0, color.gamma_multiply(OPACIDAD_AREA));
            }
            ui.label(RichText::new(texto).size(11.0).color(COL_IDLE));
            ui.add_space(4.0);
        }
    });
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

/// `14:32` si la medición es del día indicado; si no, `05/11 14:32`.
fn hora_local(ts: u64, dia: NaiveDate) -> String {
    let momento = momento_local(ts);
    let formato = if momento.date_naive() == dia { "%H:%M" } else { "%d/%m %H:%M" };
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
            dias: Vec::new(),
            dia: 0,
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
        self.asignar_registros(histo::cargar());
    }

    /// Ordena las mediciones y conserva el día que se estaba mirando, si sigue existiendo.
    fn asignar_registros(&mut self, mut registros: Vec<RegistroHistorial>) {
        registros.sort_by_key(|r| std::cmp::Reverse(r.ts));
        let fecha = self.dias.get(self.dia).map(|d| d.fecha);
        self.dias = agrupar_por_dia(&registros);
        self.dia = self.dias.iter().position(|d| Some(d.fecha) == fecha).unwrap_or(0);
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

        let Some(fecha) = self.dias.get(self.dia).map(|d| d.fecha) else {
            ui.separator();
            ui.add_space(4.0);
            ui.label(RichText::new("Todavía no hay mediciones guardadas.").color(COL_IDLE));
            return;
        };

        // ── Día: se salta a los días con mediciones ──
        ui.horizontal(|ui| {
            let hay_anterior = self.dia + 1 < self.dias.len();
            if ui.add_enabled(hay_anterior, egui::Button::new("<")).on_hover_text("Día anterior").clicked() {
                self.dia += 1;
            }
            if ui.add_enabled(self.dia > 0, egui::Button::new(">")).on_hover_text("Día siguiente").clicked() {
                self.dia -= 1;
            }
            let es_hoy = fecha == Local::now().date_naive();
            let titulo = format!("{}{}", if es_hoy { "Hoy · " } else { "" }, fecha.format("%d/%m/%Y"));
            ui.label(RichText::new(titulo).color(COL_TEXTO));
        });
        ui.add_space(4.0);

        let (Some(registros), Some(dia)) = (&self.historial, self.dias.get(self.dia)) else {
            return;
        };
        let del_dia = &registros[dia.filas.clone()];

        let horas = histo::resumen_por_hora(del_dia, self.filtro.as_deref(), |ts| {
            momento_local(ts).hour() as usize
        });
        pintar_grafico(ui, &horas);
        pintar_leyenda(ui);

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        let columnas: &[(&str, f32)] = match self.filtro {
            Some(_) => &COLUMNAS_EMPRESA,
            None => &COLUMNAS_TODAS,
        };
        fila(ui, columnas.iter().map(|(titulo, ancho)| {
            (*ancho, RichText::new(*titulo).color(COL_IDLE).weak())
        }));

        // show_rows dibuja solo las filas visibles
        egui::ScrollArea::vertical()
            .id_salt("tabla_historial")
            .auto_shrink([false, false])
            .show_rows(ui, ALTO_FILA, del_dia.len(), |ui, visibles| {
                for r in &del_dia[visibles] {
                    let celdas = match &self.filtro {
                        Some(nombre) => celdas_empresa(r, nombre, dia.fecha),
                        None => celdas_todas(r, dia.fecha),
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

    #[test]
    fn el_color_cambia_en_los_limites() {
        assert_eq!(color_latencia(36.0), COL_OK);
        assert_eq!(color_latencia(LIMITE_MS), COL_OK);
        assert_eq!(color_latencia(50.1), COL_WARN);
        assert_eq!(color_latencia(LIMITE_MS_ALTO), COL_WARN);
        assert_eq!(color_latencia(100.1), COL_OFF);
    }

    fn con_media(ms: f32) -> [ResumenHora; 24] {
        let mut horas = [ResumenHora::default(); 24];
        horas[3].media = Some(ms);
        horas
    }

    #[test]
    fn el_eje_no_baja_de_100_ms_y_contiene_al_mayor_valor() {
        assert_eq!(tope_eje(&[ResumenHora::default(); 24]), 100.0);
        assert_eq!(tope_eje(&con_media(36.0)), 100.0);
        assert_eq!(tope_eje(&con_media(100.5)), 200.0);
        assert_eq!(tope_eje(&con_media(1500.0)), 2000.0);
        assert_eq!(tope_eje(&con_media(9000.0)), 9000.0);
    }

    #[test]
    fn cada_posicion_del_grafico_cae_en_su_hora() {
        assert_eq!(hora_en_x(100.0, 100.0, 240.0), 0);
        assert_eq!(hora_en_x(129.9, 100.0, 240.0), 2);
        assert_eq!(hora_en_x(130.0, 100.0, 240.0), 3);
        assert_eq!(hora_en_x(340.0, 100.0, 240.0), 23);
        assert_eq!(hora_en_x(90.0, 100.0, 240.0), 0);
    }

    #[test]
    fn el_detalle_de_una_hora_describe_lo_que_paso() {
        let vacia = ResumenHora::default();
        assert_eq!(texto_hora(7, &vacia), "07:00 a 08:00\nSin mediciones");

        let corte = ResumenHora { mediciones: 1, sin_respuesta: 1, ..vacia };
        assert_eq!(texto_hora(7, &corte), "07:00 a 08:00\nSin respuesta\n1 medición");

        let mixta = ResumenHora { mediciones: 3, sin_respuesta: 1, media: Some(36.44), peor: Some(40.0) };
        assert_eq!(
            texto_hora(23, &mixta),
            "23:00 a 24:00\nmedia 36.4 ms · peor 40.0 ms\n3 mediciones · 1 sin respuesta"
        );
    }

    #[test]
    fn agrupa_por_dia_de_mas_nuevo_a_mas_viejo() {
        const DIA: u64 = 86_400;
        let mediodia = Local::now().with_hour(12).unwrap().timestamp() as u64;
        let registros = [registro(mediodia), registro(mediodia - 60), registro(mediodia - 3 * DIA)];
        let dias = agrupar_por_dia(&registros);
        assert_eq!(dias.len(), 2);
        assert_eq!(dias[0].filas, 0..2);
        assert_eq!(dias[1].filas, 2..3);
    }

    #[test]
    fn recargar_conserva_el_dia_elegido() {
        const DIA: u64 = 86_400;
        let mediodia = Local::now().with_hour(12).unwrap().timestamp() as u64;
        let viejo = registro(mediodia - 2 * DIA);
        let mut app = KoalaApp::vacia();
        app.asignar_registros(vec![viejo.clone(), registro(mediodia)]);
        assert_eq!(app.dia, 0, "arranca en el día más nuevo");

        app.dia = 1;
        app.asignar_registros(vec![registro(mediodia + 60), viejo, registro(mediodia)]);
        assert_eq!(app.dia, 1);
    }

    /// Dibuja la vista sin abrir una ventana, con las variantes de filtro y sin datos.
    #[test]
    fn la_vista_historial_se_dibuja_en_todos_los_casos() {
        let ctx = egui::Context::default();
        let ahora = Local::now().timestamp() as u64;
        let mut registros: Vec<_> = (0..5_000).map(|i| registro(ahora - i * 300)).collect();
        registros[7].media_ms = None;
        registros[7].empresas[0].media = None;
        for (historial, filtro) in [
            (registros.clone(), None),
            (registros.clone(), Some("GOOGLE")),
            (registros, Some("NO-ESTA")),
            (Vec::new(), None),
        ] {
            let mut app = KoalaApp {
                filtro: filtro.map(String::from),
                ..KoalaApp::vacia()
            };
            app.asignar_registros(historial);
            app.dia = app.dias.len().saturating_sub(1); // también un día que no es hoy
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.pintar_historial(ui));
            });
        }
    }
}
