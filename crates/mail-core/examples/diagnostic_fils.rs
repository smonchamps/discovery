//! Diagnostic du regroupement en conversations.
//!
//! Répond à deux questions que seule la vraie boîte peut trancher :
//!
//! 1. la passe d'en-têtes a-t-elle tourné, et qu'a-t-elle trouvé ?
//! 2. **quel identifiant** réunit les messages d'un fil anormalement gros ?
//!
//! Même discipline que [`diagnostic_index`] : aucun sujet, aucun
//! expéditeur, aucun contenu n'est lu ni affiché. Les identifiants
//! techniques sont **masqués** — on n'en montre que la forme (chevrons,
//! longueur, domaine), qui suffit à désigner le défaut.
//!
//! ```powershell
//! cargo run -p mail-core --example diagnostic_fils -- "$env:APPDATA\dev.discovery.app\discovery.db"
//! ```

use rusqlite::{Connection, OptionalExtension};

/// Ne montre que la FORME d'un identifiant : chevrons présents ou non,
/// longueur de la partie locale, domaine. De quoi reconnaître un
/// `Message-ID` réutilisé, vide ou hors norme sans en divulguer un seul.
fn shape(raw: &str) -> String {
    forme(raw, true)
}

/// Forme d'un jeton d'ANNUAIRE.
///
/// Contrairement à [`shape`], on ne dit rien des chevrons : l'annuaire ne
/// stocke que la forme canonique, qui les a déjà retirés. Les mentionner
/// ferait lire « SANS CHEVRONS » sur des identifiants parfaitement
/// normaux — un faux signal d'alarme, exactement ce qu'un diagnostic ne
/// doit pas produire.
fn shape_canonical(raw: &str) -> String {
    forme(raw, false)
}

