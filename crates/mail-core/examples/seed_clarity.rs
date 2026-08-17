//! Jeu d'essai « Clarity » — le décor EXACT du prototype d'origine
//! (supprimé à l'amendement A18 du Système ; import au commit 9975a12),
//! seedé dans une vraie base. Né pour le banc de parité de la refonte
//! (PLAN-UI-V2 §4, banc retiré avec le prototype), il reste le décor
//! des e2e et des sections d'écran du Système.
//!
//! Le décor : 18 conversations en réception dont 4 non lues, le fil
//! « Relecture du contrat Vantis » (3 messages dont une réponse à soi,
//! pièces jointes), 12 envoyés, 2 brouillons, 3 indésirables (2 non
//! lus), 64 archives, 3 à la corbeille — répartis sur DEUX comptes
//! (le modèle réel remplace la fiction Travail/Personnel).
//!
//! Les dates sont RELATIVES au lancement (aujourd'hui 09:12, hier,
//! il y a 6 jours…) : le banc reste comparable au prototype quel que
//! soit le jour où il tourne.
//!
//! ```powershell
//! cargo run -p mail-core --example seed_clarity --release -- <chemin.db>
//! ```

use chrono::{DateTime, Duration, Local, Utc};
use mail_core::{Attachment, DraftContent, Envelope, Store, Uid};

const UIDV: u32 = 424243;

fn quand(jours: i64, heure: u32, minute: u32) -> Option<DateTime<Utc>> {
    let jour = Local::now().date_naive() - Duration::days(jours);
    jour.and_hms_opt(heure, minute, 0)
        .and_then(|naif| naif.and_local_timezone(Local).single())
        .map(|local| local.with_timezone(&Utc))
}

#[allow(clippy::too_many_arguments)]
fn message(
    uid: Uid,
    sujet: &str,
    expediteur: &str,
    adresse: &str,
    mid: &str,
    irt: Option<&str>,
    date: Option<DateTime<Utc>>,
    lu: bool,
) -> Envelope {
    Envelope {
        uid,
        subject: Some(sujet.to_string()),
        sender: Some(expediteur.to_string()),
        sender_address: Some(adresse.to_string()),
        message_id: Some(mid.to_string()),
        in_reply_to: irt.map(str::to_string),
        date,
        seen: lu,
        flagged: false,
        to_addrs: Vec::new(),
        cc_addrs: Vec::new(),
    }
}

