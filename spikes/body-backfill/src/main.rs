//! Banc de mesure : que coûte le rattrapage des corps de messages ?
//!
//! La validation terrain a montré que le corpus réel est quasi sans corps
//! (18 sur 537, 1 sur 2193) : la synchro « enveloppes d'abord »
//! (PLAN.md §3) ne télécharge un corps qu'au clic. La recherche
//! plein-texte ne porte donc, en pratique, que sur les sujets et les
//! expéditeurs — alors que l'ADR 0004 a tranché FTS5 sur un corpus AVEC
//! corps.
//!
//! Avant de décider quoi que ce soit (tout rapatrier ? les N derniers
//! mois ? au fil de l'eau ?), il faut les chiffres :
//!
//! - combien d'octets pour un corps, en moyenne, sur du courrier réel ;
//! - combien de temps par corps, et donc pour un compte entier ;
//! - de combien la base grossit — corps stockés ET index FTS.
//!
//! **Deux garde-fous.** Le banc travaille sur une COPIE de la base : une
//! mesure ne mute jamais l'état de production. Et il n'emprunte que l'API
//! publique du noyau (`load_body`), donc il mesure le vrai chemin du
//! produit, pas une imitation.
//!
//! ```powershell
//! cargo run --release -- "$env:APPDATA\dev.discovery.app\discovery.db" vous@exemple.com 200
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use mail_auth::GmailAuth;
use mail_core::Store;

const MAILBOX: &str = "INBOX";
const IMAP_HOST_GMAIL: &str = "imap.gmail.com";
const IMAP_PORT: u16 = 993;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let source = args.next().context(
        "usage : <chemin.db> <email> [nombre]\n\
         (fermez l'application avant de lancer le banc)",
    )?;
    let email = args.next().context("email du compte à mesurer")?;
    let sample: usize = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(200);

    // Copie de travail : la base réelle n'est jamais touchée.
    let work = working_copy(Path::new(&source))?;
    println!("base source : {source}");
    println!("copie de travail : {}\n", work.display());

    let mut store = Store::open(&work)?;
    let account = store
        .accounts()?
        .into_iter()
        .find(|candidate| candidate.email == email)
        .with_context(|| format!("compte {email} introuvable dans la base"))?;

    // Les enveloppes sans corps, les plus récentes d'abord : c'est
    // l'ordre dans lequel un rattrapage réel procéderait.
    let envelopes = store.recent(account.id, MAILBOX, 0, 100_000)?;
    let total = envelopes.len();
    let mut pending: Vec<u32> = Vec::new();
    for envelope in &envelopes {
        if store.body(account.id, MAILBOX, envelope.uid)?.is_none() {
            pending.push(envelope.uid);
        }
    }
    let missing = pending.len();
    pending.truncate(sample);
    println!(
        "compte {} ({}) : {total} messages, {missing} sans corps — échantillon de {}\n",
        account.email,
        account.provider,
        pending.len()
    );
    if pending.is_empty() {
        println!("rien à mesurer.");
        return Ok(());
    }

    let before = file_size(&work)?;
    let mut server = connect(&account)?;

    let timer = Instant::now();
    let mut bytes = 0usize;
    let mut fetched = 0usize;
    let mut failures = 0usize;
    for (index, uid) in pending.iter().enumerate() {
        match mail_core::load_body(&mut server, &mut store, account.id, MAILBOX, *uid) {
            Ok(Some(html)) => {
                bytes += html.len();
                fetched += 1;
            }
            Ok(None) => failures += 1,
            Err(err) => {
                failures += 1;
                eprintln!("  uid {uid} : {err}");
            }
        }
        if (index + 1).is_multiple_of(25) {
            println!(
                "  {} / {} corps — {:?}",
                index + 1,
                pending.len(),
                timer.elapsed()
            );
        }
    }
    let elapsed = timer.elapsed();
    drop(store);
    let after = file_size(&work)?;

    report(Report {
        total,
        missing,
        fetched,
        failures,
        bytes,
        elapsed_ms: elapsed.as_millis(),
        grown: after.saturating_sub(before),
        before,
        after,
    });
    println!(
        "\nLa copie de travail peut être supprimée : {}",
        work.display()
    );
    Ok(())
}

