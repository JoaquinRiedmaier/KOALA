//! KOALA — medidor de estado de internet con widget de waybar y ventana gráfica.
//!
//! Subcomandos:
//!   koala medir    → corre la medición (todos los proveedores en paralelo) y guarda el caché
//!   koala barra    → imprime el JSON del widget para waybar (return-type json)
//!   koala gui      → abre la ventana gráfica (mide al abrir)
//!   koala instalar → integra el widget en HyDE (módulo + colores + posición + reinicio)

mod app;
mod core;
mod gui;
mod install;

use clap::Parser;

#[derive(Parser)]
#[command(name = "koala", about = "Medidor de internet (KOALA) con widget para waybar/HyDE")]
struct Cli {
    #[command(subcommand)]
    comando: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// mide todos los objetivos y guarda el estado (accesible por barra y gui)
    #[command(alias = "measure")]
    Medir,
    /// imprime el JSON del widget para waybar
    #[command(alias = "bar")]
    Barra,
    /// abre la ventana gráfica
    Gui,
    /// instala el widget en waybar/HyDE (idempotente)
    #[command(alias = "install")]
    Instalar,
}

fn main() {
    match Cli::parse().comando {
        Cmd::Medir => app::comando_medir(),
        Cmd::Barra => app::comando_barra(),
        Cmd::Gui => gui::ejecutar(),
        Cmd::Instalar => install::instalar(),
    }
}
