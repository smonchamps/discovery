//! Le regroupement des messages en conversations — l'algorithme, pur.
//!
//! Ce module ne connaît ni SQLite, ni le réseau. Il répond à une seule
//! question : « à quel fil ce message appartient-il ? », à partir des
//! identifiants RFC 5322 qu'il porte et de ce qui est déjà connu.
//!
//! Le regroupement est un **union-find** : chaque `Message-ID` rencontré —
//! celui du message ET ceux de ses ancêtres, *même absents de la boîte* —
//! est inscrit dans un annuaire qui pointe vers un fil. Un message citant
//! deux identifiants rattachés à deux fils différents les **fusionne**.
//!
//! Cette fusion n'est pas un cas exotique, c'est ce qui rend le
//! regroupement **convergent**. Deux raisons pour lesquelles un fil naît
//! régulièrement en morceaux :
//!
//! - les messages n'arrivent pas dans l'ordre (une réponse peut être
//!   synchronisée avant le message qu'elle cite) ;
//! - les en-têtes n'arrivent pas ensemble — `In-Reply-To` vient de
//!   l'ENVELOPE, gratuitement, tandis que `References` demande une passe
//!   séparée sur les en-têtes complets.
//!
//! Les morceaux se recollent dès que le lien manquant apparaît, sans
//! qu'aucune information acquise ne soit perdue en route. C'est la
//! propriété qui autorise à livrer l'acquisition en deux temps.

use std::collections::{BTreeSet, HashMap};

use rusqlite::{Connection, OptionalExtension, params};

use crate::envelope::Uid;
use crate::error::Error;

/// Identifiant interne d'un fil.
///
/// Un entier, et non le `Message-ID` de la racine : la racine peut arriver
/// après ses réponses, ou ne jamais arriver du tout (elle est dans
/// « Envoyés », ou elle a été supprimée). Un fil ne doit pas pouvoir
/// changer d'identité en cours de route.
pub(crate) type ThreadId = i64;

/// Nombre maximal d'ancêtres retenus dans `References`.
///
/// L'en-tête est cumulatif : une longue discussion, ou un logiciel fautif,
/// peut en accumuler des milliers. On garde les deux extrémités — la
/// racine, qui rattache le fil entier, et les ancêtres immédiats, qui
/// rattachent le voisinage. Le milieu est redondant : ces messages-là,
/// s'ils sont dans la boîte, portent leurs propres liens.
const MAX_REFERENCES: usize = 32;

/// Part de la borne réservée au début de `References` (la racine).
const KEPT_AT_ROOT: usize = 8;

/// Découpe un en-tête d'identifiants (`Message-ID`, `In-Reply-To`,
/// `References`) en identifiants canoniques.
///
/// Forme canonique = le contenu des chevrons, sans eux. La RFC 5322 les
/// rend obligatoires ; la vraie vie les omet. Comparer les deux formes
/// sans les normaliser ferait deux fils là où il n'y en a qu'un.
fn canonical_ids(raw: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut bracketed = false;
    let mut rest = raw;
    while let Some(open) = rest.find('<') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('>') else { break };
        bracketed = true;
        let id = after[..close].trim();
        if is_message_id(id) {
            ids.push(id.to_string());
        }
        rest = &after[close + 1..];
    }
    if !bracketed {
        // Aucun chevron : hors norme, mais assez répandu pour qu'ignorer
        // ces messages revienne à ne pas les regrouper du tout.
        //
        // Le repli se décide sur la PRÉSENCE de chevrons, jamais sur le
        // fait qu'on en ait tiré quelque chose : un `Message-ID: <>` — un
        // logiciel fautif en produit — retomberait sinon ici.
        ids = raw
            .split_whitespace()
            .filter(|token| is_message_id(token))
            .map(str::to_string)
            .collect();
    }
    ids
}

