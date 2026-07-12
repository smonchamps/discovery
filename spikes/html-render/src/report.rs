//! Génère le rapport visuel : une page par message (iframe sandbox + CSP
//! embarquée dans le document de l'email) et un index avec les statistiques.

use std::path::{Path, PathBuf};

use anyhow::Context;

pub struct Page {
    pub file_name: String,
    pub subject: String,
    pub sender: String,
    pub original_bytes: usize,
    pub sanitized_bytes: usize,
    pub remote_images_blocked: usize,
    pub styles_cleaned: usize,
    pub sanitize_micros: u128,
}

pub fn write_message_page(
    dir: &Path,
    index: usize,
    subject: &str,
    sender: &str,
    sanitized_html: &str,
) -> anyhow::Result<String> {
    // La CSP vit DANS le document de l'email : c'est le modèle de production
    // (une webview par message avec sa propre politique), et l'attribut
    // `sandbox` sans permission interdit déjà toute exécution de script.
    let email_document = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" \
         content=\"default-src 'none'; img-src data: cid:; style-src 'unsafe-inline'\">\
         </head><body>{sanitized_html}</body></html>"
    );
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{subject}</title><style>\
         body{{font-family:system-ui;margin:0}}\
         header{{padding:12px 16px;background:#f4f4f4;border-bottom:1px solid #ddd}}\
         iframe{{border:0;width:100%;height:calc(100vh - 80px)}}\
         </style></head><body>\
         <header><strong>{subject}</strong><br><small>{sender} — \
         <a href=\"index.html\">retour à l'index</a></small></header>\
         <iframe sandbox srcdoc=\"{srcdoc}\"></iframe>\
         </body></html>",
        subject = escape(subject),
        sender = escape(sender),
        srcdoc = escape(&email_document),
    );
    let file_name = format!("msg_{index:02}.html");
    std::fs::write(dir.join(&file_name), page).context("écriture de la page message")?;
    Ok(file_name)
}

pub fn write_index(
    dir: &Path,
    pages: &[Page],
    skipped_text_only: usize,
) -> anyhow::Result<PathBuf> {
    let total_images: usize = pages.iter().map(|p| p.remote_images_blocked).sum();
    let max_micros = pages.iter().map(|p| p.sanitize_micros).max().unwrap_or(0);
    let mut rows = String::new();
    for page in pages {
        rows.push_str(&format!(
            "<tr><td><a href=\"{file}\">{subject}</a></td><td>{sender}</td>\
             <td>{orig} → {san}</td><td>{imgs}</td><td>{styles}</td><td>{micros} µs</td></tr>",
            file = page.file_name,
            subject = escape(&page.subject),
            sender = escape(&page.sender),
            orig = kb(page.original_bytes),
            san = kb(page.sanitized_bytes),
            imgs = page.remote_images_blocked,
            styles = page.styles_cleaned,
            micros = page.sanitize_micros,
        ));
    }
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Spike rendu HTML</title><style>\
         body{{font-family:system-ui;margin:24px;max-width:1000px}}\
         table{{border-collapse:collapse;width:100%}}\
         td,th{{border:1px solid #ddd;padding:6px 10px;text-align:left;font-size:14px}}\
         th{{background:#f4f4f4}}\
         </style></head><body>\
         <h1>Spike Phase 0 — rendu HTML sécurisé</h1>\
         <p>{count} message(s) rendu(s), {skipped} texte-seul ignoré(s), \
         {total_images} image(s) distante(s) bloquée(s), assainissement max {max_micros} µs.</p>\
         <p>Cliquez chaque message et jugez la fidélité visuelle par rapport à Gmail.</p>\
         <table><tr><th>Sujet</th><th>Expéditeur</th><th>Taille</th>\
         <th>Img bloquées</th><th>Styles filtrés</th><th>Durée</th></tr>{rows}</table>\
         </body></html>",
        count = pages.len(),
        skipped = skipped_text_only,
    );
    let path = dir.join("index.html");
    std::fs::write(&path, html).context("écriture de l'index")?;
    Ok(path)
}

fn kb(bytes: usize) -> String {
    format!("{} Ko", bytes.div_ceil(1024))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
