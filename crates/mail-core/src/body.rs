//! Chargement à la demande du corps d'un message : cache SQLite d'abord,
//! serveur ensuite, puis mise en cache — le principe « enveloppes d'abord »
//! appliqué jusqu'au bout (le corps n'arrive qu'au clic, puis reste offline).

use crate::envelope::Uid;
use crate::error::Error;
use crate::remote::MailServer;
use crate::store::Store;

/// Corps HTML brut (pré-assainissement) d'un message. `None` si la boîte n'a
/// jamais été synchronisée ou si le message a disparu du serveur.
pub fn load_body(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    uid: Uid,
) -> Result<Option<String>, Error> {
    if let Some(cached) = store.body(account_id, mailbox, uid)? {
        return Ok(Some(cached));
    }
    let Some(state) = store.sync_state(account_id, mailbox)? else {
        return Ok(None);
    };
    match server.fetch_body_html(mailbox, uid)? {
        Some(fetched) => {
            store.save_body(state.mailbox_id, uid, &fetched.html, &fetched.attachments)?;
            Ok(Some(fetched.html))
        }
        None => Ok(None),
    }
}

/// Aperçu texte d'un corps — la ligne grise sous l'objet (écran 02 de la
/// refonte). Calculé UNE fois, à l'écriture du corps (`save_body`) ou au
/// rattrapage borné (`preview_catchup`) — jamais au défilement : la page
/// de liste reste au coût du gate P1.
///
/// Tolérant au HTML BRUT (le corps est stocké pré-assainissement) : le
/// contenu de `<style>`, `<script>`, `<title>` et des commentaires est
/// ignoré, les entités usuelles décodées, les blancs repliés, le tout
/// tronqué à 160 caractères sans couper un caractère.
pub(crate) fn extraire_apercu(html: &str) -> String {
    const LIMITE: usize = 160;

    // Comparaisons ASCII-insensibles À LA POSITION, jamais une copie
    // minuscule du document : certains caractères changent de longueur
    // en minuscules, et des index pris sur la copie paniqueraient sur
    // l'original. Les balises et entités sont ASCII — c'est suffisant.
    fn commence_par(reste: &str, motif: &str) -> bool {
        reste.len() >= motif.len()
            && reste
                .as_bytes()
                .iter()
                .zip(motif.as_bytes())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    }
    fn trouver(reste: &str, motif: &str) -> Option<usize> {
        (0..=reste.len().saturating_sub(motif.len()))
            .find(|&depart| reste.is_char_boundary(depart) && commence_par(&reste[depart..], motif))
    }

    let mut apercu = String::new();
    let mut compte = 0usize;
    let mut i = 0;
    let octets = html.as_bytes();
    let mut dernier_blanc = true;
    while i < octets.len() && compte < LIMITE {
        if octets[i] == b'<' {
            if commence_par(&html[i..], "<!--") {
                i = trouver(&html[i..], "-->").map_or(html.len(), |fin| i + fin + 3);
                continue;
            }
            // Les conteneurs dont le TEXTE ne doit jamais fuiter dans
            // l'aperçu : feuilles de style, scripts, titre de document.
            let mut englobant = false;
            for balise in ["style", "script", "title"] {
                if commence_par(&html[i + 1..], balise) {
                    let fermeture = format!("</{balise}");
                    let apres = trouver(&html[i..], &fermeture)
                        .map_or(html.len(), |fin| i + fin + fermeture.len());
                    // Jusqu'au chevron INCLUS : « </style> » entier.
                    i = html[apres..]
                        .find('>')
                        .map_or(html.len(), |fin| apres + fin + 1);
                    englobant = true;
                    break;
                }
            }
            if englobant {
                continue;
            }
            i = html[i..].find('>').map_or(html.len(), |fin| i + fin + 1);
            // Une balise vaut un blanc : « </p><p> » ne colle pas deux mots.
            if !dernier_blanc {
                apercu.push(' ');
                dernier_blanc = true;
            }
            continue;
        }
        if octets[i] == b'&'
            && let Some((longueur, decode)) = decoder_entite(&html[i..])
        {
            i += longueur;
            match decode {
                Some(c) if !c.is_whitespace() && !est_invisible(c) => {
                    apercu.push(c);
                    compte += 1;
                    dernier_blanc = false;
                }
                // Blanc, caractère invisible (chevilles de pré-en-tête :
                // &zwnj;, &shy;, espaces fines…) ou entité inconnue :
                // vaut UN blanc, jamais un résidu « &#8199; » à l'écran.
                _ => {
                    if !dernier_blanc {
                        apercu.push(' ');
                        dernier_blanc = true;
                    }
                }
            }
            continue;
        }
        // Avancer d'un CARACTÈRE entier, pas d'un octet.
        let caractere = html[i..].chars().next().unwrap_or(' ');
        i += caractere.len_utf8();
        if caractere.is_whitespace() || est_invisible(caractere) {
            if !dernier_blanc {
                apercu.push(' ');
                dernier_blanc = true;
            }
        } else {
            apercu.push(caractere);
            compte += 1;
            dernier_blanc = false;
        }
    }
    apercu.trim().to_string()
}

