//! UTF-7 modifié (RFC 3501 §5.1.3) : les noms de dossiers IMAP.
//!
//! IMAP est antérieur à UTF-8. Un dossier « Actualité » circule encodé
//! `Actualit&AOk-` : `&` ouvre une séquence, `-` la ferme, et le contenu
//! est du base64 d'UTF-16BE — avec `,` à la place de `/` dans l'alphabet.
//! `&-` est la façon d'écrire un `&` littéral.
//!
//! **Ce module ne décode QUE pour l'affichage et les comparaisons.** Le
//! nom encodé reste celui qu'on renvoie au serveur : envoyer « Actualité »
//! là où le protocole attend `Actualit&AOk-` ferait échouer le SELECT.
//! Les deux noms doivent donc coexister, jamais se remplacer.

use base64::Engine;

/// Décode un nom de dossier IMAP pour l'ŒIL humain.
///
/// Ne peut pas échouer : une séquence malformée est recopiée telle
/// quelle. Un nom un peu laid vaut mieux qu'un dossier disparu — la
/// règle « jamais de perte » vaut aussi pour ce qu'on affiche.
pub(crate) fn decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'&' {
            // Les octets hors séquence sont de l'ASCII imprimable ; on
            // avance caractère par caractère pour rester UTF-8-sûr.
            let rest = &raw[index..];
            let ch = rest.chars().next().unwrap_or('&');
            out.push(ch);
            index += ch.len_utf8();
            continue;
        }
        match bytes[index + 1..].iter().position(|&b| b == b'-') {
            // `&-` : une esperluette littérale.
            Some(0) => {
                out.push('&');
                index += 2;
            }
            Some(offset) => {
                let start = index + 1;
                let end = start + offset;
                match decode_segment(&raw[start..end]) {
                    Some(decoded) => out.push_str(&decoded),
                    // Illisible : on recopie la séquence brute plutôt que
                    // d'inventer ou de perdre.
                    None => out.push_str(&raw[index..=end]),
                }
                index = end + 1;
            }
            // Séquence jamais refermée : le reste est recopié tel quel.
            None => {
                out.push_str(&raw[index..]);
                break;
            }
        }
    }
    out
}

/// Un segment entre `&` et `-` : base64 modifié d'UTF-16BE.
fn decode_segment(segment: &str) -> Option<String> {
    if segment.is_empty() {
        return None;
    }
    // L'alphabet d'IMAP remplace `/` par `,` — sinon c'est du base64
    // standard, sans remplissage.
    let standard = segment.replace(',', "/");
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(standard)
        .ok()?;
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    // `from_utf16` refuse les surrogates orphelins : c'est voulu, ils
    // signalent un encodage cassé.
    String::from_utf16(&units).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ascii_is_returned_unchanged() {
        assert_eq!(decode("INBOX"), "INBOX");
        assert_eq!(decode("[Gmail]/Sent Mail"), "[Gmail]/Sent Mail");
        assert_eq!(decode(""), "");
    }

    /// Le cas exact relevé sur le terrain, consigné dans l'ADR 0006 :
    /// un compte affichait `Actualit&AOk-` au lieu d'« Actualité ».
    #[test]
    fn decodes_the_accented_name_seen_in_production() {
        assert_eq!(decode("Actualit&AOk-"), "Actualité");
    }

    #[test]
    fn decodes_several_sequences_in_one_name() {
        assert_eq!(decode("&AOk-t&AOk-"), "été");
        assert_eq!(decode("Dossier/&AOk-l&AOk-ments"), "Dossier/éléments");
    }

    /// `&-` est la seule façon d'écrire une esperluette : sans ce cas,
    /// « Ventes & Marketing » deviendrait illisible.
    #[test]
    fn an_escaped_ampersand_comes_back_as_itself() {
        assert_eq!(decode("&-"), "&");
        assert_eq!(decode("Ventes &- Marketing"), "Ventes & Marketing");
    }

    /// Alphabets non latins : le `,` de l'alphabet modifié n'apparaît que
    /// sur certains contenus, et c'est exactement là que les décodeurs
    /// écrits à la main se trompent.
    #[test]
    fn decodes_non_latin_scripts() {
        assert_eq!(decode("&BBIEMAQ2BD0EPg-"), "Важно");
        assert_eq!(decode("&ZeVnLIqe-"), "日本語");
    }

    /// Hors du plan multilingue de base, UTF-16 utilise deux unités.
    #[test]
    fn decodes_a_surrogate_pair() {
        assert_eq!(decode("&2D3es9g93qU-"), "🚳🚥");
    }

    /// Un nom mal encodé ne doit ni faire paniquer, ni disparaître : il
    /// s'affiche tel quel, et l'utilisateur voit au moins son dossier.
    #[test]
    fn malformed_sequences_survive_verbatim() {
        assert_eq!(decode("Actualit&AOk"), "Actualit&AOk");
        assert_eq!(decode("&???-"), "&???-");
        assert_eq!(decode("&"), "&");
        assert_eq!(decode("&AO-"), "&AO-");
    }

    /// Le décodage ne doit jamais faire perdre le nom réseau : c'est
    /// l'appelant qui garde les deux. Ce test documente la règle en
    /// montrant qu'un nom décodé n'est PAS ré-encodable ici.
    #[test]
    fn decoding_is_not_reversible_here_by_design() {
        let wire = "Actualit&AOk-";
        let display = decode(wire);
        assert_ne!(display, wire, "l'affichage diffère du nom réseau");
        assert_eq!(
            decode(&display),
            display,
            "re-décoder un nom déjà décodé ne doit rien casser"
        );
    }
}
