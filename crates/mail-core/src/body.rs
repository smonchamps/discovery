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
        if octets[i] == b'&' {
            const ENTITES: &[(&str, &str)] = &[
                ("&amp;", "&"),
                ("&lt;", "<"),
                ("&gt;", ">"),
                ("&quot;", "\""),
                ("&#39;", "'"),
                ("&apos;", "'"),
                ("&nbsp;", " "),
            ];
            if let Some((motif, valeur)) = ENTITES.iter().find(|(m, _)| commence_par(&html[i..], m))
            {
                if *valeur == " " {
                    if !dernier_blanc {
                        apercu.push(' ');
                        dernier_blanc = true;
                    }
                } else {
                    apercu.push_str(valeur);
                    compte += 1;
                    dernier_blanc = false;
                }
                i += motif.len();
                continue;
            }
        }
        // Avancer d'un CARACTÈRE entier, pas d'un octet.
        let caractere = html[i..].chars().next().unwrap_or(' ');
        i += caractere.len_utf8();
        if caractere.is_whitespace() {
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