/// Décode UNE entité HTML au début de `reste` (qui commence par `&`).
/// Rend la longueur consommée et le caractère — `None` pour une entité
/// inconnue (consommée quand même : mieux vaut un blanc qu'un résidu).
/// Rend `None` tout court si ce `&` n'ouvre pas une entité : il se lit
/// alors comme un caractère ordinaire (« R&D »).
fn decoder_entite(reste: &str) -> Option<(usize, Option<char>)> {
    let octets = reste.as_bytes();
    // Numérique : &#233; ou &#xE9; — terminée par « ; » sinon ce n'est
    // pas une entité.
    if octets.len() > 2 && octets[1] == b'#' {
        let (base, depart) = if octets[2] == b'x' || octets[2] == b'X' {
            (16u32, 3usize)
        } else {
            (10u32, 2usize)
        };
        let fin = octets[depart..]
            .iter()
            .position(|o| !o.is_ascii_hexdigit())
            .map(|n| depart + n)?;
        if fin == depart || fin - depart > 7 || octets.get(fin) != Some(&b';') {
            return None;
        }
        let valeur = u32::from_str_radix(&reste[depart..fin], base).ok()?;
        // Un point de code invalide ou de contrôle vaut un blanc.
        let c = char::from_u32(valeur).filter(|c| !c.is_control());
        return Some((fin + 1, c));
    }
    // Nommée : &nom; — nom ASCII de 2 à 32 caractères.
    let fin = octets[1..]
        .iter()
        .position(|o| !o.is_ascii_alphanumeric())
        .map(|n| 1 + n)?;
    if !(3..=33).contains(&fin) || octets.get(fin) != Some(&b';') || !octets[1].is_ascii_alphabetic() {
        return None;
    }
    let nom = &reste[1..fin];
    Some((fin + 1, entite_nommee(nom)))
}

/// Les entités nommées décodées — celles du courrier réel : structure
/// HTML, lettres accentuées (Latin-1), typographie. Une entité absente
/// d'ici est consommée et vaut un blanc — l'aperçu ne montre JAMAIS de
/// « &eacute; » brut.
fn entite_nommee(nom: &str) -> Option<char> {
    Some(match nom {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "eacute" => 'é',
        "egrave" => 'è',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "agrave" => 'à',
        "acirc" => 'â',
        "aacute" => 'á',
        "ccedil" => 'ç',
        "ocirc" => 'ô',
        "ouml" => 'ö',
        "oacute" => 'ó',
        "otilde" => 'õ',
        "ugrave" => 'ù',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "uacute" => 'ú',
        "icirc" => 'î',
        "iuml" => 'ï',
        "iacute" => 'í',
        "ntilde" => 'ñ',
        "aelig" => 'æ',
        "oelig" => 'œ',
        "szlig" => 'ß',
        "aring" => 'å',
        "oslash" => 'ø',
        "yuml" => 'ÿ',
        "Eacute" => 'É',
        "Egrave" => 'È',
        "Ecirc" => 'Ê',
        "Agrave" => 'À',
        "Acirc" => 'Â',
        "Ccedil" => 'Ç',
        "Ocirc" => 'Ô',
        "AElig" => 'Æ',
        "OElig" => 'Œ',
        "rsquo" => '’',
        "lsquo" => '‘',
        "rdquo" => '”',
        "ldquo" => '“',
        "hellip" => '…',
        "ndash" => '–',
        "mdash" => '—',
        "laquo" => '«',
        "raquo" => '»',
        "middot" => '·',
        "bull" => '•',
        "deg" => '°',
        "euro" => '€',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "times" => '×',
        "divide" => '÷',
        "plusmn" => '±',
        "sup2" => '²',
        "sup3" => '³',
        "frac12" => '½',
        "frac14" => '¼',
        "frac34" => '¾',
        "sect" => '§',
        "para" => '¶',
        "minus" => '−',
        // Blancs et chevilles invisibles : décodés vers leur caractère,
        // `est_invisible`/`is_whitespace` les replient en un blanc.
        "nbsp" => '\u{00A0}',
        "ensp" => '\u{2002}',
        "emsp" => '\u{2003}',
        "thinsp" => '\u{2009}',
        "zwnj" => '\u{200C}',
        "zwj" => '\u{200D}',
        "shy" => '\u{00AD}',
        "lrm" => '\u{200E}',
        "rlm" => '\u{200F}',
        _ => return None,
    })
}

/// Vrai si le texte contient encore une entité HTML bien formée, OU se
/// termine par une entité TRONQUÉE (« …&#12852 » : le premier décodeur
/// coupait à 160 au milieu d'une entité) — le critère de la réparation
/// des aperçus (migrate). Sur-large d'un cheveu en fin de texte
/// (« …R&D » re-matche) : la réparation est UNE passe marquée, le seul
/// coût est un recalcul.
pub(crate) fn contient_entite_residuelle(texte: &str) -> bool {
    let entiere = texte
        .char_indices()
        .filter(|(_, c)| *c == '&')
        .any(|(i, _)| decoder_entite(&texte[i..]).is_some());
    let queue_tronquee = texte
        .rfind('&')
        .is_some_and(|i| {
            let apres = &texte[i + 1..];
            !apres.is_empty()
                && apres
                    .bytes()
                    .all(|o| o.is_ascii_alphanumeric() || o == b'#')
        });
    entiere || queue_tronquee
}

