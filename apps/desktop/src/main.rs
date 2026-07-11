use mail_core::EmailAddress;

/// Coquille de l'application desktop : deviendra l'app Tauri en Phase 1.
fn main() -> anyhow::Result<()> {
    let sample = EmailAddress::parse("demo@example.com")?;
    println!("discovery desktop — noyau mail-core relié ({sample})");
    Ok(())
}
