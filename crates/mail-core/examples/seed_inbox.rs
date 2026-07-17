//! Outil du gate Phase 1 : remplit une base avec N enveloppes synthétiques
//! pour mesurer la liste virtualisée (PLAN.md §4, ADR 0002). Sert aussi de
//! décor aux E2E : les 500 messages les plus récents reçoivent un corps,
//! pour que lire et citer se testent entièrement hors ligne.
//!
//! ```powershell
//! cargo run -p mail-core --example seed_inbox --release -- <chemin.db> [nombre]
//! ```
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

    let timer = Instant::now();
    let mut store = Store::open(std::path::Path::new(path))?;
    let mailbox_id = match store.sync_state("INBOX")? {
        Some(state) => {
            store.reset_mailbox(state.mailbox_id, SEED_UID_VALIDITY)?;
            state.mailbox_id
        }
        None => store.create_mailbox("INBOX", SEED_UID_VALIDITY)?,
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
            message_id: Some(format!("<seed-{uid}@exemple.fr>")),
            date: Utc
                .timestamp_opt(1_600_000_000 + i64::from(uid) * 60, 0)
                .single(),
            seen: uid % 3 != 0,
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
    let body_from = count.saturating_sub(500) + 1;
    for uid in body_from..=count {
        store.save_body(
            mailbox_id,
            uid,
            &format!("<p>Corps du message n°{uid} : contenu de démonstration.</p>"),
        )?;
    }
    store.update_state(mailbox_id, count, None)?;

    println!(
        "{count} enveloppes écrites dans {path} en {:?}",
        timer.elapsed()
    );
    Ok(())
}
