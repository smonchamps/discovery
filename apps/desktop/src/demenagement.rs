//! Déménagement des données Discovery → Wind (PLAN-WIND E3, W-D1/W-D5).
//!
//! La bascule de l'identifiant (`dev.discovery.app` → `dev.elements.wind`)
//! change les deux dossiers de données : `%APPDATA%` (la base et ses
//! compagnons WAL) et `%LOCALAPPDATA%` (le profil WebView2, donc le
//! localStorage). Un poste Discovery doit retrouver TOUT son état au
//! premier lancement Wind — jamais de reconnexion, jamais de base vide.
//!
//! Le geste est un `rename` par dossier (même volume : atomique, aucun
//! octet copié — la base du terrain pèse 715 Mo), puis `discovery.db`
//! devient `wind.db`, compagnons d'abord et fichier maître en dernier :
//! si une passe meurt au milieu, la relance repart du marqueur (le `.db`
//! encore à l'ancien nom) sans rien perdre ni rien écraser.
//!
//! Court-circuit : `WIND_DB_PATH` posé (harnais e2e, ADR 0014) —
//! les bases jetables des bancs n'ont rien à déménager.

use std::io;
use std::path::{Path, PathBuf};

/// L'identifiant d'avant la bascule Wind. Seul endroit du code autorisé
/// à le citer : le pont vit tant que des postes Discovery existent.
const ANCIEN_IDENTIFIANT: &str = "dev.discovery.app";
/// Doit égaler `identifier` de `tauri.conf.json`.
const IDENTIFIANT: &str = "dev.elements.wind";

/// Déménage les données d'un poste Discovery vers les chemins Wind.
/// Répétable : un poste déjà déménagé — ou neuf — ne fait rien.
pub fn demenager() -> io::Result<()> {
    if std::env::var("WIND_DB_PATH").is_ok() {
        return Ok(());
    }
    for racine in ["APPDATA", "LOCALAPPDATA"] {
        let Ok(base) = std::env::var(racine) else {
            continue;
        };
        let base = PathBuf::from(base);
        demenager_dossier(&base.join(ANCIEN_IDENTIFIANT), &base.join(IDENTIFIANT))?;
    }
    if let Ok(base) = std::env::var("APPDATA") {
        renommer_base(&PathBuf::from(base).join(IDENTIFIANT))?;
    }
    Ok(())
}

/// Le dossier entier, d'un seul `rename` — jamais si la cible existe
/// déjà : elle serait le fruit d'un lancement Wind antérieur, et
/// l'écraser détruirait l'état le plus récent.
fn demenager_dossier(ancien: &Path, nouveau: &Path) -> io::Result<()> {
    if ancien.is_dir() && !nouveau.exists() {
        std::fs::rename(ancien, nouveau)?;
    }
    Ok(())
}

/// `discovery.db` → `wind.db` dans le dossier déménagé. Les compagnons
/// (`-wal`, `-shm`, ADR 0011) partent d'abord, le `.db` en dernier : le
/// fichier maître est le marqueur de la passe — tant qu'il porte
/// l'ancien nom, la relance reprend là où la passe est morte.
fn renommer_base(dossier: &Path) -> io::Result<()> {
    if !dossier.join("discovery.db").is_file() || dossier.join("wind.db").exists() {
        return Ok(());
    }
    for suffixe in ["-wal", "-shm", ""] {
        let source = dossier.join(format!("discovery.db{suffixe}"));
        if source.is_file() {
            std::fs::rename(&source, dossier.join(format!("wind.db{suffixe}")))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bac(nom: &str) -> PathBuf {
        let dossier = std::env::temp_dir().join(format!(
            "wind-test-demenagement-{nom}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(&dossier).unwrap();
        dossier
    }

    #[test]
    fn un_poste_discovery_demenage_dossier_et_base() {
        let bac = bac("complet");
        let ancien = bac.join(ANCIEN_IDENTIFIANT);
        std::fs::create_dir_all(&ancien).unwrap();
        std::fs::write(ancien.join("discovery.db"), b"base").unwrap();
        std::fs::write(ancien.join("discovery.db-wal"), b"wal").unwrap();
        let nouveau = bac.join(IDENTIFIANT);

        demenager_dossier(&ancien, &nouveau).unwrap();
        renommer_base(&nouveau).unwrap();

        assert!(!ancien.exists());
        assert_eq!(std::fs::read(nouveau.join("wind.db")).unwrap(), b"base");
        assert_eq!(std::fs::read(nouveau.join("wind.db-wal")).unwrap(), b"wal");
        assert!(!nouveau.join("discovery.db").exists());
        let _ = std::fs::remove_dir_all(&bac);
    }

    #[test]
    fn un_poste_wind_existant_nest_jamais_ecrase() {
        let bac = bac("jamais-ecrase");
        let ancien = bac.join(ANCIEN_IDENTIFIANT);
        std::fs::create_dir_all(&ancien).unwrap();
        std::fs::write(ancien.join("discovery.db"), b"vieux").unwrap();
        let nouveau = bac.join(IDENTIFIANT);
        std::fs::create_dir_all(&nouveau).unwrap();
        std::fs::write(nouveau.join("wind.db"), b"recent").unwrap();

        demenager_dossier(&ancien, &nouveau).unwrap();
        renommer_base(&nouveau).unwrap();

        assert_eq!(
            std::fs::read(ancien.join("discovery.db")).unwrap(),
            b"vieux"
        );
        assert_eq!(std::fs::read(nouveau.join("wind.db")).unwrap(), b"recent");
        let _ = std::fs::remove_dir_all(&bac);
    }

    #[test]
    fn une_passe_interrompue_reprend_sans_perte() {
        // Le compagnon `-wal` est déjà passé côté Wind, le `.db` est
        // encore ancien : la relance doit finir le geste sans toucher
        // au compagnon déjà déménagé.
        let bac = bac("reprise");
        let dossier = bac.join(IDENTIFIANT);
        std::fs::create_dir_all(&dossier).unwrap();
        std::fs::write(dossier.join("discovery.db"), b"base").unwrap();
        std::fs::write(dossier.join("wind.db-wal"), b"wal").unwrap();

        renommer_base(&dossier).unwrap();

        assert_eq!(std::fs::read(dossier.join("wind.db")).unwrap(), b"base");
        assert_eq!(std::fs::read(dossier.join("wind.db-wal")).unwrap(), b"wal");
        assert!(!dossier.join("discovery.db").exists());
        let _ = std::fs::remove_dir_all(&bac);
    }

    #[test]
    fn un_poste_neuf_ne_fait_rien() {
        let bac = bac("neuf");
        let ancien = bac.join(ANCIEN_IDENTIFIANT);
        let nouveau = bac.join(IDENTIFIANT);

        demenager_dossier(&ancien, &nouveau).unwrap();
        renommer_base(&nouveau).unwrap();

        assert!(!ancien.exists());
        assert!(!nouveau.exists());
        let _ = std::fs::remove_dir_all(&bac);
    }
}
