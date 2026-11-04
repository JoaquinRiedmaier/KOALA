fn escribir_con_marcadores(ruta: &PathBuf, marca_ini: &str, marca_fin: &str, bloque: &str) -> Result<(), String> {
    let contenido = fs::read_to_string(ruta).unwrap_or_default();
    let nuevo = if let Some(ini) = contenido.find(marca_ini) {
        // ya existe el bloque: reemplazar solo esa sección
        let Some(fin) = contenido.find(marca_fin) else {
            return Err("marcador final faltante".into());
        };
        let fin = fin + marca_fin.len();
        format!("{}\n{}\n{}\n{}", &contenido[..ini], bloque, &contenido[fin..])
    } else {
        let sep = if contenido.is_empty() || contenido.ends_with('\n') { "" } else { "\n" };
        format!("{}\n{}\n{}\n{}\n{}", contenido, sep, marca_ini, bloque, marca_fin)
    };
    
    // Verificar que no estamos creando duplicación
    if nuevo.contains(marca_ini) && nuevo.matches(marca_ini).count() > 1 {
        return Err("posible duplicación de marcadores detectada".into());
    }
    
    fs::write(ruta, nuevo).map_err(|e| e.to_string())
}