struct Report {
    total: usize,
    missing: usize,
    fetched: usize,
    failures: usize,
    bytes: usize,
    elapsed_ms: u128,
    grown: u64,
    before: u64,
    after: u64,
}

fn report(r: Report) {
    let mo = |octets: f64| octets / 1_048_576.0;
    println!("\n=== Mesures ===");
    println!("corps téléchargés     : {} (échecs : {})", r.fetched, r.failures);
    if r.fetched == 0 {
        return;
    }
    let per_body_bytes = r.bytes as f64 / r.fetched as f64;
    let per_body_ms = r.elapsed_ms as f64 / r.fetched as f64;
    // Ce qui compte n'est pas l'octet transféré mais ce que la base
    // grossit : corps stockés + entrées d'index FTS.
    let per_body_disk = r.grown as f64 / r.fetched as f64;

    println!("octets de HTML        : {:.1} Mo", mo(r.bytes as f64));
    println!("taille moyenne / corps: {:.1} Ko", per_body_bytes / 1024.0);
    println!("durée totale          : {:.1} s", r.elapsed_ms as f64 / 1000.0);
    println!("durée moyenne / corps : {per_body_ms:.0} ms");
    println!(
        "base : {:.1} Mo -> {:.1} Mo  (+{:.1} Mo, soit {:.1} Ko par corps)",
        mo(r.before as f64),
        mo(r.after as f64),
        mo(r.grown as f64),
        per_body_disk / 1024.0
    );

    println!("\n=== Extrapolation pour CE compte ===");
    let remaining = r.missing.saturating_sub(r.fetched) as f64;
    println!(
        "{} messages restent sans corps :",
        r.missing.saturating_sub(r.fetched)
    );
    println!(
        "  temps estimé  : {:.1} min",
        remaining * per_body_ms / 60_000.0
    );
    println!(
        "  disque estimé : {:.0} Mo (base finale ~{:.0} Mo)",
        mo(remaining * per_body_disk),
        mo(r.after as f64 + remaining * per_body_disk)
    );
    println!("(sur {} messages au total dans la boîte)", r.total);
}

/// Copie la base à côté d'elle, suffixée `-banc`. Pas de WAL à gérer :
/// le stockage n'active pas le mode WAL.
fn working_copy(source: &Path) -> Result<PathBuf> {
    if !source.exists() {
        bail!("base introuvable : {}", source.display());
    }
    let mut target = source.to_path_buf();
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("discovery");
    target.set_file_name(format!("{stem}-banc.db"));
    let _ = std::fs::remove_file(&target);
    std::fs::copy(source, &target).context("copie de la base")?;
    Ok(target)
}

fn file_size(path: &Path) -> Result<u64> {
    Ok(std::fs::metadata(path)?.len())
}

/// Reconnexion silencieuse, exactement comme l'application : jeton OAuth
/// pour Gmail, mot de passe du coffre pour un compte générique.
fn connect(account: &mail_core::Account) -> Result<mail_imap::ImapServer> {
    match account.provider.as_str() {
        "gmail" => {
            let auth = GmailAuth::from_env()
                .map_err(|err| anyhow::anyhow!("{err}"))
                .context("GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET doivent être définies")?;
            let session = auth
                .authenticate_silent(&account.email)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            Ok(mail_imap::ImapServer::connect_xoauth2(
                IMAP_HOST_GMAIL,
                IMAP_PORT,
                &session.email,
                &session.access_token,
            )?)
        }
        "imap" => {
            bail!(
                "compte générique : le banc ne lit pas encore sa configuration serveur.\n\
                 Mesurez d'abord un compte Gmail — c'est là que se trouve le volume."
            )
        }
        other => bail!("fournisseur inconnu : {other}"),
    }
}