fn forme(raw: &str, montrer_chevrons: bool) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "(vide)".to_string();
    }
    let bracketed = trimmed.starts_with('<') && trimmed.ends_with('>');
    let inner = trimmed.trim_start_matches('<').trim_end_matches('>');
    let count = inner.split('<').count();
    let (local, domain) = match inner.split_once('@') {
        Some((local, domain)) => (local.chars().count(), domain.to_string()),
        None => (inner.chars().count(), "(sans @)".to_string()),
    };
    let plural = if count > 1 {
        format!(" [{count} identifiants]")
    } else {
        String::new()
    };
    let brackets = match (montrer_chevrons, bracketed) {
        (false, _) => String::new(),
        (true, true) => "<…> ".to_string(),
        (true, false) => "SANS CHEVRONS ".to_string(),
    };
    format!("{brackets}partie locale {local} car., domaine « {domain} »{plural}")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage : diagnostic_fils <chemin.db>")?;
    let opened = std::time::Instant::now();
    let conn = Connection::open(&path)?;
    println!("base : {path}");
    println!("ouverture : {} ms\n", opened.elapsed().as_millis());

    let one = |sql: &str| -> rusqlite::Result<i64> { conn.query_row(sql, [], |row| row.get(0)) };

    let messages = one("SELECT COUNT(*) FROM envelopes")?;
    let threads = one("SELECT COUNT(*) FROM threads")?;
    let links = one("SELECT COUNT(*) FROM thread_links")?;
    println!("messages     : {messages}");
    println!("conversations: {threads}");
    println!("annuaire     : {links} identifiants\n");

    // 1. La passe d'en-têtes a-t-elle tourné ?
    //
    // NULL = jamais lu ; '' = lu, le message n'a pas de References ;
    // non vide = lu, et il en a. Les trois se distinguent, sinon on ne
    // sait pas si le silence vient du serveur ou de nous.
    println!("--- passe d'en-têtes ---");
    for (etat, sql) in [
        ("jamais lus", "refs IS NULL"),
        ("lus, sans References", "refs = ''"),
        ("lus, avec References", "refs IS NOT NULL AND refs != ''"),
    ] {
        let count = one(&format!("SELECT COUNT(*) FROM envelopes WHERE {sql}"))?;
        println!("{etat:<24}: {count}");
    }
    let in_reply = one("SELECT COUNT(*) FROM envelopes WHERE in_reply_to IS NOT NULL")?;
    println!("{:<24}: {in_reply}", "avec In-Reply-To");

    // L'horizon borne la passe : sans ce chiffre, « rien n'a bougé » est
    // indiscernable de « rien n'était éligible ».
    let horizon = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64
        - 365 * 86_400;
    let recent = one(&format!(
        "SELECT COUNT(*) FROM envelopes WHERE date_epoch >= {horizon}"
    ))?;
    println!("{:<24}: {recent}\n", "dans l'horizon (12 mois)");

    // 2. Distribution des tailles — un fil géant se voit d'un coup d'œil.
    println!("--- tailles des conversations ---");
    for (etiquette, sql) in [
        ("1 message", "size <= 1"),
        ("2 à 5", "size BETWEEN 2 AND 5"),
        ("6 à 20", "size BETWEEN 6 AND 20"),
        ("plus de 20", "size > 20"),
    ] {
        let count = one(&format!("SELECT COUNT(*) FROM threads WHERE {sql}"))?;
        println!("{etiquette:<12}: {count}");
    }

    // 3. Les plus gros fils, et surtout CE QUI LES LIE.
    //
    // Si les 17 messages d'un fil n'ont qu'un seul `Message-ID` distinct,
    // le coupable est un expéditeur qui réutilise le sien. S'ils n'ont
    // qu'un `In-Reply-To` ou qu'un `References`, c'est une ancre commune
    // — un identifiant de campagne, par exemple. Ces trois comptages
    // désignent le défaut sans montrer aucune valeur.
    println!("\n--- les plus gros fils, et ce qui les lie ---");
    let mut stmt = conn.prepare(
        "SELECT t.id, t.size,
                (SELECT COUNT(DISTINCT message_id) FROM envelopes WHERE thread_id = t.id),
                (SELECT COUNT(DISTINCT in_reply_to) FROM envelopes WHERE thread_id = t.id),
                (SELECT COUNT(DISTINCT refs) FROM envelopes WHERE thread_id = t.id),
                (SELECT COUNT(*) FROM thread_links WHERE thread_id = t.id)
         FROM threads t ORDER BY t.size DESC LIMIT 5",
    )?;
    let gros: Vec<(i64, i64, i64, i64, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<Result<_, _>>()?;

    for (id, size, ids, parents, refs, links) in gros {
        println!(
            "\nfil #{id} — {size} messages | {ids} Message-ID distincts \
             | {parents} In-Reply-To distincts | {refs} References distincts \
             | {links} entrées d'annuaire"
        );
        // Un seul identifiant distinct partagé par tout le fil : c'est
        // lui le liant. On en montre la forme, jamais la valeur.
        for (etiquette, colonne) in [
            ("Message-ID", "message_id"),
            ("In-Reply-To", "in_reply_to"),
            ("References", "refs"),
        ] {
            if size < 2 {
                continue;
            }
            let commun: Option<String> = conn
                .query_row(
                    &format!(
                        "SELECT {colonne} FROM envelopes
                         WHERE thread_id = ?1 AND {colonne} IS NOT NULL AND {colonne} != ''
                         GROUP BY {colonne} HAVING COUNT(*) > 1
                         ORDER BY COUNT(*) DESC LIMIT 1"
                    ),
                    [id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(valeur) = commun {
                let partages: i64 = conn.query_row(
                    &format!(
                        "SELECT COUNT(*) FROM envelopes WHERE thread_id = ?1 AND {colonne} = ?2"
                    ),
                    rusqlite::params![id, valeur],
                    |row| row.get(0),
                )?;
                println!(
                    "  {etiquette} partagé par {partages} messages : {}",
                    shape(&valeur)
                );
            }
        }
    }

    // 3 bis. LES ANCRES — la vraie question.
    //
    // Comparer les en-têtes entiers ne suffit pas : deux messages dont
    // les `References` diffèrent de bout en bout peuvent citer un même
    // ancêtre. C'est CE jeton-là qui les réunit, et une seule ancre
    // fausse fait s'effondrer de proche en proche tout ce qui la touche.
    //
    // On repart donc de l'annuaire, qui contient les jetons tels que le
    // regroupement les a retenus.
    println!("\n--- les ancres des deux plus gros fils ---");
    let mut stmt = conn.prepare("SELECT id FROM threads ORDER BY size DESC LIMIT 2")?;
    let sommets: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;

    for thread in sommets {
        let mut stmt = conn.prepare("SELECT message_id FROM thread_links WHERE thread_id = ?1")?;
        let jetons: Vec<String> = stmt
            .query_map([thread], |row| row.get(0))?
            .collect::<Result<_, _>>()?;

        // `instr` et non `LIKE` : un identifiant contient volontiers des
        // `_`, que `LIKE` interpréterait comme un joker.
        let mut portee: Vec<(i64, bool, String)> = Vec::new();
        for jeton in &jetons {
            let cites: i64 = conn.query_row(
                "SELECT COUNT(*) FROM envelopes
                 WHERE thread_id = ?1
                   AND (instr(COALESCE(message_id, ''), ?2) > 0
                     OR instr(COALESCE(in_reply_to, ''), ?2) > 0
                     OR instr(COALESCE(refs, ''), ?2) > 0)",
                rusqlite::params![thread, jeton],
                |row| row.get(0),
            )?;
            // Une ancre que PERSONNE ne possède est un fantôme : aucun
            // message de la base ne s'appelle ainsi. Légitime quand
            // l'ancêtre est ailleurs (dans « Envoyés »), suspect quand
            // des dizaines de messages étrangers s'y accrochent.
            let possede: i64 = conn.query_row(
                "SELECT COUNT(*) FROM envelopes WHERE instr(COALESCE(message_id, ''), ?1) > 0",
                [jeton],
                |row| row.get(0),
            )?;
            portee.push((cites, possede > 0, jeton.clone()));
        }
        portee.sort_by(|a, b| b.0.cmp(&a.0));

        println!("\nfil #{thread} — {} jetons d'annuaire", jetons.len());
        for (cites, possede, jeton) in portee.iter().take(5) {
            let nature = if *possede {
                "possédé par un message"
            } else {
                "FANTÔME (personne ne le porte)"
            };
            println!(
                "  cité par {cites} messages — {nature} — {}",
                shape_canonical(jeton)
            );
        }
    }

    // 4. Le piège classique : un expéditeur qui réutilise son Message-ID.
    println!("\n--- Message-ID réutilisés (toute la base) ---");
    let mut stmt = conn.prepare(
        "SELECT message_id, COUNT(*) FROM envelopes
         WHERE message_id IS NOT NULL AND message_id != ''
         GROUP BY message_id HAVING COUNT(*) > 1
         ORDER BY COUNT(*) DESC LIMIT 5",
    )?;
    let doublons: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    if doublons.is_empty() {
        println!("aucun — chaque message a le sien");
    }
    for (valeur, count) in doublons {
        println!(
            "{count} messages partagent un Message-ID : {}",
            shape(&valeur)
        );
    }

    Ok(())
}