/// Ce jeton est-il un `Message-ID` plausible ?
///
/// RFC 5322 §3.6.4 : `msg-id = "<" id-left "@" id-right ">"`. **L'arobase
/// est obligatoire**, et c'est elle qui sépare un identifiant d'un mot.
///
/// Ce n'est pas du purisme, c'est le garde-fou qui manquait. Sans lui, un
/// en-tête rédigé en prose — la forme RFC 822 `In-Reply-To: Votre message
/// du 3 janvier`, que des répondeurs automatiques produisent encore —
/// fabrique autant de faux identifiants que de mots. Chaque mot devient
/// une ancre, tous les messages portant la même phrase s'y accrochent, et
/// l'union-find les réunit *correctement* dans un fil qui n'a aucun sens.
///
/// Mesuré sur une vraie boîte avant correction : **43 messages étrangers
/// en une seule conversation**, accrochés à des jetons de 3 à 11
/// caractères sans arobase que personne ne portait.
///
/// Conséquence assumée : un `Message-ID` hors norme (`<1234567890>`) est
/// ignoré. Le message forme alors son propre fil et les réponses qu'il
/// reçoit ne s'y rattachent pas. C'est une perte locale et silencieuse,
/// contre une fusion massive et visible — l'échange est très favorable.
fn is_message_id(token: &str) -> bool {
    token.contains('@') && !token.chars().any(char::is_whitespace)
}

/// Tous les identifiants qui rattachent un message à son fil : le sien
/// d'abord, puis ses ancêtres, du plus ancien au plus proche.
pub(crate) fn linking_ids(
    message_id: Option<&str>,
    in_reply_to: Option<&str>,
    references: Option<&str>,
) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut push = |candidates: Vec<String>| {
        for id in candidates {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    };
    push(message_id.map(canonical_ids).unwrap_or_default());
    push(cap_references(
        references.map(canonical_ids).unwrap_or_default(),
    ));
    push(in_reply_to.map(canonical_ids).unwrap_or_default());
    ids
}

/// Applique [`MAX_REFERENCES`] en gardant les deux extrémités.
fn cap_references(mut refs: Vec<String>) -> Vec<String> {
    if refs.len() <= MAX_REFERENCES {
        return refs;
    }
    let tail = refs.split_off(refs.len() - (MAX_REFERENCES - KEPT_AT_ROOT));
    refs.truncate(KEPT_AT_ROOT);
    refs.extend(tail);
    refs
}

/// Ce qu'il faut faire pour rattacher un message, une fois l'annuaire
/// consulté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThreadPlan {
    /// Le fil d'accueil. `None` : aucun identifiant connu, il faut créer
    /// un fil neuf.
    pub keep: Option<ThreadId>,
    /// Les fils que `keep` absorbe. Vide hors fusion.
    ///
    /// Repointer leurs identifiants vers `keep` est la charge de
    /// l'appelant : c'est une écriture, et ce module n'en fait aucune.
    pub absorb: Vec<ThreadId>,
    /// Les identifiants encore absents de l'annuaire, à y inscrire.
    ///
    /// Y compris ceux d'ancêtres **absents de la boîte** : c'est
    /// précisément ce qui permet à un message arrivé plus tard de
    /// rejoindre le bon fil.
    pub register: Vec<String>,
}

/// Consulte l'annuaire et décide du rattachement.
///
/// `known` n'a besoin de contenir que les identifiants de `ids` — à
/// l'appelant de faire la seule requête qui les cherche.
pub(crate) fn plan(ids: &[String], known: &HashMap<String, ThreadId>) -> ThreadPlan {
    let mut threads: Vec<ThreadId> = Vec::new();
    let mut register: Vec<String> = Vec::new();
    for id in ids {
        match known.get(id) {
            Some(thread) => {
                if !threads.contains(thread) {
                    threads.push(*thread);
                }
            }
            None => {
                if !register.contains(id) {
                    register.push(id.clone());
                }
            }
        }
    }
    // Le fil le plus ancien l'emporte — son identifiant est le plus petit.
    // Ce départage doit être le MÊME quel que soit l'ordre d'arrivée des
    // messages : sinon deux synchronisations de la même boîte ne donnent
    // pas le même découpage, et le fil « saute » sous les yeux de
    // l'utilisateur.
    threads.sort_unstable();
    let mut threads = threads.into_iter();
    ThreadPlan {
        keep: threads.next(),
        absorb: threads.collect(),
        register,
    }
}

