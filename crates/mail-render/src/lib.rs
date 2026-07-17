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

pub use sanitize::{BLOCKED_PIXEL, ImagePolicy, Sanitized, sanitize, sanitize_with};

/// Corps d'un message réduit à son texte — la matière première d'une
/// citation (répondre, transférer — Phase 2).
///
/// Assainissement D'ABORD : `ammonia` élimine scripts et styles avec leur
/// contenu, pour qu'aucun code ne puisse se déguiser en texte cité. La
/// conversion est déléguée à `mail-parser` (décision Phase 0), qui ne coupe
/// que sur `<p>` et `<br>` (mesuré) : une pré-passe traduit donc les fins de
/// blocs en `<br>` — heuristique d'affichage sur du HTML déjà assaini, pas
/// du parsing de sécurité.
pub fn body_text(html: &str) -> String {
    let sanitized = sanitize(html);
    let text = mail_parser::decoders::html::html_to_text(&block_ends_to_breaks(&sanitized.html));
    collapse_blank_lines(text.trim())
}

/// Fins de blocs → sauts de ligne, cellules de tableau → espaces.
/// `ammonia` émet des balises en minuscules : la casse est déjà normalisée.
fn block_ends_to_breaks(html: &str) -> String {
    const BLOCK_ENDS: [&str; 12] = [
        "</div>",
        "</tr>",
        "</li>",
        "</blockquote>",
        "</h1>",
        "</h2>",
        "</h3>",
        "</h4>",
        "</h5>",
        "</h6>",
        "</table>",
        "</ul>",
    ];
    let mut result = html.replace("</td>", "</td> ").replace("</th>", "</th> ");
    for tag in BLOCK_ENDS {
        result = result.replace(tag, &format!("{tag}<br>"));
    }
    result
}

/// Jamais plus d'une ligne vide d'affilée : les emboîtements de blocs
/// produisent des rafales de sauts sans valeur pour une citation.
fn collapse_blank_lines(text: &str) -> String {
    let mut lines = Vec::new();
    let mut previous_blank = false;
    for line in text.lines() {
        let line = line.trim_end();
        let blank = line.is_empty();
        if blank && previous_blank {
            continue;
        }
        lines.push(line);
        previous_blank = blank;
    }
    lines.join("\n")
}

/// Document complet à charger dans une iframe `sandbox` (via `srcdoc`) :
/// le modèle de production est « une CSP par message », embarquée dans le
/// document lui-même. La CSP suit la politique d'images : elle n'ouvre
/// `https:` que si l'utilisateur a demandé les images distantes.
///
/// **Contrainte d'hébergement (prouvée par l'expérience, 2026-07-12)** : un
/// document `srcdoc` hérite de la CSP de la page hôte, et une CSP ne peut que
/// se resserrer. L'hôte doit donc autoriser `img-src data: https: http:` et
/// `style-src 'unsafe-inline'` — c'est CE document qui reste la couche
/// restrictive par message (images distantes bloquées par défaut).
pub fn email_document(sanitized_html: &str, policy: ImagePolicy) -> String {
    let img_sources = match policy {
        ImagePolicy::BlockRemote => "data: cid:",
        ImagePolicy::AllowRemote => "data: cid: https: http:",
    };
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" \
         content=\"default-src 'none'; img-src {img_sources}; style-src 'unsafe-inline'\">\
         <style>body{{font-family:system-ui,sans-serif;margin:12px;color:#222;\
         overflow-wrap:break-word}}</style>\
         </head><body>{sanitized_html}</body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_text_strips_tags_and_decodes_entities() {
        let text = body_text("<p>Bonjour &amp; bienvenue&nbsp;!</p><p>À demain.</p>");
        assert!(text.contains("Bonjour & bienvenue"));
        assert!(text.contains("À demain."));
        assert!(!text.contains('<'));
    }

    #[test]
    fn body_text_separates_block_elements_with_line_breaks() {
        let text = body_text("<div>ligne 1</div><div>ligne 2</div>");
        assert_eq!(text.lines().count(), 2, "{text:?}");
    }

    /// Les newsletters sont des soupes de tableaux : les cellules doivent
    /// rester séparées, les lignes aussi.
    #[test]
    fn body_text_keeps_table_structure_readable() {
        let text = body_text(
            "<table><tr><td>gauche</td><td>droite</td></tr><tr><td>bas</td></tr></table>",
        );
        assert!(text.contains("gauche droite"), "{text:?}");
        assert!(text.lines().count() >= 2, "{text:?}");
    }

    #[test]
    fn body_text_never_stacks_blank_lines() {
        let text = body_text("<div><p>haut</p></div><div></div><div><p>bas</p></div>");
        assert!(!text.contains("\n\n\n"), "{text:?}");
    }

    /// Le contenu d'un script ne doit jamais se retrouver dans une citation.
    #[test]
    fn body_text_drops_script_content_entirely() {
        let text = body_text("<p>visible</p><script>alert('caché')</script>");
        assert!(text.contains("visible"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("caché"));
    }

    #[test]
    fn email_document_embeds_csp_and_content() {
        let document = email_document("<p>bonjour</p>", ImagePolicy::BlockRemote);
        assert!(document.contains("default-src 'none'"));
        assert!(document.contains("img-src data: cid:;"));
        assert!(document.contains("<p>bonjour</p>"));
    }

    #[test]
    fn email_document_opens_https_images_only_on_request() {
        let document = email_document("<p>x</p>", ImagePolicy::AllowRemote);
        assert!(document.contains("img-src data: cid: https: http:;"));
        assert!(document.contains("default-src 'none'"));
    }
}
