//! Dé-échappement des `quoted-string` IMAP (RFC 3501 §4.3).
//!
//! `imap-proto` retire les guillemets externes d'une chaîne entre
//! guillemets mais **laisse le contenu brut**, escapes compris (prouvé par
//! ses propres tests `core.rs` : `quoted("Hello \" ")` rend `Hello \" `).
//! Sans ce passage, tout objet, nom d'expéditeur ou adresse contenant un
//! `"` ou un `\` s'affiche parasité (R2, PLAN-RETOURS-MAIL).
//!
//! Vit dans `mail-core` — et non dans l'adaptateur IMAP — parce que DEUX
//! chemins en ont besoin : le décodage à la synchro (`mail-imap`), et la
//! réparation des enveloppes déjà stockées avec leurs escapes (migration
//! `store.rs`, pour les messages synchronisés avant le correctif).

use std::borrow::Cow;

/// Retire les backslash-escapes d'une `quoted-string` IMAP : `\"` → `"`,
/// `\\` → `\`, les deux SEULES séquences valides (RFC 3501).
///
/// Compromis assumé : IMAP transmet aussi les chaînes en *littéral*
/// (`{n}`), où les octets sont bruts — et `imap-proto` ne nous dit pas
/// laquelle il a lue. Dé-échapper corromprait un littéral contenant
/// réellement `\"` (cas rarissime) ; on tranche pour le cas courant, comme
/// tout client mûr. Une entrée sans `\` ressort empruntée, sans allocation.
pub fn unescape_imap_quoted(raw: &[u8]) -> Cow<'_, [u8]> {
    if !raw.contains(&b'\\') {
        return Cow::Borrowed(raw);
    }
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        // Un `\` ne se consomme QUE devant `"` ou `\` ; devant autre chose
        // (entrée malformée) il est gardé tel quel, rien n'est perdu.
        if raw[i] == b'\\' && matches!(raw.get(i + 1), Some(b'"' | b'\\')) {
            out.push(raw[i + 1]);
            i += 2;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    Cow::Owned(out)
}

/// Variante chaîne, pour réparer une valeur déjà stockée (UTF-8 en base) :
/// dé-échappe et rend une `String`. Empruntée sans copie quand rien ne
/// change.
pub fn unescape_imap_quoted_str(value: &str) -> Cow<'_, str> {
    match unescape_imap_quoted(value.as_bytes()) {
        Cow::Borrowed(_) => Cow::Borrowed(value),
        // Le dé-échappement ne retire que des octets ASCII (`\`), jamais
        // au milieu d'une séquence multi-octets : le résultat reste de
        // l'UTF-8 valide.
        Cow::Owned(bytes) => Cow::Owned(String::from_utf8_lossy(&bytes).into_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retire_les_guillemets_echappes() {
        assert_eq!(
            unescape_imap_quoted_str(r#"Test \"Envoyes\""#),
            r#"Test "Envoyes""#
        );
    }

    #[test]
    fn retire_le_backslash_double() {
        assert_eq!(
            unescape_imap_quoted_str(r"chemin C:\\temp"),
            r"chemin C:\temp"
        );
    }

    #[test]
    fn une_chaine_sans_backslash_est_empruntee() {
        assert!(matches!(
            unescape_imap_quoted_str("Reunion de demain"),
            Cow::Borrowed(_)
        ));
    }

    /// Un `\` malformé (devant autre chose que `"` ou `\`) est gardé.
    #[test]
    fn un_backslash_isole_survit() {
        assert_eq!(unescape_imap_quoted_str(r"a\b"), r"a\b");
    }
}
