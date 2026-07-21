//! Validation de bout en bout : la boîte d'envoi de mail-core branchée sur
//! l'adaptateur SMTP réel, contre votre compte Gmail — envoi à soi-même.
//!
//! Prérequis : `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` dans l'environnement
//! et un compte déjà connecté via l'application Discovery (le refresh token
//! vit dans le Credential Manager, service « discovery-mail »).
//!
//! ```powershell
//! cargo run -p mail-smtp --example send_gmail --release
//! ```

use std::time::Instant;

use anyhow::Context;
use mail_auth::Authenticator;
use mail_core::{OutboxState, Store};
use mail_smtp::SmtpMailer;

fn main() -> anyhow::Result<()> {
    let auth = Authenticator::google_from_env().context("configuration OAuth")?;
    let account = match std::env::var("DISCOVERY_ACCOUNT") {
        Ok(email) => auth.authenticate_silent(&email),
        Err(_) => auth.authenticate_silent_legacy(),
    }
    .context("connectez d'abord un compte via Discovery (ou définissez DISCOVERY_ACCOUNT)")?;

    // Le chemin complet du produit : journaliser d'abord, envoyer ensuite.
    let db_path = std::path::PathBuf::from("target/mail-smtp-example.db");
    let mut store = Store::open(&db_path)?;
    let account_id = store.adopt_or_create_account(&account.email, "gmail")?;
    let draft = mail_core::compose(
        &account.email,
        &account.email,
        "Discovery — essai de la boîte d'envoi",
        "Ce message a transité par la boîte d'envoi persistante.\n\
         S'il arrive une seule fois, les deux règles d'or tiennent.",
        None,
    )?;
    store.enqueue_outbox(account_id, &draft)?;
    println!("Journalisé : {}", draft.message_id);

    let timer = Instant::now();
    let mut mailer =
        SmtpMailer::connect_xoauth2("smtp.gmail.com", 465, &account.email, &account.access_token)
            .map_err(|err| anyhow::anyhow!("connexion SMTP : {err}"))?;
    println!("Connecté ({}) en {:?}", account.email, timer.elapsed());

    let timer = Instant::now();
    let report = mail_core::flush_outbox(&mut mailer, &mut store, account_id)?;
    println!(
        "Vidange en {:?} : {} envoyé(s), {} reporté(s), {} refusé(s), {} en quarantaine",
        timer.elapsed(),
        report.sent,
        report.deferred,
        report.rejected,
        report.quarantined,
    );

    for message in store.outbox_in_state(OutboxState::Queued)? {
        println!(
            "Encore en file : {} ({})",
            message.subject, message.message_id
        );
    }
    Ok(())
}