fn corps(paragraphes: &[&str]) -> String {
    paragraphes
        .iter()
        .map(|p| format!("<p>{p}</p>"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn piece(index: usize, nom: &str, mime: &str, taille: u64) -> Attachment {
    Attachment {
        index,
        name: nom.to_string(),
        mime: mime.to_string(),
        size: taille,
    }
}

fn boite(store: &mut Store, compte: i64, nom: &str) -> Result<i64, mail_core::Error> {
    match store.sync_state(compte, nom)? {
        Some(etat) => {
            store.reset_mailbox(etat.mailbox_id, UIDV)?;
            Ok(etat.mailbox_id)
        }
        None => store.create_mailbox(compte, nom, UIDV),
    }
}

fn dossiers(store: &Store, compte: i64) -> Result<(), mail_core::Error> {
    let noms = [
        "INBOX",
        "Envoyés",
        "Brouillons",
        "Spam",
        "Archives",
        "Corbeille",
    ];
    store.replace_folders(
        compte,
        &noms
            .iter()
            .map(|nom| mail_core::Folder {
                wire: nom.to_string(),
                display: nom.to_string(),
                selectable: true,
            })
            .collect::<Vec<_>>(),
    )
}

fn remplissage(
    store: &mut Store,
    mailbox: i64,
    premier_uid: Uid,
    nombre: u32,
    prefixe: &str,
    depuis_jours: i64,
    lu: bool,
) -> Result<(), mail_core::Error> {
    let lots: Vec<Envelope> = (0..nombre)
        .map(|n| {
            let uid = premier_uid + n;
            message(
                uid,
                &format!("{prefixe} n°{}", n + 1),
                "Atelier Nord",
                "contact@atelier-nord.fr",
                &format!("<clarity-{prefixe}-{uid}@exemple.fr>"),
                None,
                quand(depuis_jours + i64::from(n), 10, 15),
                lu,
            )
        })
        .collect();
    store.upsert_envelopes(mailbox, &lots)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chemin = std::env::args()
        .nth(1)
        .ok_or("usage : seed_clarity <chemin.db>")?;
    let mut store = Store::open(std::path::Path::new(&chemin))?;

    let travail = store.adopt_or_create_account("paul.merand@atelier-nord.fr", "gmail")?;
    let personnel = store.adopt_or_create_account("paul@merand.fr", "gmail")?;
    for compte in [travail, personnel] {
        // La portée AVANT les boîtes : le fil Vantis contient notre
        // propre réponse, elle doit regrouper (ADR 0009).
        store.set_thread_scope(compte, Some("Envoyés"))?;
        dossiers(&store, compte)?;
    }
    // Un re-seed repart à neuf : les brouillons locaux d'une passe
    // précédente sont retirés SANS tombstone (rien à purger sur un
    // serveur qui n'existe pas).
    for brouillon in store.drafts()? {
        store.drop_stale_draft(brouillon.id)?;
    }

    // ——— Compte travail ————————————————————————————————————————————
    let inbox = boite(&mut store, travail, "INBOX")?;
    let envoyes = boite(&mut store, travail, "Envoyés")?;

    // Le fil Vantis : m1 (notre réponse, dans Envoyés) <- m2 <- m3.
    store.upsert_envelopes(
        envoyes,
        &[message(
            1,
            "Re : Relecture du contrat Vantis",
            "Paul Mérand",
            "paul.merand@atelier-nord.fr",
            "<vantis-m1@atelier-nord.fr>",
            None,
            quand(3, 18, 20),
            true,
        )],
    )?;
    store.save_body(
        envoyes,
        1,
        &corps(&["Merci Camille, je regarde ça ce soir et je te réponds demain matin."]),
        &[],
    )?;
    remplissage(&mut store, envoyes, 2, 11, "Message envoyé", 4, true)?;

    let mut reception = vec![
        message(
            1,
            "Facture 2026-0841 réglée",
            "Service comptabilité",
            "compta@atelier-nord.fr",
            "<clarity-c4@exemple.fr>",
            None,
            quand(6, 10, 12),
            true,
        ),
        message(
            2,
            "Atelier de septembre",
            "Sofia Nardi",
            "s.nardi@atelier-nord.fr",
            "<clarity-c5@exemple.fr>",
            None,
            quand(7, 9, 0),
            true,
        ),
        message(
            15,
            "Relecture du contrat Vantis",
            "Sofia Nardi",
            "s.nardi@atelier-nord.fr",
            "<vantis-m2@atelier-nord.fr>",
            Some("<vantis-m1@atelier-nord.fr>"),
            quand(2, 11, 5),
            true,
        ),
        message(
            16,
            "Planning de la semaine 33",
            "Yanis Belkacem",
            "y.belkacem@atelier-nord.fr",
            "<clarity-c2@exemple.fr>",
            None,
            quand(0, 8, 40),
            false,
        ),
        message(
            17,
            "Relecture du contrat Vantis",
            "Camille Rousseau",
            "c.rousseau@atelier-nord.fr",
            "<vantis-m3@atelier-nord.fr>",
            Some("<vantis-m2@atelier-nord.fr>"),
            quand(0, 9, 12),
            false,
        ),
    ];
    // 12 conversations lues de remplissage (uids 3..14).
    for n in 0..12u32 {
        reception.push(message(
            3 + n,
            &format!("Point d'étape n°{}", n + 1),
            "Atelier Nord",
            "contact@atelier-nord.fr",
            &format!("<clarity-filler-{n}@exemple.fr>"),
            None,
            quand(9 + i64::from(n), 14, 30),
            true,
        ));
    }
    store.upsert_envelopes(inbox, &reception)?;
    store.save_body(
        inbox,
        15,
        &corps(&[
            "J'ajoute la grille tarifaire mise à jour au fil ; elle remplace la version de juin.",
        ]),
        &[piece(
            0,
            "Annexe_tarifs.xlsx",
            "application/vnd.ms-excel",
            86_016,
        )],
    )?;
    store.save_body(
        inbox,
        16,
        &corps(&[
            "Bonjour Paul,",
            "Deux créneaux se chevauchent mardi après-midi. Je propose de décaler la relecture à 15h. Est-ce que ça te convient ?",
            "Yanis",
        ]),
        &[],
    )?;
    store.save_body(
        inbox,
        17,
        &corps(&[
            "Bonjour Paul,",
            "J'ai repris les articles 4 et 7 après notre échange de lundi. Il reste la clause de renouvellement à trancher : reconduction tacite de douze mois, ou renégociation annuelle. Les deux options sont annotées dans le document.",
            // Le seul corps du décor à LIEN — le geste « clic sur un
            // lien -> navigateur système » (terrain 2026-08-15) se joue
            // ici : l'iframe ne doit jamais naviguer.
            "Si tu peux me dire d'ici jeudi, je transmets la version finale au cabinet vendredi matin. La version annotée est lisible sur <a href=\"https://espace.exemple/vantis\">l'espace partagé</a>.",
            "Camille",
        ]),
        &[
            piece(0, "Contrat_Vantis_v4.pdf", "application/pdf", 1_258_291),
            piece(1, "Annexe_tarifs.xlsx", "application/vnd.ms-excel", 86_016),
        ],
    )?;

    // Le brouillon du décor vit en LOCAL (PLAN-BROUILLONS, B-D1) —
    // c'est lui que le dossier montre et que le clic reprend. La copie
    // IMAP reste : c'est le miroir qu'une poussée aurait laissé, et le
    // brouillon lui est relié (align + record) pour que le décor soit
    // l'état d'après reflet. Relié au fil Vantis (B-D2) : il répond au
    // dernier message de Camille (INBOX, uid 17) — la Réception
    // mentionne ce fil.
    let brouillons = boite(&mut store, travail, "Brouillons")?;
    store.upsert_envelopes(
        brouillons,
        &[message(
            1,
            "Re : Relecture du contrat Vantis",
            "Paul Mérand",
            "paul.merand@atelier-nord.fr",
            "<clarity-d1@exemple.fr>",
            None,
            quand(1, 22, 10),
            true,
        )],
    )?;
    let brouillon_vantis = store.save_draft(
        travail,
        None,
        None,
        DraftContent {
            to_raw: "c.rousseau@atelier-nord.fr",
            cc_raw: "",
            bcc_raw: "",
            subject: "Re : Relecture du contrat Vantis",
            body: "Bonjour Camille,\n\nMerci pour la v4 — je penche pour la reconduction \
                   tacite, avec un préavis porté à trois mois. Je te confirme jeudi, \
                   après un dernier échange avec Sofia.",
            reply_to_uid: Some(17),
            reply_to_mailbox: Some("INBOX"),
        },
    )?;
    store.align_drafts_uidvalidity(travail, UIDV)?;
    store.record_draft_pushed(brouillon_vantis.id, Some(1), brouillon_vantis.updated_epoch)?;
    let spam = boite(&mut store, travail, "Spam")?;
    store.upsert_envelopes(
        spam,
        &[
            message(
                1,
                "Vous avez gagné",
                "Loterie",
                "no-reply@exemple.org",
                "<clarity-s1@exemple.fr>",
                None,
                quand(2, 6, 0),
                false,
            ),
            message(
                2,
                "Offre imbattable",
                "Promotions",
                "promo@exemple.org",
                "<clarity-s2@exemple.fr>",
                None,
                quand(5, 6, 30),
                true,
            ),
        ],
    )?;
    let archives = boite(&mut store, travail, "Archives")?;
    store.upsert_envelopes(
        archives,
        &[message(
            1,
            "Version signée du contrat",
            "Cabinet Vantis",
            "contact@vantis.fr",
            "<clarity-a1@exemple.fr>",
            None,
            quand(14, 14, 0),
            true,
        )],
    )?;
    remplissage(&mut store, archives, 2, 39, "Dossier classé", 15, true)?;
    let corbeille = boite(&mut store, travail, "Corbeille")?;
    remplissage(&mut store, corbeille, 1, 2, "Ancien message", 20, true)?;

    // ——— Compte personnel ——————————————————————————————————————————
    let inbox_p = boite(&mut store, personnel, "INBOX")?;
    store.upsert_envelopes(
        inbox_p,
        &[
            message(
                1,
                "Compte rendu du 4 août",
                "Léa Fontaine",
                "l.fontaine@atelier-nord.fr",
                "<clarity-c3@exemple.fr>",
                None,
                quand(1, 16, 30),
                false,
            ),
            message(
                2,
                "Rappel : renouvellement du domaine",
                "Registrar",
                "no-reply@registrar.fr",
                "<clarity-c6@exemple.fr>",
                None,
                quand(8, 7, 15),
                false,
            ),
        ],
    )?;
    store.save_body(
        inbox_p,
        1,
        &corps(&[
            "Bonjour Paul,",
            "Trois décisions ont été actées lors de la réunion du 4 août, et une question reste ouverte sur le calendrier de livraison. Le compte rendu complet est en pièce jointe.",
            "Léa",
        ]),
        &[piece(0, "CR_04-08.pdf", "application/pdf", 225_280)],
    )?;
    // Le seul corps du décor à IMAGE DISTANTE (garde d'images, §6) — et
    // le seul aux accents en ENTITÉS HTML (nommées, décimales, hex),
    // comme les newsletters réelles : l'aperçu doit les décoder, jamais
    // les montrer.
    // Le texte VISIBLE reste celui du prototype — seul l'ENCODAGE
    // change : l'aperçu doit décoder, jamais montrer un résidu.
    store.save_body(
        inbox_p,
        2,
        "<p>Bonjour,</p>\n\
         <p>Le domaine atelier-nord.fr expire le 2&nbsp;septembre. Renouvelez-le \
         pour &eacute;viter toute interruption de&nbsp;service.</p>\n\
         <p>Support</p>\n\
         <img src=\"https://registrar.exemple/logo.png\" alt=\"Registrar\">",
        &[],
    )?;
    // Même mécanique que le compte travail — mais composition LIBRE
    // (pas de fil) : les deux formes du dossier sont au décor.
    let brouillons_p = boite(&mut store, personnel, "Brouillons")?;
    store.upsert_envelopes(
        brouillons_p,
        &[message(
            1,
            "Merci pour le compte rendu",
            "Paul Mérand",
            "paul@merand.fr",
            "<clarity-d2@exemple.fr>",
            None,
            quand(9, 18, 0),
            true,
        )],
    )?;
    let brouillon_cr = store.save_draft(
        personnel,
        None,
        None,
        DraftContent {
            to_raw: "l.fontaine@atelier-nord.fr",
            cc_raw: "",
            bcc_raw: "",
            subject: "Merci pour le compte rendu",
            body: "Bonjour Léa,\n\nBien reçu, merci — je relis le calendrier de \
                   livraison ce week-end et je te réponds lundi.",
            reply_to_uid: None,
            reply_to_mailbox: None,
        },
    )?;
    store.align_drafts_uidvalidity(personnel, UIDV)?;
    store.record_draft_pushed(brouillon_cr.id, Some(1), brouillon_cr.updated_epoch)?;
    let spam_p = boite(&mut store, personnel, "Spam")?;
    store.upsert_envelopes(
        spam_p,
        &[message(
            1,
            "Confirmez votre compte",
            "Support douteux",
            "support@exemple.org",
            "<clarity-s3@exemple.fr>",
            None,
            quand(3, 5, 45),
            false,
        )],
    )?;
    let archives_p = boite(&mut store, personnel, "Archives")?;
    remplissage(&mut store, archives_p, 1, 24, "Souvenir classé", 30, true)?;
    let corbeille_p = boite(&mut store, personnel, "Corbeille")?;
    remplissage(&mut store, corbeille_p, 1, 1, "Ancien message", 25, true)?;

    // Boîte d'envoi du compte personnel : vide — le prototype n'en montre
    // qu'une, portée par le compte travail.
    boite(&mut store, personnel, "Envoyés")?;

    // Une boîte vécue a une dernière relève : le prototype dit « il y a
    // 2 minutes », le décor la pose (PLAN-SYNCHRO E1). Relative au
    // lancement, comme les dates.
    let il_y_a_2_min = Utc::now().timestamp() - 120;
    store.set_text_pref("derniere_synchro", &il_y_a_2_min.to_string())?;

    println!("décor Clarity écrit dans {chemin}");
    Ok(())
}
