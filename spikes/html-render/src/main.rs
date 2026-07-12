//! Spike Phase 0 — rendu HTML sécurisé (problème dur n°3 du plan).
//!
//! Question à trancher : peut-on afficher les emails HTML du monde réel de
//! façon sûre (XSS neutralisé, aucun chargement distant) sans casser la mise
//! en page des newsletters ?
//!
//! Deux modes : `--sample` (corpus embarqué, hors-ligne) ou par défaut les
//! 20 derniers messages du compte Gmail (réutilise le refresh token stocké).
//! Code jetable : il valide des décisions, il ne rejoindra pas `mail-core` tel quel.

mod gmail;
mod report;
mod samples;
mod sanitize;

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Context;
use mail_parser::MessageParser;

const MESSAGE_COUNT: u32 = 20;

fn main() -> anyhow::Result<()> {
    let sample_mode = std::env::args().any(|arg| arg == "--sample");
    let out_dir = PathBuf::from("target/spike-html");
    std::fs::create_dir_all(&out_dir).context("création du dossier de sortie")?;

    let raw_messages: Vec<Vec<u8>> = if sample_mode {
        println!("Mode corpus embarqué (--sample) : 3 messages de test.");
        samples::SAMPLES
            .iter()
            .map(|s| s.as_bytes().to_vec())
            .collect()
    } else {
        fetch_recent_messages()?
    };

    let mut pages = Vec::new();
    let mut skipped_text_only = 0usize;
    for (index, raw) in raw_messages.iter().enumerate() {
        match process_message(&out_dir, index, raw)? {
            Some(page) => pages.push(page),
            None => skipped_text_only += 1,
        }
    }

    let index_path = report::write_index(&out_dir, &pages, skipped_text_only)?;
    print_summary(&pages, skipped_text_only, &index_path);
    Ok(())
}

fn fetch_recent_messages() -> anyhow::Result<Vec<Vec<u8>>> {
    let (mut session, email) = gmail::connect()?;
    println!("Connecté ({email}) — téléchargement des {MESSAGE_COUNT} derniers messages…");
    let mailbox = session.select("INBOX").context("SELECT INBOX")?;
    if mailbox.exists == 0 {
        anyhow::bail!("INBOX est vide : rien à rendre");
    }
    let first = mailbox.exists.saturating_sub(MESSAGE_COUNT - 1).max(1);
    let fetches = session
        .fetch(format!("{first}:{}", mailbox.exists), "(UID BODY.PEEK[])")
        .context("FETCH des corps de messages")?;
    let raw: Vec<Vec<u8>> = fetches
        .iter()
        .filter_map(|fetch| fetch.body().map(<[u8]>::to_vec))
        .collect();
    let _ = session.logout();
    Ok(raw)
}

/// Parse un message brut, assainit son HTML et écrit sa page de rapport.
/// Retourne `None` pour les messages texte-seul (rendus via `<pre>` exclu du
/// jugement de fidélité — ils ne posent aucun problème de rendu).
fn process_message(
    out_dir: &std::path::Path,
    index: usize,
    raw: &[u8],
) -> anyhow::Result<Option<report::Page>> {
    let Some(message) = MessageParser::new().parse(raw) else {
        println!("  message {index} : parsing impossible, ignoré");
        return Ok(None);
    };
    let subject = message.subject().unwrap_or("(sans sujet)").to_string();
    let sender = message
        .from()
        .and_then(|from| from.first())
        .and_then(|addr| addr.address())
        .unwrap_or("(expéditeur inconnu)")
        .to_string();

    let Some(html) = message.body_html(0).map(|body| body.into_owned()) else {
        return Ok(None);
    };

    let timer = Instant::now();
    let sanitized = sanitize::sanitize(&html);
    let sanitize_micros = timer.elapsed().as_micros();

    let file_name = report::write_message_page(out_dir, index, &subject, &sender, &sanitized.html)?;
    Ok(Some(report::Page {
        file_name,
        subject,
        sender,
        original_bytes: html.len(),
        sanitized_bytes: sanitized.html.len(),
        remote_images_blocked: sanitized.remote_images_blocked,
        styles_cleaned: sanitized.styles_cleaned,
        sanitize_micros,
    }))
}

fn print_summary(pages: &[report::Page], skipped_text_only: usize, index_path: &std::path::Path) {
    let total_images: usize = pages.iter().map(|p| p.remote_images_blocked).sum();
    let max_micros = pages.iter().map(|p| p.sanitize_micros).max().unwrap_or(0);
    println!("\n--- Bilan ---");
    println!(
        "{} message(s) HTML rendu(s), {} texte-seul ignoré(s)",
        pages.len(),
        skipped_text_only
    );
    println!("Images distantes bloquées : {total_images}");
    println!(
        "Assainissement le plus lent : {max_micros} µs (budget : fraction des 50 ms d'ouverture)"
    );
    println!("\nRapport visuel : {}", index_path.display());
    println!("Ouvrez-le dans un navigateur et jugez la fidélité de chaque message.");
}
