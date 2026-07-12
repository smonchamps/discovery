//! Pipeline d'assainissement HTML pour emails — défense en profondeur :
//!
//! 1. `ammonia` retire scripts, handlers d'événements et URLs dangereuses ;
//! 2. les images distantes sont remplacées par un pixel neutre (vie privée :
//!    pas de pixel espion, pas de fuite d'adresse IP) ;
//! 3. l'affichage se fait dans une iframe `sandbox` dont le document embarque
//!    une CSP `default-src 'none'` — même si une astuce d'échappement passait
//!    les couches 1-2, rien ne peut s'exécuter ni se charger.

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// GIF 1×1 gris : remplace chaque image distante bloquée.
pub const BLOCKED_PIXEL: &str =
    "data:image/gif;base64,R0lGODlhAQABAIAAAMLCwgAAACH5BAAAAAAALAAAAAABAAEAAAICRAEAOw==";

/// Sort des images distantes. Le blocage est le défaut non négociable ;
/// l'affichage est un choix explicite de l'utilisateur, par message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePolicy {
    BlockRemote,
    AllowRemote,
}

pub struct Sanitized {
    pub html: String,
    pub remote_images_blocked: usize,
    pub styles_cleaned: usize,
}

pub fn sanitize(html: &str) -> Sanitized {
    sanitize_with(html, ImagePolicy::BlockRemote)
}

pub fn sanitize_with(html: &str, policy: ImagePolicy) -> Sanitized {
    let remote_images = Arc::new(AtomicUsize::new(0));
    let styles_cleaned = Arc::new(AtomicUsize::new(0));
    let images_counter = Arc::clone(&remote_images);
    let styles_counter = Arc::clone(&styles_cleaned);

    let clean = ammonia::Builder::default()
        .add_tags(["font"])
        .add_tag_attributes("font", ["color", "face", "size"])
        .add_generic_attributes([
            "style",
            "width",
            "height",
            "align",
            "valign",
            "bgcolor",
            "border",
            "cellpadding",
            "cellspacing",
        ])
        .url_schemes(HashSet::from([
            "http", "https", "mailto", "tel", "cid", "data",
        ]))
        .attribute_filter(move |element, attribute, value| {
            filter_attribute(
                element,
                attribute,
                value,
                policy,
                &images_counter,
                &styles_counter,
            )
        })
        .clean(html)
        .to_string();

    Sanitized {
        html: clean,
        remote_images_blocked: remote_images.load(Ordering::Relaxed),
        styles_cleaned: styles_cleaned.load(Ordering::Relaxed),
    }
}

fn filter_attribute<'a>(
    element: &str,
    attribute: &str,
    value: &'a str,
    policy: ImagePolicy,
    remote_images: &AtomicUsize,
    styles_cleaned: &AtomicUsize,
) -> Option<Cow<'a, str>> {
    if element == "img" && attribute == "src" {
        let lower = value.trim().to_ascii_lowercase();
        let remote = lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("//");
        if remote && policy == ImagePolicy::BlockRemote {
            remote_images.fetch_add(1, Ordering::Relaxed);
            return Some(Cow::Borrowed(BLOCKED_PIXEL));
        }
        return Some(Cow::Borrowed(value));
    }
    // `data:` est autorisé pour les images, pas pour les liens (phishing).
    if attribute == "href" && value.trim_start().to_ascii_lowercase().starts_with("data:") {
        return None;
    }
    if attribute == "style" {
        let cleaned = clean_style(value);
        if cleaned.len() != value.len() {
            styles_cleaned.fetch_add(1, Ordering::Relaxed);
        }
        return Some(Cow::Owned(cleaned));
    }
    Some(Cow::Borrowed(value))
}