/// Les caractères de mise en forme sans dessin : chevilles de
/// pré-en-tête des newsletters (&zwnj;, &shy;, U+034F…). Dans un aperçu
/// d'une ligne, ils valent un blanc.
fn est_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    #[test]
    fn l_apercu_ignore_styles_scripts_et_commentaires() {
        let html = "<html><head><title>Titre cache</title>\n<style>p { color: red; }</style></head>\
                    <body><!-- note --><p>Bonjour&nbsp;Paul,</p><p>l&#39;essentiel &amp; le reste.</p>\
                    <script>var x = 1;</script></body></html>";
        assert_eq!(
            extraire_apercu(html),
            "Bonjour Paul, l'essentiel & le reste."
        );
    }

    #[test]
    fn l_apercu_replie_les_blancs_et_passe_le_texte_brut() {
        assert_eq!(
            extraire_apercu("Bonjour,\n\n   deux  créneaux\tse chevauchent."),
            "Bonjour, deux créneaux se chevauchent."
        );
    }

    #[test]
    fn l_apercu_decode_les_entites_numeriques_et_nommees() {
        // Le motif RÉEL du terrain : accents en entités décimales, hex
        // et nommées — plus l'apostrophe typographique.
        assert_eq!(
            extraire_apercu("Vos r&#233;f&#233;rences ont &#xE9;t&#xE9; re&ccedil;ues, merci d&rsquo;avoir voyag&eacute;."),
            "Vos références ont été reçues, merci d’avoir voyagé."
        );
    }

    #[test]
    fn l_apercu_replie_les_chevilles_invisibles_en_un_blanc() {
        // Chevilles de pré-en-tête des newsletters : zwnj, shy, espaces
        // fines en entités — jamais un résidu « &#8199; » à l'écran.
        assert_eq!(
            extraire_apercu("R&#233;compense&#847;&zwnj;&#8199;&shy;&zwnj; &#8202; d&eacute;bloqu&eacute;e"),
            "Récompense débloquée"
        );
        // Une entité INCONNUE vaut un blanc, pas un résidu.
        assert_eq!(extraire_apercu("avant&inconnue;apr&egrave;s"), "avant après");
        // Un « & » ordinaire reste un caractère : R&D.
        assert_eq!(extraire_apercu("R&D et &#litige"), "R&D et &#litige");
    }

    #[test]
    fn le_critere_de_reparation_attrape_entites_et_queues_tronquees() {
        // Entité bien formée au milieu — le cas massif du terrain.
        assert!(contient_entite_residuelle("Vos r&#233;f&#233;rences"));
        assert!(contient_entite_residuelle("voyag&eacute; loin"));
        // Entité TRONQUÉE par la coupe à 160 de l'ancien décodeur.
        assert!(contient_entite_residuelle("des journ es &#12852"));
        assert!(contient_entite_residuelle("fin coup&eacu"));
        // Texte sain : rien à réparer.
        assert!(!contient_entite_residuelle("références décodées, R&D comprise."));
        assert!(!contient_entite_residuelle("aucune esperluette"));
    }

    #[test]
    fn l_apercu_tronque_a_160_sans_couper_un_caractere() {
        let long = "é".repeat(400);
        let apercu = extraire_apercu(&long);
        assert_eq!(apercu.chars().count(), 160);
        assert!(apercu.chars().all(|c| c == 'é'));
    }

    fn synced_setup() -> (FakeServer, Store, i64) {
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "sujet", "<p>corps du message</p>");
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        (server, store, account)
    }

    #[test]
    fn fetches_then_serves_from_cache() {
        let (mut server, mut store, account) = synced_setup();

        let first = load_body(&mut server, &mut store, account, "INBOX", 1).unwrap();
        assert_eq!(first.as_deref(), Some("<p>corps du message</p>"));
        assert_eq!(server.body_fetches, 1);

        let second = load_body(&mut server, &mut store, account, "INBOX", 1).unwrap();
        assert_eq!(second.as_deref(), Some("<p>corps du message</p>"));
        assert_eq!(server.body_fetches, 1, "le cache doit éviter le serveur");
    }

    #[test]
    fn returns_none_for_vanished_message() {
        let (mut server, mut store, account) = synced_setup();
        assert_eq!(
            load_body(&mut server, &mut store, account, "INBOX", 99).unwrap(),
            None
        );
    }

    #[test]
    fn returns_none_before_first_sync_without_touching_server() {
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "sujet", "<p>x</p>");
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();

        assert_eq!(
            load_body(&mut server, &mut store, account, "INBOX", 1).unwrap(),
            None
        );
        assert_eq!(server.body_fetches, 0);
    }
}
