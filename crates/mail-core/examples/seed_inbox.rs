//! Outil du gate Phase 1 : remplit une base avec N enveloppes synthétiques
//! pour mesurer la liste virtualisée (PLAN.md §4, ADR 0002). Sert aussi de
//! décor aux E2E : les 500 messages les plus récents reçoivent un corps,
//! pour que lire et citer se testent entièrement hors ligne.
//!
//! ```powershell
//! cargo run -p mail-core --example seed_inbox --release -- <chemin.db> [nombre] [email] [corps] [ko_par_corps]
//! ```
//!
//! `email` (défaut : seed@exemple.fr) désigne le compte qui possède la
//! boîte — appeler l'outil deux fois avec deux emails peuple une base
//! multi-comptes (décor de l'E2E boîte unifiée).
//!
//! `corps` (défaut : 500) et `ko_par_corps` (défaut : 0, c'est-à-dire le
//! corps de démonstration minuscule) servent au **gate 3** : le budget
//! disque (< 1 Go) et la recherche ne se mesurent pas sur des corps de
//! soixante octets. L'[ADR 0007] a mesuré **~34 Ko par corps stocké** sur
//! une boîte réelle ; c'est la valeur à passer ici.
//!
//! Les défauts reproduisent exactement le décor d'avant : les E2E et les
//! mesures de Phase 1 restent comparables.
//!
//! Attention : la boîte INBOX de la base visée est remplacée. L'UIDVALIDITY
//! synthétique (424242) garantit qu'une future synchro réelle repartira
//! proprement de zéro.

use std::time::Instant;

use chrono::{TimeZone, Utc};
use mail_core::{Envelope, Store};

const SENDERS: [&str; 8] = [
    "Alice Martin",
    "La Gazette",
    "GitHub",
    "Bob Dupont",
    "Newsletter Cuisine",
    "Service Client",
    "Équipe Produit",
    "Charlotte Bernard",
];
const TOPICS: [&str; 6] = [
    "Les nouveautés de la semaine",
    "Votre facture est disponible",
    "Réunion de suivi — compte rendu",
    "Promotion d'été : derniers jours",
    "Rapport hebdomadaire d'activité",
    "Confirmation de votre commande",
];
const SEED_UID_VALIDITY: u32 = 424_242;
const BATCH: usize = 1_000;

/// De quoi remplir un corps sans qu'il ressemble à du bruit.
const MOTS: [&str; 32] = [
    "bonjour",
    "facture",
    "réunion",
    "projet",
    "livraison",
    "devis",
    "client",
    "équipe",
    "rapport",
    "budget",
    "contrat",
    "échéance",
    "validation",
    "commande",
    "réponse",
    "document",
    "semaine",
    "service",
    "message",
    "dossier",
    "produit",
    "atelier",
    "compte",
    "détail",
    "demande",
    "facturation",
    "planning",
    "relance",
    "remise",
    "accord",
    "suivi",
    "note",
];

/// Un corps synthétique d'environ `ko` kilooctets.
///
/// **Une longue traîne, pas un plateau.** Un corps de 34 Ko fait ~5 000
/// mots : tirés dans un vocabulaire de trente mots, chacun se retrouverait
/// dans la totalité des messages, et la recherche ne mesurerait plus que
/// le cas dégénéré que l'[ADR 0007] §4 signale déjà (« 90 % du corpus
/// matche »). Un jeton rare sur cinq, pris dans un espace de 20 000,
/// redonne au corpus la forme d'un vrai courrier : beaucoup de termes que
/// peu de messages portent.
///
/// Déterministe : le même `uid` produit le même corps, donc deux mesures
/// se comparent.
fn corps_synthetique(uid: u32, ko: usize) -> String {
    let cible = ko * 1024;
    let mut sortie = String::with_capacity(cible + 64);
    sortie.push_str("<p>");
    let mut graine = u64::from(uid)
        .wrapping_mul(2_862_933_555_777_941_757)
        .wrapping_add(3_037_000_493);
    while sortie.len() < cible {
        graine = graine
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let tirage = (graine >> 33) as usize;
        if tirage.is_multiple_of(5) {
            sortie.push_str(&format!("ref{} ", tirage % 20_000));
        } else {
            sortie.push_str(MOTS[tirage % MOTS.len()]);
            sortie.push(' ');
        }
    }
    sortie.push_str("</p>");
    sortie
}

