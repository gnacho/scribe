//! Hojas de estilo incluidas en el binario.
//!
//! Cada tema es una hoja del subconjunto CSS de [`crate::export_css`] empotrada
//! con `include_str!`, así que no hay nada que instalar y una compilación local
//! se comporta igual que el paquete Flatpak. Los temas solo tocan tipografía y
//! color: la geometría de la página es cosa de la plantilla, de modo que
//! cualquier tema se combina con cualquier plantilla.

use crate::export_css::{analizar, Ajustes};
use crate::settings::ExportTheme;

/// Hoja de estilo empotrada de un tema, si lo tiene.
pub fn hoja(tema: ExportTheme) -> Option<&'static str> {
    match tema {
        ExportTheme::None | ExportTheme::CustomFile => None,
        ExportTheme::Sepia => Some(include_str!("../data/themes/sepia.css")),
        ExportTheme::HighContrast => Some(include_str!("../data/themes/high-contrast.css")),
        ExportTheme::Technical => Some(include_str!("../data/themes/technical.css")),
        ExportTheme::Academic => Some(include_str!("../data/themes/academic.css")),
    }
}

/// Ajustes resueltos de un tema incluido.
pub fn ajustes(tema: ExportTheme) -> Option<Ajustes> {
    hoja(tema).map(analizar)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INCLUIDOS: [ExportTheme; 4] = [
        ExportTheme::Sepia,
        ExportTheme::HighContrast,
        ExportTheme::Technical,
        ExportTheme::Academic,
    ];

    #[test]
    fn tema_none_y_fichero_no_traen_hoja() {
        assert!(hoja(ExportTheme::None).is_none());
        assert!(hoja(ExportTheme::CustomFile).is_none());
        assert!(ajustes(ExportTheme::None).is_none());
    }

    #[test]
    fn cada_tema_tiene_hoja_y_no_impone_margenes() {
        for tema in INCLUIDOS {
            let ajustes = ajustes(tema).expect("el tema debe tener hoja");
            assert!(!ajustes.es_vacio(), "{tema:?} no impone nada");
            assert_eq!(
                ajustes.margenes,
                Default::default(),
                "{tema:?} no debe tocar los márgenes"
            );
        }
    }

    #[test]
    fn las_hojas_incluidas_no_dejan_propiedades_sin_efecto() {
        for tema in INCLUIDOS {
            let ajustes = ajustes(tema).expect("el tema debe tener hoja");
            assert!(
                ajustes.ignoradas.is_empty(),
                "{tema:?} tiene propiedades sin efecto: {:?}",
                ajustes.ignoradas
            );
        }
    }

    #[test]
    fn cada_tema_fija_tipografia_y_colores() {
        for tema in INCLUIDOS {
            let ajustes = ajustes(tema).expect("el tema debe tener hoja");
            assert!(ajustes.familia.is_some(), "{tema:?} sin familia");
            assert!(ajustes.tamano_cuerpo.is_some(), "{tema:?} sin tamaño");
            assert!(ajustes.interlinea.is_some(), "{tema:?} sin interlineado");
            assert!(ajustes.colores.texto.is_some(), "{tema:?} sin color");
            assert!(ajustes.colores.fondo.is_some(), "{tema:?} sin fondo");
            assert!(
                ajustes.colores.atenuado.is_some(),
                "{tema:?} sin color atenuado"
            );
        }
    }

    #[test]
    fn los_temas_se_distinguen_entre_si() {
        let mut vistos: Vec<Ajustes> = Vec::new();
        for tema in INCLUIDOS {
            let ajustes = ajustes(tema).expect("el tema debe tener hoja");
            assert!(
                !vistos.contains(&ajustes),
                "{tema:?} repite los ajustes de otro tema"
            );
            vistos.push(ajustes);
        }
    }
}
