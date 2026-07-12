//! Rendu sécurisé des emails HTML — défense en profondeur validée en Phase 0
//! (PHASE0.md §1, spike html-render) :
//!
//! 1. `ammonia` retire scripts, handlers d'événements et URLs dangereuses ;
//! 2. les images distantes sont remplacées par un pixel neutre (vie privée :
//!    pas de pixel espion, pas de fuite d'adresse IP) ;
//! 3. [`email_document`] produit le document à afficher dans une iframe
//!    `sandbox` : sa CSP `default-src 'none'` garantit que même un
//!    contournement des couches 1-2 ne peut ni exécuter ni exfiltrer.
//!
//! Limite assumée (documentée par un test) : le filtrage CSS textuel est
//! contournable par échappement — c'est la couche 3 qui fait foi. Un vrai
//! parseur CSS (`lightningcss`) viendra pour la fidélité des blocs `<style>`.

mod sanitize;

pub use sanitize::{BLOCKED_PIXEL, Sanitized, sanitize};

/// Document complet à charger dans une iframe `sandbox` (via `srcdoc`) :
/// le modèle de production est « une CSP par message », embarquée dans le
/// document lui-même.
pub fn email_document(sanitized_html: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" \
         content=\"default-src 'none'; img-src data: cid:; style-src 'unsafe-inline'\">\
         <style>body{{font-family:system-ui,sans-serif;margin:12px;color:#222;\
         overflow-wrap:break-word}}</style>\
         </head><body>{sanitized_html}</body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_document_embeds_csp_and_content() {
        let document = email_document("<p>bonjour</p>");
        assert!(document.contains("default-src 'none'"));
        assert!(document.contains("<p>bonjour</p>"));
    }
}