// ---------------------------------------------------------------------------
// Persistance — l'algorithme ci-dessus, appliqué à la base.
//
// Toutes ces fonctions prennent une `&Connection` et s'appellent DANS la
// transaction qui écrit le message, comme l'index de recherche (ADR 0004) :
// un fil à moitié rattaché serait pire qu'un message non rattaché.
// ---------------------------------------------------------------------------

/// Les deux tables des fils, aux rôles bien distincts.
///
/// `threads` est un **agrégat matérialisé** : la liste doit pouvoir
/// afficher une page de conversations sans agréger 200 000 enveloppes à
/// chaque défilement. C'est le même raisonnement que l'index de
/// recherche — l'agrégat vit dans la base et s'entretient dans la même
/// transaction que le message.
///
/// `thread_links` est l'**annuaire** : il retient aussi les identifiants
/// d'ancêtres que la boîte ne contient pas. C'est cette mémoire-là qui
/// permet à deux moitiés de fil de se reconnaître plus tard.
pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS threads (
    id         INTEGER PRIMARY KEY,
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    last_uid   INTEGER NOT NULL DEFAULT 0,
    last_epoch INTEGER,
    size       INTEGER NOT NULL DEFAULT 0,
    unseen     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_threads_date
    ON threads(mailbox_id, last_epoch DESC, last_uid DESC);
-- Le même tri, SANS préfixe de boîte : c'est celui dont la boîte unifiée
-- a besoin. Elle couvre la même boîte de TOUS les comptes, donc ne fixe
-- aucun `mailbox_id` — et un index qui commence par cette colonne ne peut
-- alors plus porter l'ordre. SQLite retombait sur un tri matérialisé de
-- toutes les conversations, à CHAQUE page de défilement : 987 ms mesurées
-- sur 160 000 conversations au gate 3, contre 0,66 ms avec cet index.
-- L'index préfixé reste utile aux requêtes bornées à une boîte.
CREATE INDEX IF NOT EXISTS idx_threads_date_globale
    ON threads(last_epoch DESC, last_uid DESC, mailbox_id);
CREATE TABLE IF NOT EXISTS thread_links (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL,
    thread_id  INTEGER NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    PRIMARY KEY (mailbox_id, message_id)
);
CREATE INDEX IF NOT EXISTS idx_thread_links_thread ON thread_links(thread_id);
";

/// Rattache un message à son fil et retourne celui-ci.
///
/// N'écrit PAS `envelopes.thread_id` : l'appelant le fait, parce que lui
/// seul sait si l'enveloppe est déjà écrite. Il doit ensuite appeler
/// [`refresh`] sur le fil retourné.
pub(crate) fn attach(
    conn: &Connection,
    mailbox_id: i64,
    message_id: Option<&str>,
    in_reply_to: Option<&str>,
    references: Option<&str>,
) -> Result<ThreadId, Error> {
    let ids = linking_ids(message_id, in_reply_to, references);
    let decision = plan(&ids, &lookup(conn, mailbox_id, &ids)?);

    let thread = match decision.keep {
        Some(thread) => thread,
        None => {
            conn.prepare_cached("INSERT INTO threads (mailbox_id) VALUES (?1)")?
                .execute([mailbox_id])?;
            conn.last_insert_rowid()
        }
    };
    for absorbed in decision.absorb {
        // L'ordre compte : repointer AVANT de supprimer, sinon la clé
        // étrangère de `thread_links` refuse la suppression — et la
        // refuser est la bonne réaction, elle signale qu'on allait
        // perdre des liens.
        conn.execute(
            "UPDATE thread_links SET thread_id = ?2 WHERE thread_id = ?1",
            params![absorbed, thread],
        )?;
        conn.execute(
            "UPDATE envelopes SET thread_id = ?2 WHERE thread_id = ?1",
            params![absorbed, thread],
        )?;
        conn.execute("DELETE FROM threads WHERE id = ?1", [absorbed])?;
    }
    for id in decision.register {
        conn.prepare_cached(
            "INSERT OR IGNORE INTO thread_links (mailbox_id, message_id, thread_id)
             VALUES (?1, ?2, ?3)",
        )?
        .execute(params![mailbox_id, id, thread])?;
    }
    Ok(thread)
}

/// Les fils déjà connus pour ces identifiants — une seule requête.
fn lookup(
    conn: &Connection,
    mailbox_id: i64,
    ids: &[String],
) -> Result<HashMap<String, ThreadId>, Error> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = (2..=ids.len() + 1)
        .map(|n| format!("?{n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut values: Vec<rusqlite::types::Value> = Vec::with_capacity(ids.len() + 1);
    values.push(mailbox_id.into());
    values.extend(ids.iter().map(|id| id.clone().into()));

    // `prepare_cached` : le cache est indexé par le texte SQL, et il n'y a
    // qu'une poignée de formes (une par nombre d'identifiants cités). Sans
    // lui, chaque message re-analyse et re-planifie sa requête — c'est le
    // poste dominant de l'adoption d'une base héritée.
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT message_id, thread_id FROM thread_links
         WHERE mailbox_id = ?1 AND message_id IN ({placeholders})"
    ))?;
    let known = stmt
        .query_map(rusqlite::params_from_iter(values), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<_, _>>()?;
    Ok(known)
}

/// Recalcule l'agrégat d'un fil depuis ses messages — et le supprime s'il
/// n'en a plus.
///
/// **Recalculer, jamais incrémenter.** Un compteur entretenu par
/// additions et soustractions dérive au premier chemin oublié (fusion,
/// UIDVALIDITY, action rejouée), et une dérive se voit à l'écran pour
/// toujours : « 4 messages » sur un fil qui en montre 3. Le recalcul est
/// borné par la taille du fil et passe par l'index.
pub(crate) fn refresh(conn: &Connection, thread: ThreadId) -> Result<(), Error> {
    let aggregate = conn
        .prepare_cached(
            "SELECT e.uid, e.date_epoch,
                    (SELECT COUNT(*) FROM envelopes WHERE thread_id = ?1),
                    (SELECT COUNT(*) FROM envelopes WHERE thread_id = ?1 AND seen = 0)
             FROM envelopes e
             WHERE e.thread_id = ?1
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT 1",
        )?
        .query_row([thread], |row| {
            Ok((
                row.get::<_, Uid>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .optional()?;

    match aggregate {
        Some((last_uid, last_epoch, size, unseen)) => {
            conn.prepare_cached(
                "UPDATE threads SET last_uid = ?2, last_epoch = ?3, size = ?4, unseen = ?5
                 WHERE id = ?1",
            )?
            .execute(params![thread, last_uid, last_epoch, size, unseen])?;
        }
        None => {
            // Le fil s'est vidé : il disparaît avec son annuaire.
            //
            // Conséquence assumée : si une réponse arrive plus tard, elle
            // ouvre un fil NEUF. C'est honnête — la boîte ne contient plus
            // rien de cette conversation. Garder l'annuaire ferait
            // ressusciter des fils vides que la liste devrait ensuite
            // filtrer, au prix de l'index qui la rend rapide.
            conn.execute("DELETE FROM thread_links WHERE thread_id = ?1", [thread])?;
            conn.execute("DELETE FROM threads WHERE id = ?1", [thread])?;
        }
    }
    Ok(())
}

/// Le fil d'un message, à rafraîchir APRÈS l'avoir retiré de la boîte.
pub(crate) fn thread_of(
    conn: &Connection,
    mailbox_id: i64,
    uid: Uid,
) -> Result<Option<ThreadId>, Error> {
    let thread = conn
        .query_row(
            "SELECT thread_id FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
            |row| row.get::<_, Option<ThreadId>>(0),
        )
        .optional()?
        .flatten();
    Ok(thread)
}

/// Efface les fils d'une boîte (UIDVALIDITY invalidée : plus rien ne
/// veut dire quoi que ce soit).
pub(crate) fn clear_mailbox(conn: &Connection, mailbox_id: i64) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM thread_links WHERE mailbox_id = ?1",
        [mailbox_id],
    )?;
    conn.execute("DELETE FROM threads WHERE mailbox_id = ?1", [mailbox_id])?;
    Ok(())
}

/// Rattache les messages déjà en base — ceux d'avant les fils.
///
/// Sans cette passe, chaque message hérité garderait `thread_id` NULL et
/// **disparaîtrait** d'une liste groupée par fil. C'est exactement le
/// piège des pièces jointes, où les métadonnées n'étaient écrites que par
/// le chemin neuf : une fonctionnalité qui n'adopte pas les données
/// anciennes est fausse dès la première ouverture, et pour toujours.
///
/// Ces messages n'ont que leur `Message-ID` : ils formeront donc surtout
/// des fils d'un seul message, qui se regrouperont au fil de l'acquisition
/// des en-têtes (c'est la propriété de convergence, en tête de module).
/// Version de la règle de regroupement inscrite dans la base
/// (`PRAGMA user_version`, libre d'usage et gratuite à lire).
///
/// **1** — les identifiants sont filtrés par [`is_message_id`].
///
/// Les bases plus anciennes ont été regroupées par une règle qui prenait
/// les mots d'un en-tête rédigé en prose pour des identifiants : leurs
/// fils sont FAUX, et aucune correction du code ne les répare tout seul.
/// Il faut les refaire.
const THREADING_VERSION: i64 = 1;

/// Refait les fils quand la règle qui les a produits a changé.
///
/// Purement local : les en-têtes bruts sont intacts en base, seule leur
/// **interprétation** était fautive. Rien à redemander au serveur.
fn rebuild_if_outdated(conn: &Connection) -> Result<(), Error> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version >= THREADING_VERSION {
        return Ok(());
    }
    // Tout effacer et laisser l'adoption, juste dessous, refaire le
    // travail : un seul chemin de reconstruction, celui qui est déjà
    // testé, plutôt qu'un correctif parallèle qui divergerait.
    conn.execute_batch(&format!(
        "BEGIN;
         DELETE FROM thread_links;
         DELETE FROM threads;
         UPDATE envelopes SET thread_id = NULL;
         PRAGMA user_version = {THREADING_VERSION};
         COMMIT;"
    ))?;
    Ok(())
}

/// Un message pas encore rattaché : sa boîte, son UID, puis les trois
/// en-têtes de regroupement.
type Orphan = (i64, Uid, Option<String>, Option<String>, Option<String>);

pub(crate) fn migrate_threads(conn: &Connection) -> Result<(), Error> {
    rebuild_if_outdated(conn)?;
    let orphans: Vec<Orphan> = conn
        .prepare(
            "SELECT mailbox_id, uid, message_id, in_reply_to, refs
             FROM envelopes WHERE thread_id IS NULL
             ORDER BY mailbox_id, uid",
        )?
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    if orphans.is_empty() {
        return Ok(());
    }

    // Une transaction, pas une par message : sur une boîte déjà remplie,
    // un fsync par enveloppe transformerait l'ouverture de l'application
    // en minutes d'attente — le budget « démarrage < 1 s » interdit ce
    // chemin.
    conn.execute_batch("BEGIN")?;
    match adopt(conn, orphans) {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(err) => {
            // L'échec du retour arrière n'apprendrait rien de plus que
            // l'erreur d'origine, qui est celle qu'il faut remonter.
            let _ = conn.execute_batch("ROLLBACK");
            return Err(err);
        }
    }
    Ok(())
}

fn adopt(conn: &Connection, orphans: Vec<Orphan>) -> Result<(), Error> {
    // Un ENSEMBLE, pas une liste. `Vec::contains` est linéaire : sur une
    // base héritée où presque chaque message ouvre son propre fil, le
    // « ai-je déjà vu ce fil ? » devenait quadratique — 160 000 fils font
    // ~1,3×10¹⁰ comparaisons. Mesuré : 11,1 s d'adoption sur 200 000
    // messages, contre un budget de démarrage d'une seconde. Invisible
    // sur les 2 800 messages d'une boîte réelle, écrasant à l'échelle du
    // gate 3. L'arbre garde en prime un ordre déterministe, sans tri.
    let mut touched: BTreeSet<ThreadId> = BTreeSet::new();
    for (mailbox_id, uid, message_id, in_reply_to, references) in orphans {
        let thread = attach(
            conn,
            mailbox_id,
            message_id.as_deref(),
            in_reply_to.as_deref(),
            references.as_deref(),
        )?;
        conn.prepare_cached(
            "UPDATE envelopes SET thread_id = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
        )?
        .execute(params![mailbox_id, uid, thread])?;
        touched.insert(thread);
    }
    for thread in touched {
        // Un fil de `touched` a pu être absorbé entre-temps ; `refresh`
        // le constate et ne fait rien.
        refresh(conn, thread)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(pairs: &[(&str, ThreadId)]) -> HashMap<String, ThreadId> {
        pairs
            .iter()
            .map(|(id, thread)| ((*id).to_string(), *thread))
            .collect()
    }

    fn ids(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|id| (*id).to_string()).collect()
    }

    /// Un message sans `Message-ID` ni ancêtre n'a AUCUN identifiant. Il
    /// faut donc qu'il n'en inscrive aucun : deux messages muets doivent
    /// rester deux fils distincts, et non se rejoindre sur « rien ».
    #[test]
    fn un_message_sans_identifiant_ne_se_rattache_a_rien() {
        assert!(linking_ids(None, None, None).is_empty());

        let plan = plan(&[], &known(&[]));
        assert_eq!(plan.keep, None, "il lui faut un fil neuf");
        assert!(
            plan.register.is_empty(),
            "rien à inscrire : sinon le message suivant, muet lui aussi, \
             tomberait dans le même fil"
        );
    }

    #[test]
    fn une_reponse_rejoint_le_fil_de_son_parent() {
        let liens = linking_ids(Some("<r@b>"), Some("<a@b>"), None);
        let plan = plan(&liens, &known(&[("a@b", 7)]));

        assert_eq!(plan.keep, Some(7));
        assert!(plan.absorb.is_empty());
        assert_eq!(
            plan.register,
            ids(&["r@b"]),
            "seul l'identifiant neuf s'inscrit"
        );
    }

    /// Le désordre est la règle, pas l'exception : la synchro rapatrie par
    /// UID, et une réponse peut précéder ce qu'elle cite. L'ancêtre absent
    /// est donc inscrit lui aussi, en réservation.
    #[test]
    fn un_ancetre_absent_est_inscrit_pour_que_son_arrivee_rejoigne_le_fil() {
        // La réponse arrive la première : rien n'est connu.
        let reponse = linking_ids(Some("<r@b>"), Some("<a@b>"), None);
        let premier = plan(&reponse, &known(&[]));
        assert_eq!(premier.keep, None);
        assert_eq!(
            premier.register,
            ids(&["r@b", "a@b"]),
            "l'ancêtre encore absent est réservé"
        );

        // Le fil 3 est créé et porte les deux réservations. Le parent
        // arrive ensuite : il se reconnaît.
        let parent = linking_ids(Some("<a@b>"), None, None);
        let ensuite = plan(&parent, &known(&[("r@b", 3), ("a@b", 3)]));
        assert_eq!(ensuite.keep, Some(3));
        assert!(ensuite.register.is_empty());
    }

    /// Le cas qui fait tout marcher : dans une boîte de réception, le
    /// message intermédiaire d'un échange est celui qu'on a ENVOYÉ — il
    /// n'est pas là. C'est `References`, qui porte aussi la racine, qui
    /// recolle les deux moitiés.
    #[test]
    fn le_message_qui_relie_deux_fils_les_fusionne() {
        let liens = linking_ids(Some("<c@b>"), Some("<b@b>"), Some("<a@b> <b@b>"));
        let plan = plan(&liens, &known(&[("a@b", 4), ("b@b", 9)]));

        assert_eq!(plan.keep, Some(4));
        assert_eq!(plan.absorb, vec![9]);
        assert_eq!(plan.register, ids(&["c@b"]));
    }

    /// Le départage ne doit pas dépendre de l'ordre des identifiants dans
    /// l'en-tête, sinon le même message classé deux fois donne deux
    /// résultats.
    #[test]
    fn la_fusion_garde_toujours_le_fil_le_plus_ancien() {
        let annuaire = known(&[("a@b", 4), ("b@b", 9), ("c@b", 6)]);

        let direct = plan(&ids(&["a@b", "b@b", "c@b"]), &annuaire);
        let inverse = plan(&ids(&["b@b", "c@b", "a@b"]), &annuaire);

        assert_eq!(direct.keep, Some(4));
        assert_eq!(direct.absorb, vec![6, 9]);
        assert_eq!(direct, inverse, "le résultat ne dépend pas de l'ordre");
    }

    /// Certains logiciels recopient le `Message-ID` du message dans ses
    /// propres `References`. Se citer soi-même ne doit ni dupliquer, ni
    /// déclencher une fusion d'un fil avec lui-même.
    #[test]
    fn un_message_qui_se_cite_lui_meme_ne_cree_pas_de_second_fil() {
        let liens = linking_ids(Some("<r@b>"), Some("<r@b>"), Some("<r@b>"));
        assert_eq!(liens, ids(&["r@b"]), "un seul identifiant retenu");

        let plan = plan(&liens, &known(&[("r@b", 2)]));
        assert_eq!(plan.keep, Some(2));
        assert!(plan.absorb.is_empty(), "un fil ne s'absorbe pas lui-même");
    }

    /// LE défaut du terrain, en une assertion.
    ///
    /// `In-Reply-To: Votre message du 3 janvier` — la forme RFC 822, que
    /// des répondeurs automatiques produisent encore. L'ancienne règle en
    /// tirait cinq identifiants : « Votre », « message », « du », « 3 »,
    /// « janvier ». Tous les messages portant cette phrase s'accrochaient
    /// aux mêmes ancres et se retrouvaient dans un seul fil. Mesuré sur
    /// une vraie boîte : 43 messages étrangers réunis.
    #[test]
    fn un_en_tete_redige_en_prose_ne_produit_aucun_identifiant() {
        assert!(canonical_ids("Votre message du 3 janvier").is_empty());
        assert!(canonical_ids("Your message of Mon, 01 Jan 2024").is_empty());
        assert!(linking_ids(None, Some("Votre message du 3 janvier"), None).is_empty());
    }

    /// RFC 5322 §3.6.4 : l'arobase n'est pas décorative, c'est elle qui
    /// distingue un identifiant d'un mot.
    #[test]
    fn un_jeton_sans_arobase_n_est_pas_un_identifiant() {
        assert!(canonical_ids("NIL").is_empty());
        assert!(canonical_ids("0").is_empty());
        assert!(
            canonical_ids("<1234567890>").is_empty(),
            "même entre chevrons : hors norme, et court donc collisionnant"
        );
        assert_eq!(canonical_ids("<a@b>"), ids(&["a@b"]));
    }

    /// Le repli sans chevrons reste utile — beaucoup de logiciels les
    /// omettent — mais il ne retient plus que ce qui EST un identifiant.
    #[test]
    fn le_repli_sans_chevrons_ne_garde_que_les_vrais_identifiants() {
        assert_eq!(canonical_ids("a@b Votre message c@d"), ids(&["a@b", "c@d"]));
    }

    /// Un identifiant ne contient pas d'espace : sans cette règle, un
    /// en-tête en prose entre chevrons repasserait par la fenêtre.
    #[test]
    fn un_jeton_contenant_une_espace_est_rejete() {
        assert!(canonical_ids("<Votre message du 3 janvier@relais>").is_empty());
    }

    #[test]
    fn les_chevrons_manquants_donnent_le_meme_identifiant() {
        assert_eq!(canonical_ids("<a@b>"), ids(&["a@b"]));
        assert_eq!(canonical_ids("  a@b  "), ids(&["a@b"]));
        assert_eq!(canonical_ids("< a@b >"), ids(&["a@b"]));
        assert_eq!(canonical_ids("a@b c@d"), ids(&["a@b", "c@d"]));
    }

    /// Le `References` géant du test voisin doit rester composé de vrais
    /// identifiants, sinon la borne ne prouverait plus rien.
    #[test]
    fn la_borne_s_applique_apres_le_filtrage() {
        let raw: String = (0..40).map(|n| format!("<m{n}@b> mot ")).collect();
        let liens = linking_ids(None, None, Some(&raw));
        assert_eq!(liens.len(), MAX_REFERENCES);
        assert!(liens.iter().all(|id| id.contains('@')));
    }

    /// `References` se lit replié sur plusieurs lignes ; les blancs qui
    /// séparent les chevrons n'appartiennent pas aux identifiants.
    #[test]
    fn un_references_replie_sur_plusieurs_lignes_se_lit_entierement() {
        assert_eq!(
            canonical_ids("<a@b>\r\n\t<c@d>\r\n <e@f>"),
            ids(&["a@b", "c@d", "e@f"])
        );
    }

    #[test]
    fn un_en_tete_vide_ou_tronque_ne_produit_aucun_identifiant() {
        assert!(canonical_ids("").is_empty());
        assert!(canonical_ids("   ").is_empty());
        assert!(canonical_ids("<>").is_empty(), "chevrons vides");
    }

    /// La borne protège la requête d'annuaire : sans elle, un en-tête
    /// pathologique ferait chercher des milliers d'identifiants pour un
    /// seul message.
    #[test]
    fn un_references_geant_garde_la_racine_et_les_ancetres_immediats() {
        let raw: String = (0..500).map(|n| format!("<m{n}@b> ")).collect();
        let liens = linking_ids(Some("<moi@b>"), None, Some(&raw));

        assert_eq!(liens.len(), MAX_REFERENCES + 1, "le sien, plus la borne");
        assert_eq!(liens[0], "moi@b");
        assert_eq!(liens[1], "m0@b", "la racine rattache le fil entier");
        assert_eq!(
            liens[KEPT_AT_ROOT + 1],
            "m476@b",
            "puis le saut vers les ancêtres immédiats"
        );
        assert_eq!(
            liens[MAX_REFERENCES], "m499@b",
            "et le plus proche ferme la liste"
        );
    }

    /// L'identifiant du message précède ses ancêtres : c'est celui que les
    /// réponses futures citeront.
    #[test]
    fn l_identifiant_du_message_vient_en_premier() {
        let liens = linking_ids(Some("<moi@b>"), Some("<parent@b>"), Some("<racine@b>"));
        assert_eq!(liens, ids(&["moi@b", "racine@b", "parent@b"]));
    }

    /// Deux ancêtres déjà rattachés au même fil ne comptent qu'une fois :
    /// `absorb` ne doit pas contenir le fil gardé.
    #[test]
    fn deux_ancetres_du_meme_fil_ne_declenchent_pas_de_fusion() {
        let plan = plan(&ids(&["a@b", "c@b"]), &known(&[("a@b", 5), ("c@b", 5)]));
        assert_eq!(plan.keep, Some(5));
        assert!(plan.absorb.is_empty());
    }
}
