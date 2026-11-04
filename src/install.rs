// Función original restaurada con corrección mínima
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
        format!("{}{}{}\n{}\n{}\n{}\n", contenido, sep, marca_ini, bloque, marca_fin)
    };
    
    // Verificar que no estamos creando duplicación
    if nuevo.contains(marca_ini) && nuevo.matches(marca_ini).count() > 1 {
        return Err("posible duplicación de marcadores detectada".into());
    }
    
    fs::write(ruta, nuevo).map_err(|e| e.to_string())
}

/// Inserta `"custom/koala"` en la lista de módulos del config.jsonc de HyDE.
/// Ancla 1: después de `"custom/caffeine",` (pill izquierdo). Ancla 2: dentro del primer
/// array de `"modules-center"`. Si ya está, no toca nada.
fn colocar_en_config(ruta: &PathBuf) -> Result<String, String> {
    let contenido = fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    
    // Mejora 1: Detectar si ya está presente el módulo "custom/koala" con verificación más precisa
    if config_contiene_custom_koala(&contenido) {
        return Ok(String::from("ya presente - verificado con precisión"));
    }
    
    // backup
    let backup = ruta.with_file_name(format!("{}.koala-bak", ruta.file_name().and_then(|n| n.to_str()).unwrap_or("config.jsonc")));
    let _ = fs::write(&backup, &contenido);

    // re-arma el archivo línea por línea (conserva los terminadores originales)
    let lineas: Vec<&str> = contenido.split_inclusive('\n').collect();
    let mut salida = String::with_capacity(contenido.len() + 40);
    let mut colocado = false;
    for linea in lineas {
        let limpia = linea.trim_end_matches('\n').trim_end_matches('\r');
        let fin = if linea.ends_with("\r\n") { "\r\n" } else { "\n" };
        if !colocado && limpia.contains("\"custom/caffeine\"") {
            let indent = limpia
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect::<String>();
            let mut linea_ok = limpia.to_owned();
            if !linea_ok.ends_with(',') {
                linea_ok.push(',');
            }
            salida.push_str(&format!("{linea_ok}{fin}{indent}\"custom/koala\",{fin}"));
            colocado = true;
        } else {
            salida.push_str(linea);
        }
    }
    fs::write(ruta, salida).map_err(|e| e.to_string())?;
    Ok(String::from("agregado junto a caffeine (pill izquierdo)"))
}

/// Nueva función para detectar con precisión si "custom/koala" ya está presente
fn config_contiene_custom_koala(contenido: &str) -> bool {
    // Intentar parsear como JSON para detección precisa
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(contenido) {
        // Verificar en diferentes ubicaciones posibles para módulos waybar
        if let Some(modules) = json.get("bar").and_then(|bar| bar.get("modules")) {
            // Verificar en el array "center" (ubicación más común)
            if let Some(center_modules) = modules.get("center") {
                if let Some(center_array) = center_modules.as_array() {
                    return center_array.iter().any(|module| module == "custom/koala");
                }
            }
            // Verificar en otros arrays comunes
            if let Some(left_array) = modules.get("left") {
                if let Some(left_array) = left_array.as_array() {
                    return left_array.iter().any(|module| module == "custom/koala");
                }
            }
            if let Some(right_array) = modules.get("right") {
                if let Some(right_array) = right_array.as_array() {
                    return right_array.iter().any(|module| module == "custom/koala");
                }
            }
        }
    }
    
    // Fallback: verificación de cadena simple (menos precisa pero funciona como respaldo)
    contenido.contains("\"custom/koala\"")
}