fn main() -> Result<(), mail_core::Error> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("target/seed-inbox.db");
    let count: u32 = args
        .get(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(50_000);

    let email = args.get(3).map(String::as_str).unwrap_or("seed@exemple.fr");
    let corps: u32 = args
        .get(4)
        .and_then(|value| value.parse().ok())
        .unwrap_or(500);
    let ko_par_corps: usize = args
        .get(5)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    // La boite a peupler. « Envoyes » sert a fabriquer le decor que
    // l ADR 0009 §4 vise sans qu on ait jamais pu l eprouver : des fils
    // PUREMENT SORTANTS, que l index partiel doit exclure.
    let boite = args.get(6).map(String::as_str).unwrap_or("INBOX");

    let timer = Instant::now();
    let mut store = Store::open(std::path::Path::new(path))?;
    let account = store.adopt_or_create_account(email, "gmail")?;
    let mailbox_id = match store.sync_state(account, boite)? {
        Some(state) => {
            store.reset_mailbox(state.mailbox_id, SEED_UID_VALIDITY)?;
            state.mailbox_id
        }
        None => store.create_mailbox(account, boite, SEED_UID_VALIDITY)?,
    };

    let mut batch = Vec::with_capacity(BATCH);
    for uid in 1..=count {
        let index = uid as usize;
        batch.push(Envelope {
            uid,
            subject: Some(format!("{} n°{uid}", TOPICS[index % TOPICS.len()])),
            sender: Some(SENDERS[(index * 7) % SENDERS.len()].to_string()),
            sender_address: Some(format!(
                "expediteur{}@exemple.fr",
                (index * 7) % SENDERS.len()
            )),
            message_id: Some(format!("<seed-{boite}-{uid}@exemple.fr>")),
            // Un message sur cinq répond au précédent : sans vraie
            // conversation dans le jeu d'essai, un regroupement cassé
            // passerait tous les tests.
            in_reply_to: (uid % 5 == 0 && uid > 1)
                .then(|| format!("<seed-{boite}-{}@exemple.fr>", uid - 1)),
            date: Utc
                .timestamp_opt(1_600_000_000 + i64::from(uid) * 60, 0)
                .single(),
            seen: uid % 3 != 0,
            flagged: uid.is_multiple_of(7),
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        });
        if batch.len() == BATCH {
            store.upsert_envelopes(mailbox_id, &batch)?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        store.upsert_envelopes(mailbox_id, &batch)?;
    }

    // Un corps pour les plus récents seulement : suffisant pour les E2E,
    // sans alourdir l'outil de mesure quand on seed 50 000 messages.
    let body_from = count.saturating_sub(corps) + 1;
    for uid in body_from..=count {
        // Un message sur dix porte une pièce jointe : de quoi exercer la
        // liste ET son absence, sans avoir à distinguer deux décors.
        let attachments: Vec<mail_core::Attachment> = if uid % 10 == 0 {
            vec![mail_core::Attachment {
                index: 0,
                name: format!("facture-{uid}.pdf"),
                mime: "application/pdf".to_string(),
                size: 20_480,
            }]
        } else {
            Vec::new()
        };
        let html = if ko_par_corps == 0 {
            format!("<p>Corps du message n°{uid} : contenu de démonstration.</p>")
        } else {
            corps_synthetique(uid, ko_par_corps)
        };
        store.save_body(mailbox_id, uid, &html, &attachments)?;
    }
    // Dossiers de destination : le déplacement se joue entièrement en
    // local (cache + journal), donc l'E2E peut l'exercer hors ligne.
    // « Archiv&AOk-s » est en UTF-7 modifié — le décodage doit se voir.
    store.replace_folders(
        account,
        &[
            mail_core::Folder {
                wire: "Archiv&AOk-s".to_string(),
                display: "Archivés".to_string(),
                selectable: true,
            },
            mail_core::Folder {
                wire: "Factures".to_string(),
                display: "Factures".to_string(),
                selectable: true,
            },
        ],
    )?;
    store.update_state(mailbox_id, count, None)?;

    println!(
        "{count} enveloppes écrites dans {path} en {:?}",
        timer.elapsed()
    );
    Ok(())
}
