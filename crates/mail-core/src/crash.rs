//! Rédaction d'un rapport de plantage — la partie PURE et prouvée.
//!
//! Un panic hook capture un panic, mais son MESSAGE peut porter une
//! donnée personnelle : `format!("{err:?}")` sur une [`crate::Error`],
//! dont `InvalidEmailAddress` contient une adresse. Ce module transforme
//! les données brutes d'un panic en un rapport dont on PROUVE qu'il ne
//! contient rien de personnel — le message est écarté, on ne garde que
//! des artefacts de CODE (localisation, symboles) et d'environnement.
//!
//! Pur : aucune I/O, aucune dépendance. L'écriture du fichier et le
//! consentement vivent dans l'app — un panic hook ne doit pas toucher la
//! base (elle est peut-être la cause du panic, ou tient un verrou
//! empoisonné), ni rien qui puisse paniquer à son tour.

/// Ce qu'un panic hook a sous la main, avant rédaction.
#[derive(Debug, Clone)]
pub struct RawPanic {
    /// Le message du panic. **Vecteur de fuite** : peut contenir une
    /// adresse, un sujet, un fragment de corps… Il est ÉCARTÉ par
    /// [`redact`], jamais conservé.
    pub message: String,
    /// `fichier:ligne` du panic — une position dans le CODE, fixée à la
    /// compilation, sans donnée d'utilisateur.
    pub location: Option<String>,
    /// La pile d'appels : des symboles de code (noms de fonctions), pas
    /// des valeurs capturées. Sans donnée personnelle par construction.
    pub backtrace: Vec<String>,
    pub app_version: String,
    pub os: String,
    pub timestamp: String,
}

/// Un rapport de plantage prêt à écrire — **sans le message du panic**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReport {
    pub app_version: String,
    pub os: String,
    pub location: Option<String>,
    pub backtrace: Vec<String>,
    pub timestamp: String,
}

/// Écarte tout ce qui peut porter une donnée personnelle.
///
/// La rédaction se réduit à **supprimer le message** : c'est le seul
/// champ qui puisse contenir du texte libre issu d'une erreur, donc
/// potentiellement une adresse ou un sujet. Localisation et pile sont des
/// artefacts de compilation, conservés tels quels — ils identifient le
/// bug sans rien divulguer.
///
/// L'implémentation est triviale (un champ écarté) ; sa valeur est
/// l'INVARIANT, tenu par test : aucune donnée du message ne survit. Si
/// quelqu'un « remet le message pour aider au débogage » un jour, le test
/// `le_rapport_n_emporte_aucune_donnee_du_message` vire au rouge.
pub fn redact(raw: RawPanic) -> CrashReport {
    CrashReport {
        app_version: raw.app_version,
        os: raw.os,
        location: raw.location,
        backtrace: raw.backtrace,
        timestamp: raw.timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L'invariant central du chantier : un rapport de plantage ne doit
    /// JAMAIS contenir de donnée personnelle. Le message d'un panic peut
    /// en porter — ce test en met une adresse ET un sujet, et exige
    /// qu'ils aient disparu du rapport.
    #[test]
    fn le_rapport_n_emporte_aucune_donnee_du_message() {
        let raw = RawPanic {
            message: "envelope invalide: alice.martin@example.com \
                      — sujet « Facture confidentielle Q3 »"
                .to_string(),
            location: Some("crates/mail-core/src/thread.rs:42".to_string()),
            backtrace: vec![
                "mail_core::thread::attach".to_string(),
                "mail_core::store::Store::upsert_envelopes".to_string(),
            ],
            app_version: "0.1.2".to_string(),
            os: "Windows 11".to_string(),
            timestamp: "2026-07-26T15:00:00Z".to_string(),
        };

        let report = redact(raw);

        // La représentation Debug, et non une liste de champs choisie à la
        // main : elle inclut AUTOMATIQUEMENT tout champ ajouté un jour.
        // Si quelqu'un remet un champ `message` dans le rapport, il
        // apparaîtra ici et le test virera au rouge — c'est ce qui donne
        // sa dent à l'invariant.
        let foin = format!("{report:?}");

        assert!(
            !foin.contains("alice.martin@example.com"),
            "l'adresse a fuité dans le rapport"
        );
        assert!(!foin.contains('@'), "aucune arobase ne doit subsister");
        assert!(
            !foin.contains("Facture"),
            "le sujet a fuité dans le rapport"
        );
        assert!(
            !foin.to_lowercase().contains("confidentielle"),
            "le sujet a fuité dans le rapport"
        );
    }

    /// Mais il GARDE ce qui sert à trouver le bug — sinon le rapport est
    /// inutile. Localisation, pile, versions : des artefacts de code, sûrs.
    #[test]
    fn le_rapport_garde_de_quoi_situer_le_bug() {
        let raw = RawPanic {
            message: "peu importe".to_string(),
            location: Some("crates/mail-core/src/thread.rs:42".to_string()),
            backtrace: vec!["mail_core::thread::attach".to_string()],
            app_version: "0.1.2".to_string(),
            os: "Windows 11".to_string(),
            timestamp: "2026-07-26T15:00:00Z".to_string(),
        };

        let report = redact(raw);

        assert_eq!(
            report.location.as_deref(),
            Some("crates/mail-core/src/thread.rs:42"),
            "la localisation situe le bug"
        );
        assert_eq!(
            report.backtrace,
            vec!["mail_core::thread::attach".to_string()],
            "la pile est conservée"
        );
        assert_eq!(report.app_version, "0.1.2");
        assert_eq!(report.os, "Windows 11");
    }
}