/// Filtrage CSS par déclaration : supprime tout chargement ou exécution.
/// Volontairement naïf (les échappements CSS type `\75rl(` passeraient) :
/// c'est la CSP de l'iframe qui sert de filet de sécurité (doc du crate).
/// La fidélité des blocs `<style>` viendra avec un vrai parseur CSS.
fn clean_style(value: &str) -> String {
    value
        .split(';')
        .filter(|declaration| {
            let compact: String = declaration
                .to_ascii_lowercase()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            !(compact.contains("url(")
                || compact.contains("expression(")
                || compact.contains("@import")
                || compact.contains("behavior:"))
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_script_tags_and_their_content() {
        let out = sanitize("<p>contenu</p><script>alert(1)</script>");
        assert!(!out.html.contains("script"));
        assert!(!out.html.contains("alert"));
        assert!(out.html.contains("contenu"));
    }

    #[test]
    fn removes_event_handlers() {
        let out = sanitize(r#"<img src="data:image/gif;base64,AA==" onerror="alert(1)">"#);
        assert!(!out.html.contains("onerror"));
        assert!(!out.html.contains("alert"));
    }

    #[test]
    fn removes_javascript_links() {
        let out = sanitize(r#"<a href="javascript:alert(1)">cliquer</a>"#);
        assert!(!out.html.contains("javascript:"));
        assert!(out.html.contains("cliquer"));
    }

    #[test]
    fn blocks_remote_images_with_neutral_pixel() {
        let out = sanitize(r#"<img src="https://tracker.example.com/pixel.gif" width="1">"#);
        assert_eq!(out.remote_images_blocked, 1);
        assert!(out.html.contains(BLOCKED_PIXEL));
        assert!(!out.html.contains("tracker.example.com"));
    }

    #[test]
    fn allow_remote_keeps_images_but_still_strips_scripts() {
        let out = sanitize_with(
            r#"<img src="https://cdn.example.com/photo.jpg"><script>alert(1)</script>"#,
            ImagePolicy::AllowRemote,
        );
        assert_eq!(out.remote_images_blocked, 0);
        assert!(out.html.contains("https://cdn.example.com/photo.jpg"));
        assert!(!out.html.contains("script"));
    }

    #[test]
    fn keeps_inline_and_data_images() {
        let out = sanitize(r#"<img src="data:image/png;base64,AA==">"#);
        assert_eq!(out.remote_images_blocked, 0);
        assert!(out.html.contains("data:image/png"));
    }

    #[test]
    fn strips_css_url_loads_but_keeps_layout_declarations() {
        let out = sanitize(
            r#"<div style="background-image: url('https://x.example/bg.png'); padding: 4px">x</div>"#,
        );
        assert!(!out.html.contains("x.example"));
        assert!(out.html.contains("padding: 4px"));
        assert_eq!(out.styles_cleaned, 1);
    }

    #[test]
    fn strips_css_url_with_surrounding_whitespace() {
        let out = sanitize("<div style=\"background:\n\t url( 'https://x.example/a' )\">x</div>");
        assert!(!out.html.contains("x.example"));
    }

    /// Limite connue et assumée : un échappement CSS (`\75rl(` = `url(`)
    /// traverse le filtre naïf. Ce test documente pourquoi la couche 3
    /// (CSP `default-src 'none'` dans l'iframe) n'est pas optionnelle.
    #[test]
    fn css_escape_bypass_passes_the_naive_filter_csp_is_the_backstop() {
        let out = sanitize(r#"<div style="background:\75rl(https://x.example/a)">x</div>"#);
        assert!(out.html.contains("x.example"));
    }

    #[test]
    fn removes_data_links_but_not_data_images() {
        let out = sanitize(r#"<a href="data:text/html;base64,PHNjcmlwdD4=">x</a>"#);
        assert!(!out.html.contains("href"));
    }

    #[test]
    fn keeps_table_layout_used_by_newsletters() {
        let out = sanitize(
            r##"<table width="600" bgcolor="#ffffff" cellpadding="0"><tbody><tr><td align="center" style="color: #333">contenu</td></tr></tbody></table>"##,
        );
        assert!(out.html.contains(r#"width="600""#));
        assert!(out.html.contains(r#"align="center""#));
        assert!(out.html.contains("color: #333"));
    }
}
