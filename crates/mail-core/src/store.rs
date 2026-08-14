//! Stockage local SQLite : enveloppes et état de synchro, multi-boîtes.
//!
//! Structure concrète (pas de trait) : SQLite est une décision produit gelée
//! (PHASE0.md §2.1) et les tests utilisent une base en mémoire — l'abstraction
//! du réseau ([`crate::MailServer`]) est la seule frontière nécessaire.

use std::collections::{BTreeSet, HashSet};
use std::ops::ControlFlow;
use std::path::Path;

use chrono::DateTime;
use rusqlite::{Connection, OptionalExtension, params};

use crate::action::{Action, PendingAction};
use crate::attachment::Attachment;
use crate::envelope::{Envelope, Uid};
use crate::error::Error;
use crate::remote::Folder;
use crate::search;
use crate::thread;

const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS accounts (
    id       INTEGER PRIMARY KEY,
    email    TEXT NOT NULL UNIQUE,
    provider TEXT NOT NULL DEFAULT 'gmail',
    -- Le dossier des envois, sous son nom RESEAU, quand le serveur en
    -- expose un. Il complete la portee du regroupement (ADR 0009), et son
    -- nom varie d'un serveur a l'autre — il ne peut donc pas etre en dur.
    --
    -- Porte par le COMPTE et non deduit a la volee : la boite « Envoyes »
    -- est CREEE par la boucle de synchronisation, donc elle n'existe pas
    -- encore quand on declare la portee. Sans cette memoire, elle naitrait
    -- hors portee et ses messages resteraient sans fil jusqu'au prochain
    -- demarrage — le piege de l'adoption differee.
    sent_mailbox TEXT
);
CREATE TABLE IF NOT EXISTS mailboxes (
    id             INTEGER PRIMARY KEY,
    account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    uid_validity   INTEGER NOT NULL,
    last_uid       INTEGER NOT NULL DEFAULT 0,
    highest_modseq INTEGER,
    -- Cette boite participe-t-elle au REGROUPEMENT en fils ?
    --
    -- Depuis l'ADR 0010 on synchronise TOUTES les boites, mais la portee
    -- d'un fil reste INBOX + Envoyes (ADR 0009). Sans ce drapeau, un spam
    -- ou un message archive rejoindrait le fil tout seul — `thread::attach`
    -- travaille par COMPTE — et ferait remonter la conversation en tete de
    -- liste. Defaut de correction, pas d'ergonomie.
    --
    -- DEFAUT A 1 : c'est la reponse de la MIGRATION, pas celle du produit.
    -- Une base d'avant l'ADR 0010 ne contient qu'INBOX et « Envoyes »,
    -- toutes deux dans la portee ; les mettre a 0 viderait la liste au
    -- premier lancement. `create_mailbox` ecrit toujours la valeur
    -- explicitement, donc ce defaut ne decide jamais pour une boite neuve.
    threaded       INTEGER NOT NULL DEFAULT 1,
    -- Combien de messages le SERVEUR annonce dans cette boite (EXISTS),
    -- au dernier passage. Denominateur de l'avancement (ADR 0010 §5).
    --
    -- 0 = jamais selectionnee. Ce n'est PAS « boite vide » : les deux se
    -- distinguent parce que l'avancement doit se taire quand il ne sait
    -- pas, au lieu d'afficher « 0 % » ou « 100 % ».
    remote_total   INTEGER NOT NULL DEFAULT 0,
    UNIQUE (account_id, name)
);
CREATE TABLE IF NOT EXISTS envelopes (
    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid            INTEGER NOT NULL,
    subject        TEXT,
    sender         TEXT,
    sender_address TEXT,
    message_id     TEXT,
    -- Les deux en-tetes du regroupement en fils. `in_reply_to` vient de
    -- l'ENVELOPE (gratuit) ; `refs` vient d'une passe separee sur les
    -- en-tetes complets, et reste NULL en attendant.
    in_reply_to    TEXT,
    refs           TEXT,
    thread_id      INTEGER,
    date_epoch     INTEGER,
    seen           INTEGER NOT NULL DEFAULT 0,
    flagged        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
CREATE INDEX IF NOT EXISTS idx_envelopes_date ON envelopes(mailbox_id, date_epoch DESC);
CREATE TABLE IF NOT EXISTS bodies (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    html       TEXT NOT NULL,
    -- 0 = corps rapatrie AVANT que les pieces jointes existent : son MIME
    -- n'a jamais ete inspecte, et l'information n'est PAS recuperable
    -- depuis le HTML stocke. Il faut le relire (voir bodies_to_backfill).
    scanned    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
-- Métadonnées seules : jamais les octets. Ils se retéléchargent à la
-- demande (ADR 0007 — le budget disque ne survivrait pas aux fichiers).
CREATE TABLE IF NOT EXISTS attachments (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    idx        INTEGER NOT NULL,
    name       TEXT NOT NULL,
    mime       TEXT NOT NULL,
    size       INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid, idx)
);
-- Liste des dossiers, mise en cache comme les enveloppes : choisir une
-- destination doit marcher HORS LIGNE, sinon le tri s'arrête avec le
-- réseau. Rafraîchie à chaque synchro.
CREATE TABLE IF NOT EXISTS folders (
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    wire       TEXT NOT NULL,
    display    TEXT NOT NULL,
    selectable INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (account_id, wire)
);
CREATE TABLE IF NOT EXISTS pending_actions (
    id         INTEGER PRIMARY KEY,
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    kind       TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS drafts (
    id            INTEGER PRIMARY KEY,
    account_id    INTEGER NOT NULL DEFAULT 1,
    to_raw        TEXT NOT NULL,
    subject       TEXT NOT NULL,
    body          TEXT NOT NULL,
    reply_to_uid  INTEGER,
    -- La boîte qui donne son sens à reply_to_uid (ADR 0009) — le lien
    -- brouillon -> conversation (PLAN-BROUILLONS, B-D2). NULL avant la
    -- colonne : ces brouillons restent sans fil, jamais mal reliés.
    reply_to_mailbox TEXT,
    updated_epoch INTEGER NOT NULL,
    remote_uid    INTEGER,
    pushed_epoch  INTEGER
);
-- Les octets des pièces d'un brouillon, copiés AU GESTE (PLAN-PIECES-JOINTES,
-- PJ-D1) : jamais de chemin nu en base — un fichier déplacé ou supprimé
-- après le geste ne peut plus rien casser. À l'inverse de `attachments`
-- (réception, métadonnées seules), ici les octets sont à NOUS : c'est le
-- message qu'on promet d'envoyer.
CREATE TABLE IF NOT EXISTS draft_attachments (
    id       INTEGER PRIMARY KEY,
    draft_id INTEGER NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    mime     TEXT NOT NULL,
    size     INTEGER NOT NULL,
    bytes    BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_draft_attachments_draft ON draft_attachments(draft_id);
CREATE TABLE IF NOT EXISTS draft_tombstones (
    account_id INTEGER NOT NULL,
    remote_uid INTEGER NOT NULL,
    PRIMARY KEY (account_id, remote_uid)
);
CREATE TABLE IF NOT EXISTS drafts_remote (
    account_id   INTEGER PRIMARY KEY,
    uid_validity INTEGER NOT NULL
);
-- Préférences de l'application persistées EN BASE (pas localStorage) :
-- elles doivent être lisibles par le shell Rust — la garde des bulles
-- d'arrivée se joue à l'émission, côté Rust (PLAN-REGLAGES, R-D2).
CREATE TABLE IF NOT EXISTS prefs (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS outbox (
    id           INTEGER PRIMARY KEY,
    account_id   INTEGER NOT NULL DEFAULT 1,
    message_id   TEXT NOT NULL,
    sender       TEXT NOT NULL,
    recipients   TEXT NOT NULL,
    subject      TEXT NOT NULL,
    body_text    TEXT NOT NULL,
    in_reply_to  TEXT,
    state        TEXT NOT NULL DEFAULT 'queued',
    attempts     INTEGER NOT NULL DEFAULT 0,
    last_error   TEXT,
    queued_epoch INTEGER NOT NULL
);
-- Les pièces du journal d'envoi, copiées de `draft_attachments` dans la
-- transaction du geste d'envoi (PJ-D2 : « jamais d'envoi perdu » couvre
-- les octets). `bytes` passe à NULL au passage à `sent` (PJ-D7) : les
-- métadonnées restent lisibles, la quarantaine et le refus gardent leurs
-- octets — le renvoi sur décision de l'utilisateur doit rester entier.
CREATE TABLE IF NOT EXISTS outbox_attachments (
    id        INTEGER PRIMARY KEY,
    outbox_id INTEGER NOT NULL REFERENCES outbox(id) ON DELETE CASCADE,
    name      TEXT NOT NULL,
    mime      TEXT NOT NULL,
    size      INTEGER NOT NULL,
    bytes     BLOB
);
CREATE INDEX IF NOT EXISTS idx_outbox_attachments_outbox ON outbox_attachments(outbox_id);
";

/// Avancement de l'adoption d'une base héritée, pour l'affichage.
///
/// `total` est un MAJORANT déclaré d'emblée (il ne bouge jamais en cours
/// de passe : une barre qui recule est pire qu'une barre imprécise), et
/// `fait == total` n'est annoncé qu'une fois la passe COMMISE — jamais
/// avant, c'est l'exigence « un signal doit être observable » (§9 de la
/// passation). L'affichage passe par [`crate::sync_percent`], qui porte
/// déjà les cas dégénérés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdoptionProgress {
    pub done: u64,
    pub total: u64,
}

/// État de synchro persisté d'une boîte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    pub mailbox_id: i64,
    pub uid_validity: u32,
    pub last_uid: Uid,
    pub highest_modseq: Option<u64>,
}

/// Un compte connecté au client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub provider: String,
}

/// Une ligne de la boîte unifiée : l'enveloppe ET son compte — un UID
/// seul n'identifie plus un message en multi-comptes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedRow {
    pub account_id: i64,
    pub account_email: String,
    /// La boîte qui CONTIENT ce message, sous son nom réseau.
    ///
    /// Sans elle, `(account_id, uid)` n'identifie plus rien depuis que les
    /// fils réunissent plusieurs boîtes ([ADR 0009]) : les UID sont
    /// attribués par boîte et repartent de 1, donc le message n°1 d'INBOX
    /// et le n°1 d'« Envoyés » sont deux messages différents du même
    /// compte. Toute lecture et toute action doivent la porter.
    pub mailbox: String,
    pub envelope: Envelope,
    /// Le message porte-t-il au moins une piece jointe ?
    ///
    /// Faux tant que son corps n'a pas ete lu — meme condition que la
    /// recherche dans le texte. Le trombone apparait donc au fil du
    /// rattrapage, jamais a tort.
    pub has_attachment: bool,
    /// COMBIEN de pièces jointes — la puce du prototype dit « 2
    /// fichiers », pas « des fichiers ». 0 tant que le corps n'a pas été
    /// lu, comme `has_attachment`.
    pub attachment_count: u32,
    /// L'aperçu texte sous l'objet (écran 02) — calculé à l'écriture du
    /// corps, `None` tant que le corps n'est pas rapatrié.
    pub preview: Option<String>,
    /// Le fil auquel ce message appartient. `None` seulement pendant la
    /// fenêtre où une base héritée n'a pas encore été adoptée.
    pub thread_id: Option<i64>,
    /// Nombre de messages du fil, **reçus et envoyés confondus**.
    /// 1 = message isolé.
    ///
    /// Depuis l'ADR 0009, un fil appartient au COMPTE et non à une boîte :
    /// nos propres réponses en font partie. Le compteur doit donc les
    /// inclure, sans quoi il contredirait à l'écran le bandeau de
    /// conversation, qui montre l'échange entier.
    pub thread_size: u32,
    /// Non-lus du fil. Un fil se montre non lu tant qu'il en reste un,
    /// même si son dernier message est lu.
    pub thread_unseen: u32,
}

/// Colonnes du SELECT unifié, partagées par [`Store::unified_recent`] et
/// [`Store::search`] — l'ordre est celui de [`row_to_unified`].
/// La derniere colonne est un EXISTS sur `attachments` : la liste doit
/// pouvoir afficher le trombone sans une requete par ligne. La cle
/// primaire (mailbox_id, uid, idx) rend ce test indexe.
// Exige les alias `e` (envelopes), `m` (mailboxes), `a` (accounts) ET la
// jointure `LEFT JOIN bodies b` — l'aperçu de liste vient de là, NULL
// tant que le corps n'est pas rapatrié. Le COUNT de pièces jointes
// remplace l'ancien EXISTS : la puce du prototype dit « 2 fichiers »,
// pas « des fichiers ». Les deux ne s'exécutent que sur les lignes
// RETENUES par la pagination (gate P1).
pub(crate) const SELECT_UNIFIED: &str = "SELECT a.id, a.email, e.uid, e.subject, e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen, e.flagged, (SELECT COUNT(*) FROM attachments att WHERE att.mailbox_id = e.mailbox_id AND att.uid = e.uid), e.thread_id, e.in_reply_to, m.name, b.preview";

/// Le SELECT de la liste groupée : les colonnes ci-dessus, plus l'agrégat
/// du fil. Il exige la jointure sur `threads` (alias `t`), que la
/// recherche n'a pas — un résultat de recherche est UN message, pas une
/// conversation.
pub(crate) const THREAD_AGGREGATE: &str = ", t.size, t.unseen";

pub struct Store(Connection);

impl Store {
    /// Accès réservé aux modules du crate qui étendent le stockage
    /// (boîte d'envoi, dans `outbox.rs`) sans grossir ce fichier.
    pub(crate) fn conn(&self) -> &Connection {
        &self.0
    }

    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::init(Connection::open(path)?)
    }

    /// Ouvre en rendant l'adoption d'une base héritée VISIBLE et
    /// INTERRUPTIBLE (Phase 5, chantier arbitré — passation §8).
    ///
    /// `on_progress` est appelé pendant la passe d'adoption avec
    /// l'avancement `(fait, total)`. Répondre [`ControlFlow::Break`]
    /// annule : **tout est défait** (`ROLLBACK`), `PRAGMA user_version`
    /// reste inchangé, et l'ouverture rend [`Error::Interrupted`] — la
    /// passe entière se rejouera au prochain lancement. Jamais d'adoption
    /// partielle persistée : la liste part de `threads`, une base à
    /// moitié adoptée serait une boîte à moitié vide.
    ///
    /// Sur une base à jour, `on_progress` n'est JAMAIS appelé : rien à
    /// adopter, rien à raconter — pas de faux bandeau à chaque lancement.
    pub fn open_with_progress(
        path: &Path,
        mut on_progress: impl FnMut(AdoptionProgress) -> ControlFlow<()>,
    ) -> Result<Self, Error> {
        Self::init_with(Connection::open(path)?, &mut on_progress)
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        Self::init(Connection::open_in_memory()?)
    }

    /// Une adoption de base héritée attend-elle ici ? Sonde en **lecture
    /// seule** : rien n'est déclenché, rien n'est créé — c'est ce qui
    /// permet au desktop d'afficher l'écran de migration AVANT la
    /// première vraie ouverture, celle qui paiera la passe.
    ///
    /// Rend le nombre de messages concernés (`None` = rien à faire).
    /// C'est un ordre de grandeur pour l'écran d'attente, pas le
    /// dénominateur de l'avancement : celui-ci arrive par
    /// [`Store::open_with_progress`], seule à connaître la portée exacte.
    pub fn pending_adoption(path: &Path) -> Result<Option<u64>, Error> {
        if !path.exists() {
            // Première installation : rien d'hérité, et ouvrir créerait
            // le fichier — une sonde ne laisse pas de trace.
            return Ok(None);
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version >= thread::THREADING_VERSION {
            return Ok(None);
        }
        // Une base d'avant les fils peut ne pas avoir la table : le COUNT
        // direct échouerait, et la sonde doit répondre, pas expliquer.
        let has_envelopes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'envelopes'",
            [],
            |row| row.get(0),
        )?;
        if has_envelopes == 0 {
            return Ok(None);
        }
        // Quand la base connaît la portée du regroupement (ADR 0010),
        // n'annoncer QUE ce que la passe adoptera : sur une boîte
        // intégrale, la portée (INBOX + Envoyés) est très en dessous du
        // total, et « 256 312 messages » pour une passe qui en rattache
        // 7 500 serait un chiffre qui ne désigne pas ce qu'il dit.
        let messages: i64 = if table_columns(&conn, "mailboxes")?.contains("threaded") {
            conn.query_row(
                "SELECT COUNT(*) FROM envelopes e
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.threaded = 1",
                [],
                |row| row.get(0),
            )?
        } else {
            conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?
        };
        if messages == 0 {
            Ok(None)
        } else {
            Ok(Some(messages as u64))
        }
    }

    fn init(conn: Connection) -> Result<Self, Error> {
        Self::init_with(conn, &mut |_| ControlFlow::Continue(()))
    }

    fn init_with(
        conn: Connection,
        on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
    ) -> Result<Self, Error> {
        // Plusieurs commandes ouvrent chacune leur connexion : patienter
        // plutôt que d'échouer en SQLITE_BUSY sur une écriture concurrente.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        // WAL (ADR 0011) : une lecture ne bloque plus jamais une écriture,
        // ni l'inverse. Le mode rollback tenait tant que les écritures
        // duraient quelques secondes ; la synchronisation intégrale
        // (ADR 0010) les étire en minutes, et le PREMIER essai terrain a
        // produit « database is locked » — le sondage d'avancement et la
        // liste, en lisant, faisaient expirer le busy_timeout de la passe
        // d'en-têtes.
        //
        // `query_row` et non `pragma_update` : ce PRAGMA répond une ligne
        // (le mode effectif). Une base en mémoire répond « memory » — ce
        // n'est pas un échec, les tests y vivent très bien sans WAL. Le
        // mode est PERSISTANT : écrit une fois dans l'en-tête du fichier,
        // relu à chaque ouverture, bases héritées comprises.
        conn.query_row("PRAGMA journal_mode = wal", [], |row| {
            row.get::<_, String>(0)
        })?;
        conn.execute_batch(SCHEMA)?;
        // Les migrations légères d'abord : colonnes, recherche, index.
        // Idempotentes et atomiques une à une — et l'adoption des fils,
        // juste dessous, a besoin des colonnes qu'elles ajoutent
        // (`thread_id`, `in_reply_to`, `refs`).
        migrate(&conn)?;
        // ——— L'unité des fils, d'un seul tenant (passation §8). ———
        // Du DROP conditionnel jusqu'à `user_version`, tout vit dans UNE
        // transaction : annuler pendant l'adoption rembobine TOUT — une
        // adoption partielle persistée serait une boîte à moitié vide,
        // la liste partant de `threads`. Le BEGIN est DEFERRED : sur une
        // base à jour rien n'écrit, la transaction reste lectrice et ne
        // rencontre jamais l'écrivain d'une synchro longue (ADR 0011).
        conn.execute_batch("BEGIN")?;
        let unit = (|| {
            // AVANT le schéma des fils, jamais après : si la règle de
            // regroupement a changé, les deux tables doivent DISPARAÎTRE
            // pour que le `CREATE TABLE IF NOT EXISTS` juste dessous les
            // recrée dans leur forme neuve. Sans cela l'ouverture échoue —
            // voir `thread::drop_if_outdated`.
            thread::drop_if_outdated(&conn)?;
            conn.execute_batch(thread::SCHEMA)?;
            thread::migrate_threads_with(&conn, on_progress)
        })();
        let announced = match unit {
            Ok(announced) => {
                conn.execute_batch("COMMIT")?;
                announced
            }
            Err(err) => {
                // L'échec du retour arrière n'apprendrait rien de plus que
                // l'erreur d'origine, qui est celle qu'il faut remonter —
                // l'annulation volontaire comprise.
                let _ = conn.execute_batch("ROLLBACK");
                return Err(err);
            }
        };
        if let Some(total) = announced {
            // « Fini » ne se dit qu'une fois la passe COMMISE — jamais
            // avant (un signal doit être observable, passation §9). Trop
            // tard pour annuler : la réponse est ignorée.
            let _ = on_progress(AdoptionProgress { done: total, total });
        }
        Ok(Self(conn))
    }

    /// Préférence booléenne persistée en base. Absente = `default` : une
    /// préférence jamais touchée n'écrit rien — la base ne porte que les
    /// choix explicites.
    pub fn bool_pref(&self, key: &str, default: bool) -> Result<bool, Error> {
        let value: Option<String> = self
            .0
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.map_or(default, |v| v == "1"))
    }

    pub fn set_bool_pref(&self, key: &str, value: bool) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO prefs (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, if value { "1" } else { "0" }],
        )?;
        Ok(())
    }

    /// Préférence texte persistée en base — le pendant de `bool_pref`
    /// pour les valeurs nommées (la langue de l'interface, PLAN-LANGUES).
    /// Absente = `None` : une préférence jamais touchée n'écrit rien,
    /// c'est l'appelant qui connaît son défaut.
    pub fn text_pref(&self, key: &str) -> Result<Option<String>, Error> {
        let value = self
            .0
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    pub fn set_text_pref(&self, key: &str, value: &str) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO prefs (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn sync_state(&self, account_id: i64, mailbox: &str) -> Result<Option<SyncState>, Error> {
        let state = self
            .0
            .query_row(
                "SELECT id, uid_validity, last_uid, highest_modseq
                 FROM mailboxes WHERE account_id = ?1 AND name = ?2",
                params![account_id, mailbox],
                |row| {
                    Ok(SyncState {
                        mailbox_id: row.get(0)?,
                        uid_validity: row.get(1)?,
                        last_uid: row.get(2)?,
                        highest_modseq: row.get::<_, Option<i64>>(3)?.map(|m| m as u64),
                    })
                },
            )
            .optional()?;
        Ok(state)
    }

    /// Enregistre une boite. Elle n'entre dans la portee du regroupement
    /// que si c'est la boite de reception : le dossier « Envoyes » y entre
    /// aussi, mais son NOM varie d'un serveur a l'autre (ADR 0009 §7), donc
    /// seul l'appelant qui l'a decouvert peut le declarer —
    /// [`Store::set_thread_scope`].
    ///
    /// Toutes les autres — Archive, Corbeille, Spam, dossiers utilisateur —
    /// sont stockees et indexees, jamais regroupees (ADR 0010 §3).
    pub fn create_mailbox(
        &self,
        account_id: i64,
        mailbox: &str,
        uid_validity: u32,
    ) -> Result<i64, Error> {
        // `COALESCE` : sans lui, un compte sans dossier d'envois connu
        // rendrait `?2 = NULL` — donc NULL — et `faux OR NULL` vaut NULL en
        // SQL. La colonne étant NOT NULL, l'insertion échouerait pour tout
        // dossier ordinaire d'un compte qui n'a pas encore découvert ses
        // envois. C'est-à-dire au tout premier passage.
        self.0.execute(
            "INSERT INTO mailboxes (account_id, name, uid_validity, threaded)
             VALUES (?1, ?2, ?3,
                     ?2 = ?4 OR COALESCE(
                         ?2 = (SELECT sent_mailbox FROM accounts WHERE id = ?1), 0))",
            params![account_id, mailbox, uid_validity, thread::RECEIVED_MAILBOX],
        )?;
        Ok(self.0.last_insert_rowid())
    }

    /// Les boites d'un compte, dans l'ordre ou le rattrapage doit les
    /// servir : la reception d'abord (c'est elle que la liste montre et
    /// que la recherche du quotidien interroge), les envois ensuite (ils
    /// completent les fils), le reste par nom — deterministe, donc
    /// reprenable d'une session a l'autre.
    ///
    /// Miroir HORS LIGNE de `sync_order` : meme priorite, mais la source
    /// est la base et non le serveur — le rattrapage ne doit pas payer un
    /// LIST pour savoir quoi pomper.
    pub fn mailbox_names(&self, account_id: i64) -> Result<Vec<String>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT name FROM mailboxes WHERE account_id = ?1
             ORDER BY (name = ?2) DESC,
                      (name = (SELECT sent_mailbox FROM accounts WHERE id = ?1)) DESC,
                      name",
        )?;
        let names = stmt
            .query_map(params![account_id, thread::RECEIVED_MAILBOX], |row| {
                row.get(0)
            })?
            .collect::<Result<_, _>>()?;
        Ok(names)
    }

    /// Combien de messages ce COMPTE possede en base, toutes boites
    /// confondues. C'est le « deja fait » que la garde d'espace disque
    /// soustrait de l'annonce des serveurs (ADR 0010 §4) : sans lui, une
    /// boite deja rapatriee aux trois quarts serait refusee comme si tout
    /// restait a telecharger.
    pub fn account_message_count(&self, account_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1",
            [account_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Le UIDNEXT vu au relevé qui a précédé la dernière relève soldée
    /// (ADR 0017) — `None` tant qu'aucune relève gardée n'a eu lieu.
    pub fn remote_uidnext(&self, mailbox_id: i64) -> Result<Option<u32>, Error> {
        Ok(self.0.query_row(
            "SELECT remote_uidnext FROM mailboxes WHERE id = ?1",
            params![mailbox_id],
            |row| row.get(0),
        )?)
    }

    /// Pose le UIDNEXT vu — APRÈS que la relève a été soldée, jamais
    /// avant : un repère posé sur une relève interrompue ferait sauter
    /// un dossier pas encore rattrapé.
    pub fn set_remote_uidnext(&self, mailbox_id: i64, uidnext: u32) -> Result<(), Error> {
        self.0.execute(
            "UPDATE mailboxes SET remote_uidnext = ?2 WHERE id = ?1",
            params![mailbox_id, uidnext],
        )?;
        Ok(())
    }

    /// Messages en base pour CE dossier — le pendant local du MESSAGES
    /// du STATUS, comparés par `faut_relever` (ADR 0017).
    pub fn envelope_count(&self, mailbox_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes WHERE mailbox_id = ?1",
            params![mailbox_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Des actions locales attendent-elles leur rejeu dans ce dossier ?
    /// EXISTS et non la liste : la question est fermée, la réponse aussi.
    pub fn has_pending_actions(&self, mailbox_id: i64) -> Result<bool, Error> {
        Ok(self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM pending_actions WHERE mailbox_id = ?1)",
            params![mailbox_id],
            |row| row.get(0),
        )?)
    }

    /// Releve ce que le serveur annonce dans cette boite (EXISTS).
    pub fn record_remote_total(&self, mailbox_id: i64, exists: u32) -> Result<(), Error> {
        self.0.execute(
            "UPDATE mailboxes SET remote_total = ?2 WHERE id = ?1",
            params![mailbox_id, exists],
        )?;
        Ok(())
    }

    /// Avancement de la synchronisation, toutes boites et tous comptes
    /// confondus : (messages en base, messages annonces par les serveurs).
    ///
    /// Ne compte QUE les boites deja selectionnees au moins une fois
    /// (`remote_total > 0`). Sinon un compte dont la moitie des dossiers
    /// n'a pas encore ete visitee afficherait un avancement qui RECULE a
    /// mesure qu'on les decouvre — le denominateur grandissant plus vite
    /// que le numerateur. Un avancement qui recule est pire que pas
    /// d'avancement du tout.
    pub fn sync_progress(&self) -> Result<(u64, u64), Error> {
        let (local, remote): (i64, i64) = self.0.query_row(
            "SELECT COALESCE(SUM(
                        (SELECT COUNT(*) FROM envelopes e WHERE e.mailbox_id = m.id)), 0),
                    COALESCE(SUM(m.remote_total), 0)
             FROM mailboxes m WHERE m.remote_total > 0",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok((local as u64, remote as u64))
    }

    /// Declare la portee du regroupement d'un compte : la boite de
    /// reception, plus le dossier des envois quand le serveur en expose un.
    ///
    /// Appele APRES la decouverte des dossiers, a chaque synchronisation :
    /// un serveur peut renommer son dossier d'envois, et un compte peut
    /// n'en avoir aucun — auquel cas les fils ne regroupent que les recus,
    /// exactement comme avant l'ADR 0009. Idempotent.
    pub fn set_thread_scope(&self, account_id: i64, sent: Option<&str>) -> Result<(), Error> {
        // Mémorisé sur le compte d'ABORD : c'est cette mémoire que
        // `create_mailbox` consultera pour les boîtes que la boucle de
        // synchronisation n'a pas encore créées.
        self.0.execute(
            "UPDATE accounts SET sent_mailbox = ?2 WHERE id = ?1",
            params![account_id, sent],
        )?;
        self.0.execute(
            "UPDATE mailboxes SET threaded = (name = ?2 OR (?3 IS NOT NULL AND name = ?3))
             WHERE account_id = ?1",
            params![account_id, thread::RECEIVED_MAILBOX, sent],
        )?;
        Ok(())
    }

    /// Enregistre un compte, ou revendique le compte « en attente
    /// d'adoption » créé par la migration Phase 2 → 3 (email vide) : la
    /// première connexion après la mise à jour est, en pratique, le même
    /// compte Gmail qu'avant — ses données l'attendent.
    pub fn adopt_or_create_account(&self, email: &str, provider: &str) -> Result<i64, Error> {
        if let Some(id) = self.account_id(email)? {
            return Ok(id);
        }
        let claimed = self.0.execute(
            "UPDATE accounts SET email = ?1, provider = ?2
             WHERE email = '' AND id = (SELECT MIN(id) FROM accounts WHERE email = '')",
            params![email, provider],
        )?;
        if claimed == 0 {
            self.0.execute(
                "INSERT INTO accounts (email, provider) VALUES (?1, ?2)",
                params![email, provider],
            )?;
            return Ok(self.0.last_insert_rowid());
        }
        self.account_id(email)?
            .ok_or_else(|| Error::Corrupt("compte revendiqué introuvable".to_string()))
    }

    fn account_id(&self, email: &str) -> Result<Option<i64>, Error> {
        let id = self
            .0
            .query_row("SELECT id FROM accounts WHERE email = ?1", [email], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(id)
    }

    /// Les comptes connus — sans l'éventuel compte en attente d'adoption.
    pub fn accounts(&self) -> Result<Vec<Account>, Error> {
        let mut stmt = self
            .0
            .prepare("SELECT id, email, provider FROM accounts WHERE email != '' ORDER BY id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Account {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    provider: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Configuration serveur d'un compte (Gmail ou IMAP générique).
    pub fn account_config(&self, account_id: i64) -> Result<AccountConfig, Error> {
        let config = self
            .0
            .query_row(
                "SELECT imap_host, imap_port, smtp_host, smtp_port, username
                 FROM accounts WHERE id = ?1",
                [account_id],
                |row| {
                    Ok(AccountConfig {
                        imap_host: row.get(0)?,
                        imap_port: row.get(1)?,
                        smtp_host: row.get(2)?,
                        smtp_port: row.get(3)?,
                        username: row.get(4)?,
                    })
                },
            )
            .optional()?
            .unwrap_or(AccountConfig {
                imap_host: None,
                imap_port: None,
                smtp_host: None,
                smtp_port: None,
                username: None,
            });
        Ok(config)
    }

    /// Crée ou met à jour un compte IMAP/SMTP générique.
    pub fn create_generic_account(
        &self,
        email: &str,
        username: &str,
        imap_host: &str,
        imap_port: u16,
        smtp_host: &str,
        smtp_port: u16,
    ) -> Result<i64, Error> {
        self.0.execute(
            "INSERT INTO accounts (email, provider, username, imap_host, imap_port, smtp_host, smtp_port)
             VALUES (?1, 'imap', ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(email) DO UPDATE SET
                provider = 'imap',
                username = excluded.username,
                imap_host = excluded.imap_host,
                imap_port = excluded.imap_port,
                smtp_host = excluded.smtp_host,
                smtp_port = excluded.smtp_port",
            params![
                email,
                username,
                imap_host,
                imap_port,
                smtp_host,
                smtp_port
            ],
        )?;
        // JAMAIS `last_insert_rowid()` : sur le chemin UPDATE (ré-ajout),
        // aucune ligne n'est insérée et il renverrait 0 (ou un id d'une
        // autre écriture de la connexion). L'id fait toujours foi en base.
        self.account_id(email)?.ok_or_else(|| {
            Error::Corrupt("compte générique introuvable après écriture".to_string())
        })
    }

    /// Supprime un compte et TOUT ce qui s'y rattache, en une transaction.
    ///
    /// Les cascades du schéma emportent boîtes, enveloppes, corps, pièces
    /// jointes, actions en attente, dossiers et fils. Trois familles n'ont
    /// PAS de clé étrangère et se vident à la main : l'index de recherche
    /// (boîte par boîte, AVANT que la cascade ne fasse disparaître les
    /// boîtes), les brouillons (avec pierres tombales et repère distant)
    /// et la boîte d'envoi. Rien ne doit survivre au compte — un reste
    /// orphelin ne serait jamais relu, mais continuerait de sortir en
    /// recherche ou de partir à la prochaine vidange.
    pub fn delete_account(&mut self, account_id: i64) -> Result<(), Error> {
        let tx = self.0.transaction()?;
        let mailboxes: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM mailboxes WHERE account_id = ?1")?;
            stmt.query_map([account_id], |row| row.get(0))?
                .collect::<Result<Vec<i64>, _>>()?
        };
        for mailbox_id in mailboxes {
            search::deindex_mailbox(&tx, mailbox_id)?;
        }
        tx.execute("DELETE FROM drafts WHERE account_id = ?1", [account_id])?;
        tx.execute(
            "DELETE FROM draft_tombstones WHERE account_id = ?1",
            [account_id],
        )?;
        tx.execute(
            "DELETE FROM drafts_remote WHERE account_id = ?1",
            [account_id],
        )?;
        tx.execute("DELETE FROM outbox WHERE account_id = ?1", [account_id])?;
        tx.execute("DELETE FROM accounts WHERE id = ?1", [account_id])?;
        tx.commit()?;
        Ok(())
    }

    /// Repart de zéro pour une boîte dont l'UIDVALIDITY a changé : les UIDs
    /// ne veulent plus rien dire — corps et actions en attente compris (une
    /// intention sur un UID invalidé est irréalisable par construction).
    pub fn reset_mailbox(&self, mailbox_id: i64, uid_validity: u32) -> Result<(), Error> {
        search::deindex_mailbox(&self.0, mailbox_id)?;
        self.0.execute(
            "DELETE FROM pending_actions WHERE mailbox_id = ?1",
            [mailbox_id],
        )?;
        self.0
            .execute("DELETE FROM bodies WHERE mailbox_id = ?1", [mailbox_id])?;
        self.0
            .execute("DELETE FROM envelopes WHERE mailbox_id = ?1", [mailbox_id])?;
        self.0.execute(
            "UPDATE mailboxes
             SET uid_validity = ?2, last_uid = 0, highest_modseq = NULL
             WHERE id = ?1",
            params![mailbox_id, uid_validity],
        )?;
        // APRÈS la suppression des enveloppes, jamais avant : un fil se
        // recalcule à partir de ce qui reste. Le refaire d'abord le
        // ferait pointer sur des messages qu'on s'apprête à effacer.
        //
        // Et sur le COMPTE, pas la boîte : depuis l'ADR 0009 un fil peut
        // réunir INBOX et « Envoyés », donc réinitialiser l'une oblige à
        // reconsidérer les deux.
        let account_id: i64 = self.0.query_row(
            "SELECT account_id FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        thread::rebuild_account(&self.0, account_id)?;
        Ok(())
    }

    pub fn update_state(
        &self,
        mailbox_id: i64,
        last_uid: Uid,
        highest_modseq: Option<u64>,
    ) -> Result<(), Error> {
        self.0.execute(
            "UPDATE mailboxes SET last_uid = ?2, highest_modseq = ?3 WHERE id = ?1",
            params![mailbox_id, last_uid, highest_modseq.map(|m| m as i64)],
        )?;
        Ok(())
    }

    pub fn upsert_envelopes(
        &mut self,
        mailbox_id: i64,
        envelopes: &[Envelope],
    ) -> Result<(), Error> {
        let tx = self.0.transaction()?;
        // Résolu UNE fois : la boîte ne change pas dans un lot, et le fil
        // se raisonne désormais au compte (ADR 0009). Le faire par message
        // ajouterait une requête par enveloppe sur le chemin le plus chaud
        // de la synchronisation.
        // Même raison pour la portée : elle est propre à la boîte, pas au
        // message. Hors portée, on stocke et on indexe sans regrouper —
        // `thread_id` reste NULL (ADR 0010 §3).
        let (account_id, threaded): (i64, bool) = tx.query_row(
            "SELECT account_id, threaded FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        // Un ENSEMBLE, pas une liste : même défaut quadratique que celui
        // mesuré dans l'adoption (`Vec::contains` est linéaire). Borné ici
        // par la taille du lot, donc moins spectaculaire — mais c'est le
        // même chemin chaud, et la même correction.
        let mut touched: BTreeSet<i64> = BTreeSet::new();
        {
            // `INSERT OR REPLACE` remettrait à NULL toute colonne absente
            // de la liste — et `refs` comme `thread_id` sont écrits par
            // d'AUTRES chemins que la synchro. Une re-synchronisation
            // effacerait donc silencieusement le travail de rattrapage des
            // en-têtes, exactement comme elle aurait effacé les pièces
            // jointes. On énumère donc les colonnes que la synchro possède,
            // et elles seules.
            let mut stmt = tx.prepare(
                "INSERT INTO envelopes
                 (mailbox_id, uid, subject, sender, sender_address, message_id,
                  in_reply_to, date_epoch, seen, flagged)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT (mailbox_id, uid) DO UPDATE SET
                     subject = excluded.subject,
                     sender = excluded.sender,
                     sender_address = excluded.sender_address,
                     message_id = excluded.message_id,
                     in_reply_to = excluded.in_reply_to,
                     date_epoch = excluded.date_epoch,
                     seen = excluded.seen,
                     flagged = excluded.flagged",
            )?;
            let mut body_stmt =
                tx.prepare("SELECT html FROM bodies WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut refs_stmt =
                tx.prepare("SELECT refs FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2")?;
            for envelope in envelopes {
                stmt.execute(params![
                    mailbox_id,
                    envelope.uid,
                    envelope.subject,
                    envelope.sender,
                    envelope.sender_address,
                    envelope.message_id,
                    envelope.in_reply_to,
                    envelope.date.map(|d| d.timestamp()),
                    envelope.seen,
                    envelope.flagged,
                ])?;

                // Les `References` déjà acquises comptent dans le
                // rattachement : une re-synchronisation ne doit pas
                // dégrouper un fil que la passe d'en-têtes avait recollé.
                let references: Option<String> = refs_stmt
                    .query_row(params![mailbox_id, envelope.uid], |row| row.get(0))
                    .optional()?
                    .flatten();
                if threaded {
                    let thread = thread::attach(
                        &tx,
                        account_id,
                        envelope.message_id.as_deref(),
                        envelope.in_reply_to.as_deref(),
                        references.as_deref(),
                    )?;
                    tx.execute(
                        "UPDATE envelopes SET thread_id = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
                        params![mailbox_id, envelope.uid, thread],
                    )?;
                    touched.insert(thread);
                }

                let html: Option<String> = body_stmt
                    .query_row(params![mailbox_id, envelope.uid], |row| row.get(0))
                    .optional()?;
                search::index_message(
                    &tx,
                    mailbox_id,
                    envelope.uid,
                    envelope.subject.as_deref(),
                    envelope.sender.as_deref(),
                    envelope.sender_address.as_deref(),
                    html.as_deref(),
                )?;
            }
            // Après la boucle, et une seule fois par fil : recalculer à
            // chaque message ferait N fois le travail sur une conversation
            // de N messages arrivant dans le même lot.
            for thread in &touched {
                thread::refresh(&tx, *thread)?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Enregistre les en-têtes de fil lus dans le bloc d'en-têtes complet,
    /// et recolle le fil s'il y a lieu.
    ///
    /// `references` vaut `""` quand le message n'en porte pas : c'est la
    /// marque « déjà lu, rien à y trouver ». Écrire NULL le ferait
    /// redemander à chaque passage, indéfiniment.
    ///
    /// Retourne `true` si le rattachement a changé — l'appelant sait alors
    /// que la liste affichée n'est plus à jour.
    pub fn set_thread_headers(
        &mut self,
        mailbox_id: i64,
        uid: Uid,
        in_reply_to: Option<&str>,
        references: &str,
    ) -> Result<bool, Error> {
        let tx = self.0.transaction()?;
        let before: Option<i64> = tx
            .query_row(
                "SELECT thread_id FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let context: Option<(Option<String>, Option<String>)> = tx
            .query_row(
                "SELECT message_id, in_reply_to FROM envelopes
                 WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((message_id, known_parent)) = context else {
            // Le message a disparu entre la lecture des en-têtes et leur
            // écriture (archivé, supprimé) : il n'y a plus rien à rattacher.
            return Ok(false);
        };

        // `COALESCE` : le bloc d'en-têtes fait autorité quand il dit
        // quelque chose, mais un `In-Reply-To` déjà donné par l'ENVELOPE ne
        // doit pas être effacé par une lecture qui n'en trouve pas.
        tx.execute(
            "UPDATE envelopes SET refs = ?3, in_reply_to = COALESCE(?4, in_reply_to)
             WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid, references, in_reply_to],
        )?;
        let parent = in_reply_to.map(str::to_string).or(known_parent);
        // Le COMPTE, pas la boîte (ADR 0009). Les deux sont des `i64` :
        // le compilateur ne peut pas distinguer l'un de l'autre, et se
        // tromper ici ne casserait rien — cela rattacherait simplement les
        // messages au mauvais espace de fils, en silence.
        let (account_id, threaded): (i64, bool) = tx.query_row(
            "SELECT account_id, threaded FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        // Hors portée, les en-têtes sont conservés — ils servent la
        // recherche, et ils resserviront si la boîte entre un jour dans la
        // portée — mais ils ne rattachent rien (ADR 0010 §3).
        if !threaded {
            tx.commit()?;
            return Ok(false);
        }
        let thread = thread::attach(
            &tx,
            account_id,
            message_id.as_deref(),
            parent.as_deref(),
            Some(references),
        )?;
        tx.execute(
            "UPDATE envelopes SET thread_id = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid, thread],
        )?;
        thread::refresh(&tx, thread)?;
        if let Some(previous) = before.filter(|previous| *previous != thread) {
            thread::refresh(&tx, previous)?;
        }
        tx.commit()?;
        Ok(before != Some(thread))
    }

    /// Supprime les enveloppes absentes du serveur ; retourne leur nombre.
    pub fn remove_absent(
        &mut self,
        mailbox_id: i64,
        present: &HashSet<Uid>,
    ) -> Result<usize, Error> {
        let local: Vec<Uid> = self
            .0
            .prepare("SELECT uid FROM envelopes WHERE mailbox_id = ?1")?
            .query_map([mailbox_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let stale: Vec<Uid> = local
            .into_iter()
            .filter(|uid| !present.contains(uid))
            .collect();
        let tx = self.0.transaction()?;
        let mut touched: BTreeSet<i64> = BTreeSet::new();
        {
            let mut envelopes =
                tx.prepare("DELETE FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut bodies = tx.prepare("DELETE FROM bodies WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut actions =
                tx.prepare("DELETE FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2")?;
            for uid in &stale {
                // Relever le fil AVANT de supprimer l'enveloppe : après,
                // le lien est perdu et l'agrégat resterait faux.
                if let Some(thread) = thread::thread_of(&tx, mailbox_id, *uid)? {
                    touched.insert(thread);
                }
                search::deindex_message(&tx, mailbox_id, *uid)?;
                envelopes.execute(params![mailbox_id, uid])?;
                bodies.execute(params![mailbox_id, uid])?;
                actions.execute(params![mailbox_id, uid])?;
            }
            for thread in &touched {
                thread::refresh(&tx, *thread)?;
            }
        }
        tx.commit()?;
        Ok(stale.len())
    }

    /// Retire localement une enveloppe et son corps (archivage/suppression
    /// optimiste) ; le serveur suivra via la file d'actions.
    pub fn remove_local(&self, mailbox_id: i64, uid: Uid) -> Result<(), Error> {
        let thread = thread::thread_of(&self.0, mailbox_id, uid)?;
        search::deindex_message(&self.0, mailbox_id, uid)?;
        self.0.execute(
            "DELETE FROM bodies WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
        )?;
        self.0.execute(
            "DELETE FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
        )?;
        if let Some(thread) = thread {
            thread::refresh(&self.0, thread)?;
        }
        Ok(())
    }

    /// Applique localement un changement lu/non-lu (optimisme UI).
    /// Retourne `false` si l'enveloppe était déjà dans cet état.
    pub fn set_seen_local(&self, mailbox_id: i64, uid: Uid, seen: bool) -> Result<bool, Error> {
        let changed = self.0.execute(
            "UPDATE envelopes SET seen = ?3
             WHERE mailbox_id = ?1 AND uid = ?2 AND seen != ?3",
            params![mailbox_id, uid, seen],
        )?;
        if changed > 0 {
            // Le compteur de non-lus du fil vient de bouger. L'oublier
            // laisserait une conversation en gras alors qu'on vient de
            // lire son dernier message non lu.
            if let Some(thread) = thread::thread_of(&self.0, mailbox_id, uid)? {
                thread::refresh(&self.0, thread)?;
            }
        }
        Ok(changed > 0)
    }

    /// Applique localement l'étoile (optimisme UI).
    /// Retourne `false` si l'enveloppe était déjà dans cet état.
    pub fn set_flagged_local(
        &self,
        mailbox_id: i64,
        uid: Uid,
        flagged: bool,
    ) -> Result<bool, Error> {
        let changed = self.0.execute(
            "UPDATE envelopes SET flagged = ?3
             WHERE mailbox_id = ?1 AND uid = ?2 AND flagged != ?3",
            params![mailbox_id, uid, flagged],
        )?;
        Ok(changed > 0)
    }

    /// Journalise une intention à rejouer vers le serveur.
    pub fn enqueue_action(&self, mailbox_id: i64, uid: Uid, action: Action) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, action.to_kind()],
        )?;
        Ok(())
    }

    /// La file d'actions, dans l'ordre d'émission.
    pub fn pending_actions(&self, mailbox_id: i64) -> Result<Vec<PendingAction>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT id, uid, kind FROM pending_actions WHERE mailbox_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map([mailbox_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get(1)?, row.get::<_, String>(2)?))
            })?
            .collect::<Result<Vec<(i64, Uid, String)>, _>>()?;
        rows.into_iter()
            .map(|(id, uid, kind)| {
                let action = Action::parse(&kind)
                    .ok_or_else(|| Error::Corrupt(format!("action inconnue : {kind}")))?;
                Ok(PendingAction { id, uid, action })
            })
            .collect()
    }

    pub fn remove_action(&self, action_id: i64) -> Result<(), Error> {
        self.0
            .execute("DELETE FROM pending_actions WHERE id = ?1", [action_id])?;
        Ok(())
    }

    /// Corps HTML brut (pré-assainissement) d'un message, s'il est en cache.
    pub fn body(&self, account_id: i64, mailbox: &str, uid: Uid) -> Result<Option<String>, Error> {
        let body = self
            .0
            .query_row(
                "SELECT b.html FROM bodies b JOIN mailboxes m ON m.id = b.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND b.uid = ?3",
                params![account_id, mailbox, uid],
                |row| row.get(0),
            )
            .optional()?;
        Ok(body)
    }

    /// Enregistre un corps, son index de recherche et la description de
    /// ses pièces jointes — **dans une seule transaction**.
    ///
    /// Les trois se lisent dans les mêmes octets et n'ont de sens
    /// qu'ensemble : un corps sans son index sortirait des recherches, un
    /// corps sans ses pièces jointes les rendrait invisibles jusqu'au
    /// prochain re-téléchargement. Un crash entre deux écritures ne doit
    /// jamais pouvoir produire cet état.
    pub fn save_body(
        &self,
        mailbox_id: i64,
        uid: Uid,
        html: &str,
        attachments: &[Attachment],
    ) -> Result<(), Error> {
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO bodies (mailbox_id, uid, html, scanned, preview)
             VALUES (?1, ?2, ?3, 1, ?4)",
            params![mailbox_id, uid, html, crate::body::extraire_apercu(html)],
        )?;
        // Remplacement intégral : un message re-téléchargé dont une pièce
        // aurait disparu ne doit pas garder l'ancienne ligne fantôme.
        tx.execute(
            "DELETE FROM attachments WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
        )?;
        for attachment in attachments {
            tx.execute(
                "INSERT INTO attachments (mailbox_id, uid, idx, name, mime, size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    mailbox_id,
                    uid,
                    attachment.index as i64,
                    attachment.name,
                    attachment.mime,
                    attachment.size as i64
                ],
            )?;
        }
        if let Some((subject, sender, sender_address)) = tx
            .query_row(
                "SELECT subject, sender, sender_address
                 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?
        {
            search::index_message(
                &tx,
                mailbox_id,
                uid,
                subject.as_deref(),
                sender.as_deref(),
                sender_address.as_deref(),
                Some(html),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Remplace la liste des dossiers connus d'un compte.
    ///
    /// Remplacement intégral et transactionnel : un dossier supprimé côté
    /// serveur ne doit pas rester proposé comme destination — le
    /// déplacement échouerait au rejeu, longtemps après le clic.
    pub fn replace_folders(&self, account_id: i64, folders: &[Folder]) -> Result<(), Error> {
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM folders WHERE account_id = ?1",
            params![account_id],
        )?;
        for folder in folders {
            tx.execute(
                "INSERT OR REPLACE INTO folders (account_id, wire, display, selectable)
                 VALUES (?1, ?2, ?3, ?4)",
                params![account_id, folder.wire, folder.display, folder.selectable],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Les dossiers connus d'un compte — lecture LOCALE, jamais de réseau.
    pub fn folders(&self, account_id: i64) -> Result<Vec<Folder>, Error> {
        let mut statement = self.0.prepare(
            "SELECT wire, display, selectable FROM folders
             WHERE account_id = ?1 ORDER BY display",
        )?;
        let rows = statement.query_map(params![account_id], |row| {
            Ok(Folder {
                wire: row.get(0)?,
                display: row.get(1)?,
                selectable: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Rattrape l'aperçu des corps écrits AVANT la colonne `preview` —
    /// par lots bornés. Appelé par le shell au fil de son sondage :
    /// jamais sur le chemin d'ouverture (budget démarrage < 1 s, leçon
    /// de la chasse aux orphelins), jamais au défilement. Rend le nombre
    /// de retardataires restants — zéro quand la passe est soldée.
    pub fn preview_catchup(&self, limit: usize) -> Result<u64, Error> {
        let lot: Vec<(i64, Uid, String)> = self
            .0
            .prepare("SELECT mailbox_id, uid, html FROM bodies WHERE preview IS NULL LIMIT ?1")?
            .query_map(params![limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        if !lot.is_empty() {
            let tx = self.0.unchecked_transaction()?;
            for (mailbox_id, uid, html) in &lot {
                tx.execute(
                    "UPDATE bodies SET preview = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid, crate::body::extraire_apercu(html)],
                )?;
            }
            tx.commit()?;
        }
        let restants: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM bodies WHERE preview IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(restants as u64)
    }

    /// Les pièces jointes connues d'un message, dans l'ordre du MIME.
    ///
    /// Vide tant que le corps n'a pas été rapatrié : c'est la même
    /// condition que la recherche dans le texte, et le rattrapage la
    /// lève pour tout l'horizon de récence.
    pub fn attachments(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Vec<Attachment>, Error> {
        let mut statement = self.0.prepare(
            "SELECT a.idx, a.name, a.mime, a.size
             FROM attachments a
             JOIN mailboxes m ON m.id = a.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2 AND a.uid = ?3
             ORDER BY a.idx",
        )?;
        let rows = statement.query_map(params![account_id, mailbox, uid], |row| {
            Ok(Attachment {
                index: row.get::<_, i64>(0)? as usize,
                name: row.get(1)?,
                mime: row.get(2)?,
                size: row.get::<_, i64>(3)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Les messages NON LUS arrivés après `uid_gt`, du plus ancien au
    /// plus récent — la matière des notifications.
    ///
    /// Le critère est l'UID, pas la date : c'est l'ordre d'arrivée que
    /// le serveur garantit, et c'est lui qui distingue « nouveau » de
    /// « ancien mais récemment daté ». Les messages déjà lus ailleurs
    /// sont exclus : notifier un message que l'utilisateur vient de lire
    /// sur son téléphone est du bruit pur.
    pub fn new_unread_after(
        &self,
        account_id: i64,
        mailbox: &str,
        uid_gt: Uid,
        limit: usize,
    ) -> Result<Vec<Envelope>, Error> {
        let mut statement = self.0.prepare(
            "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                    e.date_epoch, e.seen, e.flagged, e.in_reply_to
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND e.uid > ?3 AND e.seen = 0
             ORDER BY e.uid
             LIMIT ?4",
        )?;
        let rows = statement.query_map(
            params![account_id, mailbox, uid_gt, limit as i64],
            row_to_envelope,
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Les messages RÉCENTS dont le corps manque encore, du plus récent au
    /// plus ancien — le travail du rattrapage ([ADR 0007](../../../docs/adr/0007-rattrapage-des-corps.md)).
    ///
    /// `since_epoch` borne le coût : c'est l'horizon de récence. L'ordre
    /// décroissant rend la reprise après coupure naturelle — on redemande
    /// simplement la liste, les corps déjà écrits n'y sont plus.
    ///
    /// Un message sans date est TOUJOURS éligible — révisé par
    /// l'ADR 0010. L'ancienne règle l'excluait comme « non situable dans
    /// l'horizon » ; depuis que la production ne borne plus rien
    /// ([`crate::NO_HORIZON`]), l'exclure serait un trou silencieux : un
    /// message dont la date ne se lit pas n'aurait jamais de corps, donc
    /// jamais de recherche, sans que rien ne le signale. Il passe en
    /// dernier (les NULL ferment un tri DESC) : le doute ne coûte que son
    /// rang.
    pub fn bodies_to_backfill(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
        limit: usize,
    ) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND NOT EXISTS (
                   SELECT 1 FROM bodies b
                    WHERE b.mailbox_id = e.mailbox_id AND b.uid = e.uid
                      AND b.scanned = 1
               )
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?4",
        )?;
        let uids = stmt
            .query_map(
                params![account_id, mailbox, since_epoch, limit as i64],
                |row| row.get(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    /// Les messages dont les en-têtes de fil n'ont pas encore été lus, du
    /// plus récent au plus ancien.
    ///
    /// `refs IS NULL` = jamais lu. Un message sans `References` reçoit
    /// `""` et sort définitivement de cette liste.
    pub fn thread_headers_to_backfill(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
        limit: usize,
    ) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND e.refs IS NULL
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?4",
        )?;
        let uids = stmt
            .query_map(
                params![account_id, mailbox, since_epoch, limit as i64],
                |row| row.get(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    /// Combien de messages attendent encore leurs en-têtes de fil.
    pub fn thread_headers_pending_count(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
    ) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND e.refs IS NULL",
            params![account_id, mailbox, since_epoch],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Combien de messages attendent encore leur corps dans l'horizon —
    /// de quoi afficher un avancement honnête.
    pub fn bodies_pending_count(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
    ) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND NOT EXISTS (
                   SELECT 1 FROM bodies b
                    WHERE b.mailbox_id = e.mailbox_id AND b.uid = e.uid
                      AND b.scanned = 1
               )",
            params![account_id, mailbox, since_epoch],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Une page d'enveloppes d'UN compte, les plus récentes d'abord.
    pub fn recent(
        &self,
        account_id: i64,
        mailbox: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Envelope>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                    e.date_epoch, e.seen, e.flagged, e.in_reply_to
             FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt
            .query_map(
                params![account_id, mailbox, limit as i64, offset as i64],
                row_to_envelope,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// La boîte unifiée : la même boîte (INBOX) de TOUS les comptes,
    /// fusionnée par date — le cœur produit du multi-comptes. Chaque
    /// ligne porte son compte : un UID seul n'identifie plus un message.
    pub fn unified_recent(&self, offset: usize, limit: usize) -> Result<Vec<UnifiedRow>, Error> {
        // Une ligne par CONVERSATION, représentée par son dernier message.
        //
        // Le départ se fait depuis `threads`, pas depuis `envelopes` : un
        // `GROUP BY thread_id` avec un `MAX(date)` obligerait SQLite à
        // parcourir puis trier les 200 000 enveloppes à chaque page de
        // défilement. Ici l'index `idx_threads_date_globale` porte à la
        // fois le tri et la pagination — le coût d'une page ne dépend
        // plus de la taille de la boîte. C'est l'agrégat matérialisé qui
        // paie ça, et il se maintient dans la transaction d'écriture.
        //
        // La pagination vit dans une SOUS-REQUÊTE sur `threads` seul :
        // voir `unified_page_sql`.
        let mut stmt = self.0.prepare(&unified_page_sql(false, false))?;
        let rows = stmt
            .query_map(params![limit as i64, offset as i64], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Total de la boîte unifiée — en CONVERSATIONS, puisque c'est ce que
    /// la liste affiche. Compter les messages ferait défiler dans le vide.
    //
    // (`unified_page_sql`, plus bas, porte la requête de la page.)
    pub fn unified_count(&self) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM threads WHERE inbox_size > 0",
            [],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Les messages d'une conversation, du plus ancien au plus récent —
    /// l'ordre de lecture d'un échange.
    pub fn thread_messages(&self, thread_id: i64) -> Result<Vec<UnifiedRow>, Error> {
        // Jointure sur `threads`, et non le mapping « message seul » :
        // chaque message doit repartir en connaissant la taille de SON
        // fil. Sans elle il vaudrait 1, et l'écran conclurait qu'il n'y a
        // pas de conversation à montrer — au moment précis où on la
        // parcourt.
        let mut stmt = self.0.prepare(&format!(
            "{SELECT_UNIFIED}{THREAD_AGGREGATE}
             FROM envelopes e
             JOIN threads t ON t.id = e.thread_id
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             WHERE e.thread_id = ?1
             ORDER BY e.date_epoch ASC, e.uid ASC"
        ))?;
        let rows = stmt
            .query_map([thread_id], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Une enveloppe précise — le contexte nécessaire pour répondre
    /// (adresse brute de l'expéditeur, Message-ID du fil).
    pub fn envelope(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<Envelope>, Error> {
        let envelope = self
            .0
            .query_row(
                "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                        e.date_epoch, e.seen, e.flagged, e.in_reply_to
                 FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND e.uid = ?3",
                params![account_id, mailbox, uid],
                row_to_envelope,
            )
            .optional()?;
        Ok(envelope)
    }

    pub fn count(&self, mailbox_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes WHERE mailbox_id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn max_uid(&self, mailbox_id: i64) -> Result<Uid, Error> {
        let max: Uid = self.0.query_row(
            "SELECT COALESCE(MAX(uid), 0) FROM envelopes WHERE mailbox_id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        Ok(max)
    }
}

/// Fait évoluer en place une base d'une version précédente : les colonnes
/// s'ajoutent sans perdre ce qui est déjà là, et la bascule multi-comptes
/// (Phase 3) reconstruit les tables dont les contraintes changent.
/// Configuration serveur d'un compte IMAP/SMTP générique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountConfig {
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub username: Option<String>,
}

/// La requête d'une page de la boîte unifiée.
///
/// Isolée pour qu'un test puisse interroger **son** plan d'exécution, et
/// non une copie qui divergerait le jour où l'une des deux change. Le
/// coût de cette requête est le chemin le plus chaud du produit.
pub(crate) fn unified_page_sql(par_compte: bool, non_lues: bool) -> String {
    // La pagination (`LIMIT`/`OFFSET`) s'applique dans une sous-requête
    // sur `threads` SEUL, pas sur la jointure : `OFFSET` produit puis
    // jette chaque ligne sautée, donc tout ce qui se calcule par ligne —
    // la triple jointure et l'`EXISTS` corrélé sur `attachments` de
    // SELECT_UNIFIED — se payait pour les 200 000 lignes d'un saut
    // profond. Mesuré (gate P1 de la refonte, 205 050 conversations) :
    // 252,6 ms à l'offset 200 000, croissance linéaire. Avec le squelette
    // en sous-requête, le saut ne parcourt que l'index partiel
    // `idx_threads_date_globale` — qui porte la clé de tri COMPLÈTE
    // (last_epoch DESC, last_uid DESC, account_id) et le filtre
    // `inbox_size > 0` — et les jointures ne s'exécutent que sur les
    // `limit` lignes retenues.
    //
    // Le ORDER BY externe re-trie les lignes retenues avec la même clé :
    // il garantit l'ordre final quelle que soit la stratégie de jointure,
    // pour le prix d'un tri de `limit` lignes.
    // `par_compte` ajoute le filtre `account_id = ?3` de la nav v2
    // (« Boîtes » de l'écran 02) : même squelette, l'index préfixé
    // `idx_threads_date (account_id, …)` porte alors tri et pagination.
    // `non_lues` est l'onglet « Non lus » du prototype — filtré ICI, pas
    // côté client : 331 conversations sur 2 929 au terrain, une page ne
    // doit transporter que ce qu'elle affiche.
    let filtre = if par_compte {
        " AND account_id = ?3"
    } else {
        ""
    };
    let non_lues_seulement = if non_lues { " AND unseen > 0" } else { "" };
    format!(
        "{SELECT_UNIFIED}{THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0{filtre}{non_lues_seulement}
                ORDER BY last_epoch DESC, last_uid DESC, account_id
                LIMIT ?1 OFFSET ?2) t
         JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
         JOIN mailboxes m ON m.id = e.mailbox_id
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
         ORDER BY t.last_epoch DESC, t.last_uid DESC, a.id"
    )
}

fn migrate(conn: &Connection) -> Result<(), Error> {
    migrate_multi_account(conn)?;
    add_missing_columns(
        conn,
        "drafts",
        &[
            ("account_id", "INTEGER NOT NULL DEFAULT 1"),
            ("reply_to_mailbox", "TEXT"),
        ],
    )?;
    // ADR 0010 : la portee du regroupement devient explicite. Les boites
    // deja en base sont INBOX et « Envoyes » — toutes deux dedans, d'ou
    // le defaut a 1. Une base heritee garde donc exactement les fils
    // qu'elle avait : la migration ne change rien a ce qui est affiche.
    add_missing_columns(
        conn,
        "mailboxes",
        &[("threaded", "INTEGER NOT NULL DEFAULT 1")],
    )?;
    add_missing_columns(conn, "accounts", &[("sent_mailbox", "TEXT")])?;
    add_missing_columns(
        conn,
        "mailboxes",
        &[("remote_total", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    // ADR 0017 : le UIDNEXT vu au dernier relevé — NULL tant qu'aucune
    // relève gardée n'a eu lieu, donc une base héritée relève tout à son
    // premier cycle (conservateur), puis devient sobre.
    add_missing_columns(conn, "mailboxes", &[("remote_uidnext", "INTEGER")])?;
    add_missing_columns(
        conn,
        "outbox",
        &[("account_id", "INTEGER NOT NULL DEFAULT 1")],
    )?;
    add_missing_columns(
        conn,
        "envelopes",
        &[
            ("sender_address", "TEXT"),
            ("message_id", "TEXT"),
            ("flagged", "INTEGER NOT NULL DEFAULT 0"),
            ("in_reply_to", "TEXT"),
            ("refs", "TEXT"),
            // NULL = « pas encore rattaché ». C'est ce que
            // `thread::migrate_threads` cherche, plus bas.
            ("thread_id", "INTEGER"),
        ],
    )?;
    add_missing_columns(
        conn,
        "drafts",
        &[("remote_uid", "INTEGER"), ("pushed_epoch", "INTEGER")],
    )?;
    // Les corps deja en base valent 0 : ils datent d'avant les pieces
    // jointes, et le rattrapage devra les relire une fois.
    add_missing_columns(conn, "bodies", &[("scanned", "INTEGER NOT NULL DEFAULT 0")])?;
    // L'aperçu de liste (écran 02 de la refonte) se calcule à l'ÉCRITURE
    // du corps ; les corps antérieurs le rattrapent PAR LOTS
    // (`preview_catchup`, appelé par le shell au fil du sondage) — jamais
    // sur le chemin d'ouverture ni au défilement. L'index partiel rend la
    // sonde « des retardataires ? » gratuite une fois la passe soldée.
    add_missing_columns(conn, "bodies", &[("preview", "TEXT")])?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_bodies_apercu_manquant
             ON bodies(mailbox_id, uid) WHERE preview IS NULL;",
    )?;
    // La sonde d'exclusion des intégrales (nav, catégorie Archives sur
    // Gmail) cherche par message_id : sans cet index, chaque ligne de
    // « Tous les messages » paierait un parcours de table.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_message
             ON envelopes(message_id) WHERE message_id IS NOT NULL;",
    )?;
    // Réparation des aperçus extraits par le premier décodeur, qui
    // laissait passer les entités numériques (&#233;) et nommées
    // (&eacute;, &zwnj;…) — défaut vu au terrain. Remettre à NULL suffit :
    // le rattrapage par lots les recalcule avec le décodeur complet, hors
    // du chemin d'ouverture. Le critère est LE scanner du décodeur
    // lui-même (pas un motif SQL approximatif). UNE seule passe, tenue
    // par un marqueur : un corps double-encodé (« &amp;gt; ») produit
    // légitimement « &gt; » dans l'aperçu neuf — sans le marqueur, la
    // réparation le remettrait à NULL à chaque ouverture, pour rien.
    conn.execute_batch("CREATE TABLE IF NOT EXISTS reparations (nom TEXT PRIMARY KEY);")?;
    let deja_faite: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'apercus-entites'")?
        .exists([])?;
    if !deja_faite {
        let mut stmt = conn.prepare(
            "SELECT mailbox_id, uid, preview FROM bodies
                 WHERE preview IS NOT NULL AND preview LIKE '%&%'",
        )?;
        let pollues: Vec<(i64, u32)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(Result::ok)
            .filter(|(_, _, p)| crate::body::contient_entite_residuelle(p))
            .map(|(m, u, _)| (m, u))
            .collect();
        drop(stmt);
        for (mailbox_id, uid) in pollues {
            conn.execute(
                "UPDATE bodies SET preview = NULL WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
            )?;
        }
        conn.execute_batch("INSERT INTO reparations (nom) VALUES ('apercus-entites');")?;
    }
    // Réparation des corps mutilés au décodage — défaut vu au terrain
    // (25 corps sur la base de mesure). Deux causes, corrigées côté
    // mail-imap : les charsets multi-octets (gb2312…) exigeaient la
    // feature `full_encoding` de mail-parser, et un charset absent
    // tombait en UTF-8 avec remplacement au lieu du windows-1252 de fait.
    // Supprimer la ligne suffit : le rattrapage (`bodies_to_backfill`)
    // retélécharge tout message sans corps, et `save_body` refait au
    // passage l'aperçu, l'index de recherche et les pièces. Les U+FFFD
    // authentiques (envoyés tels quels) reviendront identiques — c'est un
    // retéléchargement pour rien, mais UNE seule fois, tenu par le
    // marqueur.
    let deja_faite: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'corps-fffd'")?
        .exists([])?;
    if !deja_faite {
        conn.execute_batch(
            "DELETE FROM bodies WHERE html LIKE '%' || char(65533) || '%';
             INSERT INTO reparations (nom) VALUES ('corps-fffd');",
        )?;
    }
    add_missing_columns(
        conn,
        "accounts",
        &[
            ("imap_host", "TEXT"),
            ("imap_port", "INTEGER"),
            ("smtp_host", "TEXT"),
            ("smtp_port", "INTEGER"),
            ("username", "TEXT"),
        ],
    )?;
    search::migrate_search(conn)?;
    // L'index vient APRÈS `add_missing_columns`, pas dans `SCHEMA` : sur
    // une base héritée, `CREATE TABLE IF NOT EXISTS envelopes` ne fait
    // rien et la colonne `thread_id` n'existe pas encore au moment où le
    // schéma s'exécute. Deux tests de migration l'ont prouvé.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_thread
             ON envelopes(thread_id, date_epoch DESC);",
    )?;
    // L'adoption des fils ne vit PAS ici : elle appartient à l'unité
    // transactionnelle de `init_with`, pour être rembobinable (§8). Elle
    // vient après ce module — la colonne et l'index doivent exister avant
    // d'adopter les messages hérités.
    Ok(())
}

/// Bascule Phase 2 → 3 : les contraintes de trois tables changent
/// (UNIQUE et clés par compte) — SQLite exige une reconstruction. Les
/// données existantes sont adoptées par un compte « en attente » (email
/// vide) que la première connexion revendiquera : en pratique, le même
/// compte Gmail qu'avant la mise à jour. Zéro perte, prouvé par test.
fn migrate_multi_account(conn: &Connection) -> Result<(), Error> {
    if table_columns(conn, "mailboxes")?.contains("account_id") {
        return Ok(());
    }
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         BEGIN;
         INSERT INTO accounts (id, email, provider) VALUES (1, '', 'gmail');

         CREATE TABLE mailboxes_v3 (
             id             INTEGER PRIMARY KEY,
             account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
             name           TEXT NOT NULL,
             uid_validity   INTEGER NOT NULL,
             last_uid       INTEGER NOT NULL DEFAULT 0,
             highest_modseq INTEGER,
             UNIQUE (account_id, name)
         );
         INSERT INTO mailboxes_v3 (id, account_id, name, uid_validity, last_uid, highest_modseq)
             SELECT id, 1, name, uid_validity, last_uid, highest_modseq FROM mailboxes;
         DROP TABLE mailboxes;
         ALTER TABLE mailboxes_v3 RENAME TO mailboxes;

         CREATE TABLE drafts_remote_v3 (
             account_id   INTEGER PRIMARY KEY,
             uid_validity INTEGER NOT NULL
         );
         INSERT INTO drafts_remote_v3 (account_id, uid_validity)
             SELECT 1, uid_validity FROM drafts_remote;
         DROP TABLE drafts_remote;
         ALTER TABLE drafts_remote_v3 RENAME TO drafts_remote;

         CREATE TABLE draft_tombstones_v3 (
             account_id INTEGER NOT NULL,
             remote_uid INTEGER NOT NULL,
             PRIMARY KEY (account_id, remote_uid)
         );
         INSERT INTO draft_tombstones_v3 (account_id, remote_uid)
             SELECT 1, remote_uid FROM draft_tombstones;
         DROP TABLE draft_tombstones;
         ALTER TABLE draft_tombstones_v3 RENAME TO draft_tombstones;

         COMMIT;
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get(1))?
        .collect::<Result<_, _>>()?;
    Ok(columns)
}

fn add_missing_columns(
    conn: &Connection,
    table: &str,
    columns: &[(&str, &str)],
) -> Result<(), Error> {
    let existing = table_columns(conn, table)?;
    for (column, ddl) in columns {
        if !existing.contains(*column) {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}"),
                [],
            )?;
        }
    }
    Ok(())
}

/// Mapping partagé par toutes les lectures d'enveloppes — l'ordre des
/// colonnes est celui des SELECT ci-dessus.
fn row_to_envelope(row: &rusqlite::Row<'_>) -> rusqlite::Result<Envelope> {
    Ok(Envelope {
        uid: row.get(0)?,
        subject: row.get(1)?,
        sender: row.get(2)?,
        sender_address: row.get(3)?,
        message_id: row.get(4)?,
        date: row
            .get::<_, Option<i64>>(5)?
            .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
        seen: row.get(6)?,
        flagged: row.get(7)?,
        in_reply_to: row.get(8)?,
    })
}

/// Mapping partagé par les lectures de la boîte unifiée — l'ordre des
/// colonnes est celui de [`SELECT_UNIFIED`].
pub(crate) fn row_to_unified(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnifiedRow> {
    let attachment_count = row.get::<_, i64>(10)?.max(0) as u32;
    Ok(UnifiedRow {
        account_id: row.get(0)?,
        account_email: row.get(1)?,
        envelope: Envelope {
            uid: row.get(2)?,
            subject: row.get(3)?,
            sender: row.get(4)?,
            sender_address: row.get(5)?,
            message_id: row.get(6)?,
            date: row
                .get::<_, Option<i64>>(7)?
                .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
            seen: row.get(8)?,
            flagged: row.get(9)?,
            in_reply_to: row.get(12)?,
        },
        mailbox: row.get(13)?,
        has_attachment: attachment_count > 0,
        attachment_count,
        preview: row.get(14)?,
        thread_id: row.get(11)?,
        // Valeurs d'un message vu SEUL — c'est le cas de la recherche, qui
        // ne joint pas `threads`. La liste groupée les écrase avec
        // l'agrégat réel via [`row_to_threaded`].
        thread_size: 1,
        thread_unseen: u32::from(!row.get::<_, bool>(8)?),
    })
}

/// Mapping de la liste groupée : les colonnes unifiées, puis l'agrégat du
/// fil ajouté par [`THREAD_AGGREGATE`].
pub(crate) fn row_to_threaded(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnifiedRow> {
    Ok(UnifiedRow {
        thread_size: row.get(15)?,
        thread_unseen: row.get(16)?,
        ..row_to_unified(row)?
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn envelope(uid: Uid, subject: &str, epoch: i64, seen: bool) -> Envelope {
        Envelope {
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Alice Martin".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<m{uid}@example.com>")),
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen,
            flagged: uid.is_multiple_of(2),
        }
    }

    fn test_account(store: &Store) -> i64 {
        store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap()
    }

    fn store_with_mailbox() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = test_account(&store);
        let id = store.create_mailbox(account, "INBOX", 1).unwrap();
        (store, id)
    }

    fn recent(store: &Store, offset: usize, limit: usize) -> Vec<Envelope> {
        store
            .recent(test_account(store), "INBOX", offset, limit)
            .unwrap()
    }

    /// Une préférence jamais posée répond le défaut demandé ; posée, elle
    /// se relit telle quelle et s'écrase sans doublon.
    #[test]
    fn bool_pref_default_then_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.bool_pref("arrival_bubbles", true).unwrap());
        assert!(!store.bool_pref("arrival_bubbles", false).unwrap());
        store.set_bool_pref("arrival_bubbles", false).unwrap();
        assert!(!store.bool_pref("arrival_bubbles", true).unwrap());
        store.set_bool_pref("arrival_bubbles", true).unwrap();
        assert!(store.bool_pref("arrival_bubbles", false).unwrap());
    }

    /// Le repère de la relève gardée (ADR 0017) : jamais posé -> `None`
    /// (une base héritée relève tout à son premier cycle), posé -> relu.
    #[test]
    fn remote_uidnext_absent_puis_pose() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        // NULL tant qu'aucune relève gardée n'a eu lieu : une base
        // héritée relève tout à son premier cycle (ADR 0017).
        assert_eq!(store.remote_uidnext(mailbox).unwrap(), None);
        store.set_remote_uidnext(mailbox, 101).unwrap();
        assert_eq!(store.remote_uidnext(mailbox).unwrap(), Some(101));
        assert_eq!(store.envelope_count(mailbox).unwrap(), 0);
        assert!(!store.has_pending_actions(mailbox).unwrap());
    }

    /// Le pendant texte : jamais posée -> `None` (le défaut appartient à
    /// l'appelant), posée -> relue telle quelle, écrasée sans doublon.
    #[test]
    fn text_pref_none_then_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.text_pref("lang").unwrap(), None);
        store.set_text_pref("lang", "en").unwrap();
        assert_eq!(store.text_pref("lang").unwrap(), Some("en".to_string()));
        store.set_text_pref("lang", "fr").unwrap();
        assert_eq!(store.text_pref("lang").unwrap(), Some("fr".to_string()));
    }

    #[test]
    fn roundtrips_all_envelope_fields() {
        let (mut store, id) = store_with_mailbox();
        let original = envelope(7, "Sujet accentué : été", 1_700_000_000, true);
        store
            .upsert_envelopes(id, std::slice::from_ref(&original))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![original]);
    }

    #[test]
    fn roundtrips_envelope_without_optional_fields() {
        let (mut store, id) = store_with_mailbox();
        let bare = Envelope {
            uid: 1,
            subject: None,
            sender: None,
            sender_address: None,
            message_id: None,
            in_reply_to: None,
            date: None,
            seen: false,
            flagged: false,
        };
        store
            .upsert_envelopes(id, std::slice::from_ref(&bare))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![bare]);
    }

    /// L'ordre du rattrapage est un choix de PRODUIT, pas un accident de
    /// tri SQL : INBOX d'abord, les envois ensuite, le reste par nom. Un
    /// serveur qui liste « Archive » avant INBOX ne doit pas faire
    /// rattraper 80 000 corps d'archive avant le courrier que la liste
    /// affiche.
    #[test]
    fn les_boites_se_rattrapent_reception_d_abord() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(account, "Archive", 1).unwrap();
        store.create_mailbox(account, "Corbeille", 1).unwrap();
        store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .create_mailbox(account, "Messages envoyés", 1)
            .unwrap();
        store
            .set_thread_scope(account, Some("Messages envoyés"))
            .unwrap();

        assert_eq!(
            store.mailbox_names(account).unwrap(),
            vec!["INBOX", "Messages envoyés", "Archive", "Corbeille"]
        );
    }

    /// Le retrait d'un compte ne laisse RIEN derrière lui : ni les lignes
    /// en cascade (boîtes, enveloppes, corps), ni celles sans clé
    /// étrangère (brouillons, boîte d'envoi, index de recherche) — et le
    /// compte voisin garde tout, recherche comprise.
    #[test]
    fn delete_account_efface_tout_et_ne_touche_pas_le_voisin() {
        let mut store = Store::open_in_memory().unwrap();
        let parti = store
            .adopt_or_create_account("part@exemple.fr", "gmail")
            .unwrap();
        let voisin = store
            .adopt_or_create_account("reste@exemple.fr", "gmail")
            .unwrap();
        for (account, sujet) in [(parti, "Facture du départ"), (voisin, "Devis qui reste")] {
            let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(mailbox, &[envelope(1, sujet, 100, false)])
                .unwrap();
            store.save_body(mailbox, 1, "<p>corps</p>", &[]).unwrap();
            store
                .save_draft(
                    account,
                    None,
                    None,
                    crate::DraftContent {
                        to_raw: "a@b.fr",
                        subject: sujet,
                        body: "brouillon",
                        reply_to_uid: None,
                        reply_to_mailbox: None,
                    },
                )
                .unwrap();
            store
                .enqueue_outbox(
                    account,
                    &crate::compose::Draft {
                        message_id: format!("<sortant-{account}@exemple.fr>"),
                        from: "moi@exemple.fr".to_string(),
                        to: vec!["a@b.fr".to_string()],
                        subject: sujet.to_string(),
                        body_text: "corps".to_string(),
                        in_reply_to: None,
                    },
                )
                .unwrap();
        }

        store.delete_account(parti).unwrap();

        let comptes = store.accounts().unwrap();
        assert_eq!(comptes.len(), 1);
        assert_eq!(comptes[0].email, "reste@exemple.fr");
        for table in [
            "mailboxes",
            "envelopes",
            "bodies",
            "drafts",
            "outbox",
            "search_docs",
        ] {
            let total: i64 = store
                .0
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(total, 1, "{table} : seule la ligne du voisin doit rester");
        }
        assert!(
            store.search("départ", 10).unwrap().is_empty(),
            "le courrier du compte parti ne doit plus sortir en recherche"
        );
        assert_eq!(
            store.search("reste", 10).unwrap().len(),
            1,
            "la recherche du voisin doit survivre au retrait"
        );
    }

    /// ADR 0010 : un message SANS date reste éligible au rattrapage, même
    /// sous un horizon borné. L'ancienne règle l'excluait (« non situable
    /// dans l'horizon ») — un trou silencieux : jamais de corps, donc
    /// jamais de recherche, et rien à l'écran pour le signaler. Le doute
    /// ne coûte plus que son rang : les NULL ferment le tri.
    #[test]
    fn un_message_sans_date_reste_a_rattraper() {
        let (mut store, id) = store_with_mailbox();
        let sans_date = Envelope {
            uid: 9,
            subject: None,
            sender: None,
            sender_address: None,
            message_id: None,
            in_reply_to: None,
            date: None,
            seen: false,
            flagged: false,
        };
        store
            .upsert_envelopes(id, std::slice::from_ref(&sans_date))
            .unwrap();

        let account = test_account(&store);
        let uids = store
            .bodies_to_backfill(account, "INBOX", 1_000_000, 10)
            .unwrap();
        assert_eq!(uids, vec![9], "l'horizon borné n'exclut plus les sans-date");
        assert_eq!(
            store
                .bodies_pending_count(account, "INBOX", 1_000_000)
                .unwrap(),
            1,
            "le compteur d'avancement le voit aussi — sinon la barre mentirait"
        );
    }

    #[test]
    fn upsert_replaces_existing_envelope() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "avant", 100, false)])
            .unwrap();
        store
            .upsert_envelopes(id, &[envelope(1, "après", 100, true)])
            .unwrap();
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].subject.as_deref(), Some("après"));
        assert!(rows[0].seen);
    }

    #[test]
    fn recent_orders_by_date_then_uid_descending() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "ancien", 100, false),
                    envelope(3, "récent", 300, false),
                    envelope(2, "milieu", 200, false),
                ],
            )
            .unwrap();
        let uids: Vec<Uid> = recent(&store, 0, 2).iter().map(|e| e.uid).collect();
        assert_eq!(uids, vec![3, 2]);
    }

    #[test]
    fn remove_absent_deletes_only_missing_uids() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "a", 100, false),
                    envelope(2, "b", 200, false),
                    envelope(3, "c", 300, false),
                ],
            )
            .unwrap();
        let present: HashSet<Uid> = [1, 3].into_iter().collect();
        assert_eq!(store.remove_absent(id, &present).unwrap(), 1);
        assert_eq!(store.count(id).unwrap(), 2);
    }

    #[test]
    fn sync_state_roundtrips_including_modseq() {
        let (store, id) = store_with_mailbox();
        assert_eq!(
            store.sync_state(test_account(&store), "INBOX").unwrap(),
            Some(SyncState {
                mailbox_id: id,
                uid_validity: 1,
                last_uid: 0,
                highest_modseq: None,
            })
        );
        store.update_state(id, 42, Some(9000)).unwrap();
        let state = store
            .sync_state(test_account(&store), "INBOX")
            .unwrap()
            .unwrap();
        assert_eq!(state.last_uid, 42);
        assert_eq!(state.highest_modseq, Some(9000));
    }

    #[test]
    fn sync_state_is_none_for_unknown_mailbox() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.sync_state(test_account(&store), "INBOX").unwrap(),
            None
        );
    }

    #[test]
    fn reset_mailbox_clears_envelopes_and_state() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.update_state(id, 1, Some(5)).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert_eq!(store.count(id).unwrap(), 0);
        let state = store
            .sync_state(test_account(&store), "INBOX")
            .unwrap()
            .unwrap();
        assert_eq!(state.uid_validity, 2);
        assert_eq!(state.last_uid, 0);
        assert_eq!(state.highest_modseq, None);
    }

    #[test]
    fn max_uid_is_zero_for_empty_mailbox() {
        let (store, id) = store_with_mailbox();
        assert_eq!(store.max_uid(id).unwrap(), 0);
    }

    #[test]
    fn recent_pages_with_offset() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &(1..=5)
                    .map(|uid| envelope(uid, "sujet", 100 * i64::from(uid), false))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let page: Vec<Uid> = recent(&store, 2, 2).iter().map(|e| e.uid).collect();
        assert_eq!(page, vec![3, 2], "offset 2 saute les deux plus récents");
        assert!(recent(&store, 10, 5).is_empty());
    }

    #[test]
    fn action_queue_roundtrips_in_emission_order() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 5, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 3, Action::MarkUnseen).unwrap();

        let queued = store.pending_actions(id).unwrap();
        assert_eq!(queued.len(), 2);
        assert_eq!(
            (queued[0].uid, queued[0].action.clone()),
            (5, Action::MarkSeen)
        );
        assert_eq!(
            (queued[1].uid, queued[1].action.clone()),
            (3, Action::MarkUnseen)
        );

        store.remove_action(queued[0].id).unwrap();
        assert_eq!(store.pending_actions(id).unwrap().len(), 1);
    }

    #[test]
    fn set_seen_local_updates_and_reports_actual_change() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();

        assert!(store.set_seen_local(id, 1, true).unwrap());
        assert!(recent(&store, 0, 1)[0].seen);
        assert!(
            !store.set_seen_local(id, 1, true).unwrap(),
            "déjà lu : aucun changement à journaliser"
        );
    }

    #[test]
    fn set_flagged_local_updates_and_reports_actual_change() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();

        assert!(store.set_flagged_local(id, 1, true).unwrap());
        assert!(recent(&store, 0, 1)[0].flagged);
        assert!(
            !store.set_flagged_local(id, 1, true).unwrap(),
            "déjà étoilé : aucun changement à journaliser"
        );
    }

    #[test]
    fn remove_local_drops_envelope_and_body() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();

        store.remove_local(id, 1).unwrap();

        assert!(recent(&store, 0, 10).is_empty());
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn reset_mailbox_clears_pending_actions() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    #[test]
    fn body_roundtrips_and_is_none_when_absent() {
        let (store, id) = store_with_mailbox();
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
        store.save_body(id, 1, "<p>bonjour</p>", &[]).unwrap();
        assert_eq!(
            store
                .body(test_account(&store), "INBOX", 1)
                .unwrap()
                .as_deref(),
            Some("<p>bonjour</p>")
        );
    }

    fn pdf(index: usize, name: &str) -> Attachment {
        Attachment {
            index,
            name: name.to_string(),
            mime: "application/pdf".to_string(),
            size: 2048,
        }
    }

    /// Le defaut livre : un corps rapatrie AVANT les pieces jointes n'a
    /// jamais eu son MIME inspecte, et l'information n'est pas
    /// recuperable depuis le HTML stocke. Comme le rattrapage ne
    /// selectionnait que les corps ABSENTS, ces messages n'auraient
    /// jamais montre leurs pieces jointes — soit, en pratique, la
    /// totalite d'une boite deja rattrapee.
    #[test]
    fn a_body_fetched_before_attachments_existed_is_queued_for_a_re_read() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(id, &[envelope(1, "sujet", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>corps</p>", &[]).unwrap();

        // Rien a faire : le corps a ete lu par la version courante.
        assert!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 0);

        // On simule l'heritage : corps present, MIME jamais inspecte.
        store
            .conn()
            .execute("UPDATE bodies SET scanned = 0", [])
            .unwrap();

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1],
            "un corps jamais inspecte doit revenir dans le rattrapage"
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 1);

        // Le relire le sort definitivement de la file.
        store.save_body(id, 1, "<p>corps</p>", &[]).unwrap();
        assert!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 10)
                .unwrap()
                .is_empty()
        );
    }

    /// Un message deja lu ailleurs — telephone, webmail — ne doit pas
    /// declencher de bulle : c'est du bruit pur, et c'est ce qui fait
    /// couper les notifications.
    #[test]
    fn only_genuinely_new_and_unread_messages_are_notifiable() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(10, "ancien", 100, false),
                    envelope(11, "deja lu", 200, true),
                    envelope(12, "vraiment nouveau", 300, false),
                ],
            )
            .unwrap();

        let arrivals = store.new_unread_after(account, "INBOX", 10, 20).unwrap();
        let subjects: Vec<_> = arrivals
            .iter()
            .map(|e| e.subject.clone().unwrap_or_default())
            .collect();
        assert_eq!(subjects, vec!["vraiment nouveau".to_string()]);
    }

    fn folder(wire: &str, display: &str) -> Folder {
        Folder {
            wire: wire.to_string(),
            display: display.to_string(),
            selectable: true,
        }
    }

    /// Choisir une destination doit fonctionner HORS LIGNE : la liste est
    /// donc lue localement, comme les enveloppes. Le nom réseau et le nom
    /// lisible sont conservés tous les deux — perdre le premier rendrait
    /// le déplacement irréalisable au rejeu.
    #[test]
    fn folders_are_cached_locally_with_both_names() {
        let (store, _) = store_with_mailbox();
        let account = test_account(&store);
        assert!(store.folders(account).unwrap().is_empty());

        store
            .replace_folders(account, &[folder("Archiv&AOk-s", "Archivés")])
            .unwrap();

        let cached = store.folders(account).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].wire, "Archiv&AOk-s");
        assert_eq!(cached[0].display, "Archivés");
    }

    /// Un dossier supprimé côté serveur ne doit plus être proposé : le
    /// déplacement échouerait au rejeu, longtemps après le clic — et
    /// l'utilisateur ne ferait plus le lien.
    #[test]
    fn refreshing_folders_drops_the_ones_that_disappeared() {
        let (store, _) = store_with_mailbox();
        let account = test_account(&store);
        store
            .replace_folders(
                account,
                &[folder("Ancien", "Ancien"), folder("Reste", "Reste")],
            )
            .unwrap();

        store
            .replace_folders(account, &[folder("Reste", "Reste")])
            .unwrap();

        let cached = store.folders(account).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].wire, "Reste");
    }

    #[test]
    fn attachments_are_saved_with_the_body_and_read_back_in_order() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        assert!(
            store.attachments(account, "INBOX", 1).unwrap().is_empty(),
            "rien tant que le corps n'est pas rapatrié"
        );

        store
            .save_body(
                id,
                1,
                "<p>ci-joint</p>",
                &[pdf(0, "un.pdf"), pdf(1, "deux.pdf")],
            )
            .unwrap();

        let found = store.attachments(account, "INBOX", 1).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "un.pdf");
        assert_eq!(found[1].name, "deux.pdf");
        assert_eq!(found[1].size, 2048);
    }

    /// Un message re-téléchargé dont une pièce a disparu ne doit pas
    /// garder l'ancienne ligne : l'utilisateur cliquerait sur un fichier
    /// que le serveur ne sert plus, et l'échec n'arriverait qu'au
    /// téléchargement — loin de la cause.
    #[test]
    fn re_saving_replaces_the_attachment_list_instead_of_accumulating() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "un.pdf"), pdf(1, "deux.pdf")])
            .unwrap();

        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "un.pdf")])
            .unwrap();

        let found = store.attachments(account, "INBOX", 1).unwrap();
        assert_eq!(found.len(), 1, "la pièce disparue doit l'être aussi ici");
        assert_eq!(found[0].name, "un.pdf");
    }

    /// Les pièces jointes appartiennent à un message d'un COMPTE : la
    /// même paire (boîte, uid) chez un autre compte ne doit rien voir.
    #[test]
    fn attachments_never_leak_across_accounts() {
        let (store, id) = store_with_mailbox();
        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "prive.pdf")])
            .unwrap();

        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(other, "INBOX", 1).unwrap();

        assert!(store.attachments(other, "INBOX", 1).unwrap().is_empty());
    }

    #[test]
    fn reset_mailbox_clears_bodies_too() {
        let (store, id) = store_with_mailbox();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn envelope_returns_reply_context_fields() {
        let (mut store, id) = store_with_mailbox();
        let original = envelope(7, "sujet", 100, false);
        store
            .upsert_envelopes(id, std::slice::from_ref(&original))
            .unwrap();

        assert_eq!(
            store.envelope(test_account(&store), "INBOX", 7).unwrap(),
            Some(original)
        );
        assert_eq!(
            store.envelope(test_account(&store), "INBOX", 99).unwrap(),
            None
        );
    }

    /// ADR 0011 : sur une base FICHIER, l'ouverture passe en WAL — et le
    /// mode persiste, une base héritée en rollback est convertie. C'est ce
    /// qui empêche « database is locked » quand la jauge d'avancement lit
    /// pendant qu'une synchronisation intégrale écrit — le premier défaut
    /// que le terrain ait rendu sur l'ADR 0010.
    ///
    /// Sur base fichier et non en mémoire, comme le terrain : une base en
    /// mémoire répond « memory » à ce PRAGMA, et le test validerait un
    /// modèle faux.
    #[test]
    fn une_base_fichier_s_ouvre_en_wal() {
        let path = std::env::temp_dir().join(format!("wind-test-wal-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        // Une base héritée, née AVANT le WAL : mode rollback (delete).
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE heritage (id INTEGER)")
                .unwrap();
        }

        {
            let _store = Store::open(&path).unwrap();
            let conn = Connection::open(&path).unwrap();
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .unwrap();
            assert_eq!(mode.to_lowercase(), "wal", "la base héritée est convertie");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// Une base Phase 1 (sans les colonnes de réponse) doit s'ouvrir et
    /// s'enrichir sans perdre les enveloppes déjà synchronisées.
    #[test]
    fn opens_and_migrates_a_phase1_database() {
        let path =
            std::env::temp_dir().join(format!("wind-test-migration-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid        INTEGER NOT NULL,
                    subject    TEXT,
                    sender     TEXT,
                    date_epoch INTEGER,
                    seen       INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 1);
                INSERT INTO envelopes (mailbox_id, uid, subject, sender, date_epoch, seen)
                VALUES (1, 42, 'hérité de la phase 1', 'Alice', 100, 1);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uid, 42);
        assert_eq!(rows[0].subject.as_deref(), Some("hérité de la phase 1"));
        assert_eq!(
            rows[0].sender_address, None,
            "colonne ajoutée par migration : valeur inconnue pour l'existant"
        );
        assert!(
            !rows[0].flagged,
            "étoile absente par défaut après migration"
        );

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Migration Phase 2 → 3 sur une base complète : toutes les données
    /// (enveloppes, corps, actions, brouillons, tombstones, boîte d'envoi)
    /// sont adoptées par le compte en attente — zéro perte, et la première
    /// connexion revendique le tout.
    #[test]
    fn migrates_a_full_phase2_database_and_adopts_everything() {
        let path =
            std::env::temp_dir().join(format!("wind-test-migration-p2-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid            INTEGER NOT NULL,
                    subject        TEXT,
                    sender         TEXT,
                    sender_address TEXT,
                    message_id     TEXT,
                    date_epoch     INTEGER,
                    seen           INTEGER NOT NULL DEFAULT 0,
                    flagged        INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE bodies (
                    mailbox_id INTEGER NOT NULL,
                    uid        INTEGER NOT NULL,
                    html       TEXT NOT NULL,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE pending_actions (
                    id INTEGER PRIMARY KEY, mailbox_id INTEGER NOT NULL,
                    uid INTEGER NOT NULL, kind TEXT NOT NULL
                );
                CREATE TABLE drafts (
                    id INTEGER PRIMARY KEY, to_raw TEXT NOT NULL,
                    subject TEXT NOT NULL, body TEXT NOT NULL,
                    reply_to_uid INTEGER, updated_epoch INTEGER NOT NULL,
                    remote_uid INTEGER, pushed_epoch INTEGER
                );
                CREATE TABLE draft_tombstones (remote_uid INTEGER PRIMARY KEY);
                CREATE TABLE drafts_remote (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    uid_validity INTEGER NOT NULL
                );
                CREATE TABLE outbox (
                    id INTEGER PRIMARY KEY, message_id TEXT NOT NULL,
                    sender TEXT NOT NULL, recipients TEXT NOT NULL,
                    subject TEXT NOT NULL, body_text TEXT NOT NULL,
                    in_reply_to TEXT, state TEXT NOT NULL DEFAULT 'queued',
                    attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
                    queued_epoch INTEGER NOT NULL
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 7);
                INSERT INTO envelopes (mailbox_id, uid, subject, seen, flagged)
                    VALUES (1, 42, 'hérité', 1, 1);
                INSERT INTO bodies VALUES (1, 42, '<p>corps</p>');
                INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (1, 42, 'mark_seen');
                INSERT INTO drafts (to_raw, subject, body, updated_epoch, remote_uid, pushed_epoch)
                    VALUES ('x@y.fr', 'précieux', 'texte', 10, 77, 10);
                INSERT INTO draft_tombstones VALUES (99);
                INSERT INTO drafts_remote VALUES (1, 1234);
                INSERT INTO outbox (message_id, sender, recipients, subject, body_text, queued_epoch)
                    VALUES ('<m@x>', 'moi@y.fr', 'toi@y.fr', 's', 'b', 20);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("legacy@exemple.fr", "gmail")
            .unwrap();
        assert_eq!(account, 1, "la revendication prend le compte en attente");

        assert_eq!(store.recent(account, "INBOX", 0, 10).unwrap()[0].uid, 42);
        assert_eq!(
            store.body(1, "INBOX", 42).unwrap().as_deref(),
            Some("<p>corps</p>")
        );
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts[0].account_id, 1);
        assert_eq!(drafts[0].remote_uid, Some(77));
        assert_eq!(store.draft_tombstones(1).unwrap(), vec![99]);
        assert!(
            !store.align_drafts_uidvalidity(1, 1234).unwrap(),
            "l'UIDVALIDITY des brouillons a survécu : pas de réinitialisation"
        );
        assert_eq!(store.outbox_to_send(1).unwrap().len(), 1);
        assert_eq!(store.accounts().unwrap().len(), 1);

        let second = store
            .adopt_or_create_account("deux@exemple.fr", "gmail")
            .unwrap();
        assert_ne!(second, 1, "le placeholder ne se revendique qu'une fois");

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Le cœur produit du multi-comptes : la même boîte de tous les
    /// comptes, fusionnée par date — chaque ligne connaît son compte.
    #[test]
    fn unified_recent_merges_accounts_by_date() {
        let store = Store::open_in_memory().unwrap();
        let first = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let second = store
            .adopt_or_create_account("b@exemple.fr", "gmail")
            .unwrap();
        let inbox_a = store.create_mailbox(first, "INBOX", 1).unwrap();
        let inbox_b = store.create_mailbox(second, "INBOX", 1).unwrap();

        let mut store = store;
        store
            .upsert_envelopes(
                inbox_a,
                &[
                    envelope(1, "a-ancien", 100, false),
                    envelope(2, "a-récent", 300, false),
                ],
            )
            .unwrap();
        store
            .upsert_envelopes(
                inbox_b,
                &[
                    envelope(1, "b-milieu", 200, false),
                    envelope(2, "b-dernier", 400, false),
                ],
            )
            .unwrap();

        let rows = store.unified_recent(0, 10).unwrap();
        let order: Vec<(&str, &str)> = rows
            .iter()
            .map(|row| {
                (
                    row.account_email.as_str(),
                    row.envelope.subject.as_deref().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            order,
            vec![
                ("b@exemple.fr", "b-dernier"),
                ("a@exemple.fr", "a-récent"),
                ("b@exemple.fr", "b-milieu"),
                ("a@exemple.fr", "a-ancien"),
            ],
            "fusion par date, chaque ligne porte son compte"
        );
        assert_eq!(store.unified_count().unwrap(), 4);
        // Même UID dans deux comptes : deux messages distincts.
        assert!(store.envelope(first, "INBOX", 1).unwrap().is_some());
        assert!(store.envelope(second, "INBOX", 1).unwrap().is_some());
    }

    #[test]
    fn remove_absent_drops_orphaned_bodies() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        assert_eq!(store.remove_absent(id, &HashSet::new()).unwrap(), 1);
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    /// Réparation `corps-fffd` : un corps mutilé au décodage (U+FFFD) est
    /// purgé pour que le rattrapage le retélécharge avec le décodeur
    /// corrigé ; un corps sain reste en place.
    #[test]
    fn reparation_corps_fffd_purge_les_corps_mutiles() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[envelope(1, "a", 100, false), envelope(2, "b", 100, false)],
            )
            .unwrap();
        store
            .save_body(id, 1, "<p>journ\u{FFFD}es</p>", &[])
            .unwrap();
        store.save_body(id, 2, "<p>sain</p>", &[]).unwrap();
        // Simule une base d'avant la réparation : le marqueur disparaît,
        // et la migration se rejoue comme à la prochaine ouverture.
        store
            .conn()
            .execute("DELETE FROM reparations WHERE nom = 'corps-fffd'", [])
            .unwrap();
        migrate(store.conn()).unwrap();
        let account = test_account(&store);
        assert_eq!(
            store.body(account, "INBOX", 1).unwrap(),
            None,
            "corps mutilé purgé"
        );
        assert!(
            store.body(account, "INBOX", 2).unwrap().is_some(),
            "corps sain conservé"
        );
        // Le message purgé redevient une cible du rattrapage.
        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    /// Régression (bug #2) : ré-ajouter un compte générique déjà connu
    /// doit renvoyer le MÊME id et appliquer la nouvelle configuration.
    /// Sur le chemin UPDATE de l'upsert, `last_insert_rowid()` renvoyait
    /// 0 — un id fantôme que l'UI récupérait pour la pastille et la
    /// sélection. Chaque commande ouvre SA connexion : on modélise donc
    /// le ré-ajout par deux `Store` distincts sur la même base fichier,
    /// car c'est la connexion fraîche (sans INSERT préalable) qui emprunte
    /// le chemin UPDATE et exhibe le 0.
    #[test]
    fn re_adding_a_generic_account_returns_the_same_id_and_updates_config() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-generic-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let first = {
            let store = Store::open(&path).unwrap();
            store
                .create_generic_account(
                    "compte@exemple.fr",
                    "compte",
                    "imap.a.fr",
                    993,
                    "smtp.a.fr",
                    465,
                )
                .unwrap()
        };
        let second = {
            let store = Store::open(&path).unwrap();
            store
                .create_generic_account(
                    "compte@exemple.fr",
                    "login",
                    "imap.b.fr",
                    143,
                    "smtp.b.fr",
                    587,
                )
                .unwrap()
        };
        let (count, config) = {
            let store = Store::open(&path).unwrap();
            (
                store.accounts().unwrap().len(),
                store.account_config(first).unwrap(),
            )
        };
        // Nettoyage avant les assertions : un échec ne doit pas laisser de
        // fichier temporaire derrière lui.
        let _ = std::fs::remove_file(&path);

        assert!(first > 0, "la primo-création doit renvoyer un id réel");
        assert_eq!(
            second, first,
            "le ré-ajout doit renvoyer l'id existant, jamais 0"
        );
        assert_eq!(count, 1, "un seul compte, pas un doublon");
        assert_eq!(config.username.as_deref(), Some("login"));
        assert_eq!(config.imap_host.as_deref(), Some("imap.b.fr"));
        assert_eq!(config.imap_port, Some(143));
        assert_eq!(config.smtp_host.as_deref(), Some("smtp.b.fr"));
        assert_eq!(config.smtp_port, Some(587));
    }

    /// Le rattrapage vise les messages RÉCENTS sans corps, du plus récent
    /// au plus ancien : c'est l'ordre où la recherche a le plus de valeur,
    /// et celui qui rend la reprise après coupure naturelle.
    #[test]
    fn backfill_lists_recent_bodyless_messages_newest_first() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "ancien", 1_000, false),
                    envelope(2, "milieu", 2_000, false),
                    envelope(3, "récent", 3_000, false),
                ],
            )
            .unwrap();
        let account = test_account(&store);

        let todo = store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap();
        assert_eq!(todo, vec![3, 2, 1]);
    }

    #[test]
    fn backfill_skips_messages_that_already_have_a_body() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "sans corps", 1_000, false),
                    envelope(2, "avec corps", 2_000, false),
                ],
            )
            .unwrap();
        store.save_body(id, 2, "<p>déjà là</p>", &[]).unwrap();
        let account = test_account(&store);

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    /// L'horizon de récence est ce qui BORNE le coût (ADR 0007) : au-delà,
    /// on ne rapatrie rien.
    #[test]
    fn backfill_respects_the_recency_horizon() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "hors horizon", 1_000, false),
                    envelope(2, "dans l'horizon", 5_000, false),
                ],
            )
            .unwrap();
        let account = test_account(&store);

        assert_eq!(
            store
                .bodies_to_backfill(account, "INBOX", 4_000, 10)
                .unwrap(),
            vec![2]
        );
    }

    #[test]
    fn backfill_honours_the_batch_limit() {
        let (mut store, id) = store_with_mailbox();
        let envelopes: Vec<Envelope> = (1..=10)
            .map(|uid| envelope(uid, "message", uid as i64 * 100, false))
            .collect();
        store.upsert_envelopes(id, &envelopes).unwrap();
        let account = test_account(&store);

        assert_eq!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 3)
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn backfill_never_leaks_another_accounts_messages() {
        let (mut store, mine) = store_with_mailbox();
        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        let theirs = store.create_mailbox(other, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(mine, &[envelope(1, "à moi", 1_000, false)])
            .unwrap();
        store
            .upsert_envelopes(theirs, &[envelope(1, "à l'autre", 2_000, false)])
            .unwrap();
        let account = test_account(&store);

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1],
            "un seul message : celui du compte demandé"
        );
        assert_eq!(
            store.bodies_to_backfill(other, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    // -----------------------------------------------------------------
    // Regroupement en conversations
    // -----------------------------------------------------------------

    /// Une réponse à `parent`, dans le format de [`envelope`] — dont le
    /// `Message-ID` est `<m{uid}@example.com>`.
    fn reply(uid: Uid, subject: &str, epoch: i64, seen: bool, parent: Uid) -> Envelope {
        Envelope {
            in_reply_to: Some(format!("<m{parent}@example.com>")),
            ..envelope(uid, subject, epoch, seen)
        }
    }

    fn unified(store: &Store) -> Vec<UnifiedRow> {
        store.unified_recent(0, 50).unwrap()
    }

    fn uids(rows: &[UnifiedRow]) -> Vec<Uid> {
        rows.iter().map(|row| row.envelope.uid).collect()
    }

    /// Le cœur du chantier : deux messages, une seule ligne.
    #[test]
    fn la_liste_montre_une_ligne_par_conversation() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    reply(2, "Re: Devis", 200, true, 1),
                ],
            )
            .unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "un fil, et non deux messages");
        assert_eq!(rows[0].thread_size, 2);
        assert_eq!(
            rows[0].envelope.uid, 2,
            "la ligne montre le DERNIER message"
        );
        assert_eq!(
            store.unified_count().unwrap(),
            1,
            "le défilement compte des conversations, sinon il défile dans le vide"
        );
    }

    #[test]
    fn une_reponse_fait_remonter_tout_le_fil() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    envelope(2, "Facture", 200, true),
                ],
            )
            .unwrap();
        assert_eq!(uids(&unified(&store)), vec![2, 1]);

        store
            .upsert_envelopes(id, &[reply(3, "Re: Devis", 300, true, 1)])
            .unwrap();

        let rows = unified(&store);
        assert_eq!(
            uids(&rows),
            vec![3, 2],
            "le devis repasse devant la facture"
        );
        assert_eq!(rows[0].thread_size, 2);
    }

    /// Un fil dont le dernier message est lu, mais qui garde un non-lu
    /// plus haut, doit rester en gras. Lire l'état du seul message affiché
    /// donnerait la réponse inverse.
    #[test]
    fn un_fil_reste_non_lu_tant_qu_un_de_ses_messages_l_est() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, false),
                    reply(2, "Re: Devis", 200, true, 1),
                ],
            )
            .unwrap();

        let rows = unified(&store);
        assert!(rows[0].envelope.seen, "le dernier message est lu…");
        assert_eq!(rows[0].thread_unseen, 1, "…mais le fil garde un non-lu");

        store.set_seen_local(id, 1, true).unwrap();
        assert_eq!(
            unified(&store)[0].thread_unseen,
            0,
            "lire le message manquant éteint le fil"
        );
    }

    /// Le cas qui justifie la passe sur les en-têtes complets : dans une
    /// boîte de réception, le message du milieu d'un échange est celui
    /// qu'on a soi-même ENVOYÉ — il n'y est pas. `In-Reply-To` seul laisse
    /// donc deux fils ; `References`, qui porte aussi la racine, les
    /// recolle.
    #[test]
    fn les_references_recollent_deux_moities_de_fil() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    // Répond à <m2@…> : notre propre réponse, absente.
                    reply(3, "Re: Devis", 300, true, 2),
                ],
            )
            .unwrap();
        assert_eq!(unified(&store).len(), 2, "deux fils, faute du chaînon");

        assert!(
            store
                .set_thread_headers(id, 3, None, "<m1@example.com> <m2@example.com>")
                .unwrap(),
            "le rattachement a changé"
        );

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "les deux moitiés se rejoignent");
        assert_eq!(rows[0].thread_size, 2);
        assert_eq!(rows[0].envelope.uid, 3);
    }

    /// Une re-synchronisation réécrit l'enveloppe. Si elle écrasait les
    /// `References` déjà acquises, elle DÉGROUPERAIT un fil recollé : le
    /// regroupement se déferait tout seul, sans que rien ne le signale.
    /// C'est le piège qui avait coûté les pièces jointes.
    #[test]
    fn une_resynchronisation_ne_degroupe_pas_un_fil_recolle() {
        let (mut store, id) = store_with_mailbox();
        let arrivee = [
            envelope(1, "Devis", 100, true),
            reply(3, "Re: Devis", 300, true, 2),
        ];
        store.upsert_envelopes(id, &arrivee).unwrap();
        store
            .set_thread_headers(id, 3, None, "<m1@example.com> <m2@example.com>")
            .unwrap();
        assert_eq!(unified(&store).len(), 1);

        store.upsert_envelopes(id, &arrivee).unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "le fil tient la re-synchronisation");
        assert_eq!(rows[0].thread_size, 2);
    }

    /// Le piège des pièces jointes appliqué aux fils : une base d'avant le
    /// regroupement a `thread_id` NULL partout. La liste part de
    /// `threads` — sans adoption, elle serait VIDE à la première
    /// ouverture, et pour toujours.
    #[test]
    fn une_base_heritee_voit_tous_ses_messages_adoptes() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    envelope(2, "Facture", 200, true),
                ],
            )
            .unwrap();

        // On rembobine à l'état d'une base d'avant les fils.
        store
            .conn()
            .execute_batch(
                "UPDATE envelopes SET thread_id = NULL;
                 DELETE FROM thread_links;
                 DELETE FROM threads;",
            )
            .unwrap();
        assert!(
            unified(&store).is_empty(),
            "sans adoption, la boîte entière disparaît de l'écran"
        );

        crate::thread::migrate_threads(store.conn()).unwrap();

        assert_eq!(uids(&unified(&store)), vec![2, 1]);
        assert_eq!(store.unified_count().unwrap(), 2);
    }

    /// Le désordre d'arrivée ne doit rien changer : ici la réponse précède
    /// son parent dans le même lot.
    #[test]
    fn un_fil_se_lit_du_plus_ancien_au_plus_recent() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    reply(2, "Re: Devis", 200, true, 1),
                    envelope(1, "Devis", 100, true),
                ],
            )
            .unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "l'ordre d'arrivée ne casse pas le fil");
        let thread = rows[0].thread_id.unwrap();
        let messages = store.thread_messages(thread).unwrap();
        assert_eq!(uids(&messages), vec![1, 2]);
        // Chaque message repart en connaissant la taille de SON fil :
        // sinon l'écran qui le rouvre conclurait qu'il est seul.
        assert!(messages.iter().all(|m| m.thread_size == 2));
    }

    #[test]
    fn retirer_les_messages_d_un_fil_le_fait_disparaitre() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    reply(2, "Re: Devis", 200, true, 1),
                ],
            )
            .unwrap();

        store.remove_local(id, 2).unwrap();
        let rows = unified(&store);
        assert_eq!(uids(&rows), vec![1], "le fil retombe sur ce qui reste");
        assert_eq!(rows[0].thread_size, 1);

        store.remove_local(id, 1).unwrap();
        assert!(unified(&store).is_empty());
        assert_eq!(store.unified_count().unwrap(), 0);
    }

    /// Le défaut du terrain, de bout en bout : deux messages étrangers
    /// dont l'`In-Reply-To` est une PHRASE — pas un identifiant — doivent
    /// rester deux conversations.
    ///
    /// Avant correction, chaque mot de la phrase devenait une ancre
    /// commune et les réunissait. Sur une vraie boîte, cela donnait un
    /// fil de 43 messages sans rapport les uns avec les autres.
    #[test]
    fn deux_messages_dont_l_en_tete_est_en_prose_ne_fusionnent_pas() {
        let (mut store, id) = store_with_mailbox();
        let prose = "Votre message du 3 janvier";
        store
            .upsert_envelopes(
                id,
                &[
                    Envelope {
                        in_reply_to: Some(prose.to_string()),
                        ..envelope(1, "Promotion", 100, true)
                    },
                    Envelope {
                        in_reply_to: Some(prose.to_string()),
                        ..envelope(2, "Autre promotion", 200, true)
                    },
                ],
            )
            .unwrap();

        assert_eq!(unified(&store).len(), 2, "aucun lien entre ces deux-là");
    }

    /// Une base regroupée par l'ancienne règle porte des fils FAUX, et
    /// corriger le code ne les répare pas tout seul. Le marqueur de
    /// version les fait refaire à l'ouverture — sans réseau, les en-têtes
    /// bruts étant intacts en base.
    #[test]
    fn une_base_mal_regroupee_est_refaite_a_l_ouverture() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Promotion", 100, true),
                    envelope(2, "Autre promotion", 200, true),
                ],
            )
            .unwrap();
        assert_eq!(unified(&store).len(), 2);

        // On rejoue l'état que produisait la règle permissive : un seul
        // fil pour deux messages étrangers, et la version d'avant.
        store
            .conn()
            .execute_batch(
                "DELETE FROM thread_links WHERE thread_id = (SELECT MAX(id) FROM threads);
                 UPDATE envelopes SET thread_id = (SELECT MIN(id) FROM threads);
                 DELETE FROM threads WHERE id = (SELECT MAX(id) FROM threads);
                 UPDATE threads SET size = 2, last_uid = 2, last_epoch = 200;
                 PRAGMA user_version = 0;",
            )
            .unwrap();
        assert_eq!(unified(&store).len(), 1, "l'état fautif est bien reproduit");

        crate::thread::migrate_threads(store.conn()).unwrap();

        assert_eq!(unified(&store).len(), 2, "les fils sont refaits");
        let version: i64 = store
            .conn()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        // Contre la CONSTANTE, jamais contre un littéral : chaque
        // changement de règle de regroupement l'incrémente, et un « 1 »
        // en dur ferait échouer ce test pour une raison qui n'est pas la
        // sienne.
        assert_eq!(
            version,
            crate::thread::THREADING_VERSION,
            "et la reconstruction ne se rejoue pas"
        );
    }

    /// UIDVALIDITY invalidée : les fils partent avec le reste, et
    /// l'annuaire ne doit pas empêcher une repopulation propre.
    #[test]
    fn reset_mailbox_efface_aussi_les_fils() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Devis", 100, true),
                    reply(2, "Re: Devis", 200, true, 1),
                ],
            )
            .unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert!(unified(&store).is_empty());

        store
            .upsert_envelopes(id, &[envelope(1, "Devis", 100, true)])
            .unwrap();
        assert_eq!(unified(&store).len(), 1, "la boîte se repeuple sans butée");
    }

    /// Rejoue sur `path` les tables telles que la version 1 des fils les
    /// créait — le seul décor où la passe d'adoption a du travail réel.
    /// Partagé par le test d'ouverture ci-dessous et par ceux du
    /// rembobinage (chantier Phase 5).
    fn rembobine_au_schema_v1(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "DROP TABLE thread_links;
             DROP TABLE threads;
             CREATE TABLE threads (
                 id         INTEGER PRIMARY KEY,
                 mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                 last_uid   INTEGER NOT NULL DEFAULT 0,
                 last_epoch INTEGER,
                 size       INTEGER NOT NULL DEFAULT 0,
                 unseen     INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX idx_threads_date
                 ON threads(mailbox_id, last_epoch DESC, last_uid DESC);
             CREATE TABLE thread_links (
                 mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                 message_id TEXT NOT NULL,
                 thread_id  INTEGER NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
                 PRIMARY KEY (mailbox_id, message_id)
             );
             CREATE INDEX idx_thread_links_thread ON thread_links(thread_id);
             UPDATE envelopes SET thread_id = NULL;
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }

    /// Défaut trouvé au TERRAIN, pas ici : une base créée par la version
    /// précédente porte une table `threads` sans `inbox_size`.
    /// `CREATE TABLE IF NOT EXISTS` ne la touche pas — mais l'index
    /// partiel, lui, n'existe pas encore, donc SQLite le crée vraiment :
    /// il échoue sur une colonne absente, et **l'ouverture entière est
    /// refusée** (« no such column: inbox_size »). L'application ne
    /// démarrait plus.
    ///
    /// Aucun test ne pouvait l'attraper : ils créent tous une base neuve,
    /// donc déjà au schéma courant. Celui-ci REMBOBINE une vraie base au
    /// schéma d'avant — le seul décor où le défaut existe.
    #[test]
    fn une_base_au_schema_des_fils_precedent_s_ouvre_et_se_migre() {
        let path =
            std::env::temp_dir().join(format!("wind-test-fils-v1-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            let mut premier = envelope(1, "Devis", 100, true);
            premier.message_id = Some("<a@exemple.fr>".to_string());
            let mut second = envelope(2, "Re: Devis", 200, true);
            second.message_id = Some("<b@exemple.fr>".to_string());
            second.in_reply_to = Some("<a@exemple.fr>".to_string());
            store.upsert_envelopes(inbox, &[premier, second]).unwrap();
            assert_eq!(unified(&store).len(), 1, "décor : un fil de deux messages");
        }

        // Rembobinage : les tables telles que la version 1 les créait.
        rembobine_au_schema_v1(&path);

        // C'est CETTE ouverture qui refusait de se faire.
        let store = Store::open(&path).unwrap();
        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "le fil est refait, et la liste le montre");
        assert_eq!(lignes[0].thread_size, 2, "avec son compteur");

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// LE test du chantier Phase 5 (passation §8) : l'adoption n'est PAS
    /// fractionnable — la liste part de `threads`, une adoption partielle
    /// persistée serait une boîte à moitié vide. « Interruptible » veut
    /// donc dire : annuler AU MILIEU de la passe défait TOUT et laisse
    /// `user_version` inchangé, pour que la passe entière se rejoue au
    /// prochain lancement — où la liste est complète.
    #[test]
    fn annuler_l_adoption_defait_tout_et_laisse_user_version_inchangee() {
        let path =
            std::env::temp_dir().join(format!("wind-test-rembobinage-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        // Assez de messages pour que l'annulation tombe en PLEINE passe :
        // l'avancement se rapporte par paliers, il faut en franchir un.
        const MESSAGES: u32 = 1_200;
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            let decor: Vec<Envelope> = (1..=MESSAGES)
                .map(|uid| envelope(uid, "Sujet", 100 + i64::from(uid), true))
                .collect();
            store.upsert_envelopes(inbox, &decor).unwrap();
        }
        rembobine_au_schema_v1(&path);

        // Annuler dès que 1 000 messages sont passés — au milieu, pas au
        // seuil de la porte : le rembobinage doit défaire du travail réel.
        let mut plus_haut_fait = 0;
        let result = Store::open_with_progress(&path, |p| {
            plus_haut_fait = plus_haut_fait.max(p.done);
            if p.done >= 1_000 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        assert!(
            matches!(result, Err(Error::Interrupted)),
            "annuler doit rendre Error::Interrupted, pas un Store"
        );
        assert!(
            plus_haut_fait >= 1_000,
            "le décor doit exercer une annulation EN COURS de passe \
             (relevé le plus haut : {plus_haut_fait})"
        );

        // Tout est défait : la base est revenue à l'état d'AVANT
        // l'ouverture annulée.
        {
            let conn = Connection::open(&path).unwrap();
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1, "user_version inchangé : la passe se rejouera");
            let forme_neuve: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('threads')
                     WHERE name = 'inbox_size'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                forme_neuve, 0,
                "la table v1 est intacte : le DROP aussi est rembobiné"
            );
            let enveloppes: i64 = conn
                .query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))
                .unwrap();
            assert_eq!(enveloppes, i64::from(MESSAGES), "aucun message perdu");
        }

        // Le prochain lancement rejoue la passe ENTIÈRE : liste complète.
        {
            let store = Store::open(&path).unwrap();
            let sans_fil: i64 = store
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM envelopes WHERE thread_id IS NULL",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(sans_fil, 0, "tous les messages hérités sont adoptés");
            let version: i64 = store
                .conn()
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, crate::thread::THREADING_VERSION);
        }
        let _ = std::fs::remove_file(&path);
    }

    /// L'avancement est OBSERVABLE (enseignement §9) : le total s'annonce
    /// d'emblée et ne bouge plus, l'avancement ne recule jamais, et
    /// « fini » ne se dit qu'à la fin — jamais avant.
    #[test]
    fn l_adoption_annonce_son_avancement_du_depart_a_la_fin() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-avancement-adoption-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Devis", 100, true),
                        reply(2, "Re: Devis", 200, true, 1),
                    ],
                )
                .unwrap();
        }
        rembobine_au_schema_v1(&path);

        let mut releves: Vec<AdoptionProgress> = Vec::new();
        let store = Store::open_with_progress(&path, |p| {
            releves.push(p);
            ControlFlow::Continue(())
        })
        .unwrap();

        assert!(!releves.is_empty(), "une adoption muette n'est pas visible");
        assert_eq!(releves[0].done, 0, "le départ se dit tout de suite");
        assert!(releves[0].total > 0, "le total est annoncé d'emblée");
        for paire in releves.windows(2) {
            assert!(paire[1].done >= paire[0].done, "l'avancement ne recule pas");
            assert_eq!(
                paire[1].total, paire[0].total,
                "le total ne bouge pas en route — une barre qui recule \
                 est pire qu'une barre imprécise"
            );
        }
        let dernier = releves.last().unwrap();
        assert_eq!(
            dernier.done, dernier.total,
            "le dernier relevé dit « fini »"
        );
        assert!(
            releves[..releves.len() - 1]
                .iter()
                .all(|p| p.done < p.total),
            "et il est le SEUL : jamais « 100 % » avant la fin"
        );

        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "le fil est refait");
        assert_eq!(lignes[0].thread_size, 2, "avec son compteur");
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// La sonde répond sans rien déclencher : le desktop l'appelle AVANT
    /// la première vraie ouverture pour décider d'afficher l'écran de
    /// migration — si elle migrait elle-même, l'écran arriverait après
    /// la bataille.
    #[test]
    fn la_sonde_dit_quand_une_adoption_attend_sans_la_declencher() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-sonde-adoption-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        // Fichier absent : première installation, rien d'hérité — et la
        // sonde ne doit PAS créer le fichier.
        assert_eq!(Store::pending_adoption(&path).unwrap(), None);
        assert!(!path.exists(), "une sonde ne laisse pas de trace");

        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Devis", 100, true),
                        reply(2, "Re: Devis", 200, true, 1),
                    ],
                )
                .unwrap();
            // Un message HORS portée (ADR 0010 §3) : la passe ne
            // l'adoptera jamais, la sonde ne doit pas l'annoncer.
            let spam = store.create_mailbox(account, "Spam", 1).unwrap();
            store
                .upsert_envelopes(spam, &[envelope(1, "Gagné !", 300, true)])
                .unwrap();
        }
        // Base à jour : rien à annoncer.
        assert_eq!(Store::pending_adoption(&path).unwrap(), None);

        rembobine_au_schema_v1(&path);
        assert_eq!(
            Store::pending_adoption(&path).unwrap(),
            Some(2),
            "une base héritée annonce ses messages à adopter — la PORTÉE, \
             pas la base entière : un chiffre doit désigner ce qu'il dit"
        );
        // Et RIEN n'a été déclenché : la version n'a pas bougé.
        {
            let conn = Connection::open(&path).unwrap();
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1, "la sonde n'a pas migré à notre place");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// Sur une base à jour il n'y a RIEN à adopter — et donc rien à dire.
    /// Un bandeau de migration à chaque lancement serait un faux signal,
    /// et chaque commande du desktop ouvre sa propre connexion.
    #[test]
    fn une_base_a_jour_s_ouvre_sans_annoncer_de_migration() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-adoption-muette-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Devis", 100, true),
                        reply(2, "Re: Devis", 200, true, 1),
                    ],
                )
                .unwrap();
        }

        let mut appels = 0;
        let store = Store::open_with_progress(&path, |_| {
            appels += 1;
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(appels, 0, "rien à adopter, rien à raconter");
        assert_eq!(unified(&store).len(), 1, "et la liste est là");
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// LE point du chantier de l'[ADR 0009] : un message reçu et la
    /// réponse qu'on lui a faite appartiennent au même échange, donc au
    /// même fil — bien qu'ils vivent dans **deux boîtes différentes**.
    ///
    /// Avant, les fils étaient cloisonnés par boîte : cette réponse aurait
    /// formé son propre fil dans son propre espace d'identifiants, et
    /// synchroniser « Envoyés » aurait coûté sans rien rapporter.
    ///
    /// Le décor donne le même UID (1) aux deux messages, à dessein :
    /// l'identité d'un message est `(compte, boîte, UID)`, et un
    /// regroupement qui confondrait deux UID égaux se verrait ici.
    #[test]
    fn une_reponse_dans_envoyes_rejoint_le_fil_du_message_recu() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let envoyes = store.create_mailbox(account, "Sent", 1).unwrap();
        // Le décor doit DÉCLARER la portée qu'il exerce : depuis l'ADR 0010
        // une boîte ne regroupe que si on l'a dite dedans, et le nom du
        // dossier des envois varie d'un serveur à l'autre.
        store.set_thread_scope(account, Some("Sent")).unwrap();

        // Alice écrit.
        let mut recu = envelope(1, "Devis", 100, true);
        recu.message_id = Some("<alice-1@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[recu]).unwrap();

        // Je réponds : le message part dans « Envoyés » et cite le premier.
        let mut reponse = envelope(1, "Re: Devis", 200, true);
        reponse.message_id = Some("<moi-1@exemple.fr>".to_string());
        reponse.in_reply_to = Some("<alice-1@exemple.fr>".to_string());
        store.upsert_envelopes(envoyes, &[reponse]).unwrap();

        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "un seul fil, pas deux");
        assert_eq!(
            lignes[0].thread_size, 2,
            "le compteur couvre tout l'échange, envoyés compris"
        );
        assert_eq!(
            lignes[0].envelope.subject.as_deref(),
            Some("Re: Devis"),
            "le fil est représenté par son message le plus récent, \
             même quand c'est notre propre réponse"
        );
    }

    /// Deux messages du MÊME compte peuvent porter le MÊME UID dès qu'ils
    /// vivent dans deux boîtes — c'est la règle et non l'exception, les
    /// UID étant attribués par boîte et repartant de 1.
    ///
    /// Chaque ligne doit donc dire **où elle habite**. Sans cela, ouvrir
    /// notre réponse depuis le bandeau de conversation afficherait le
    /// message reçu à sa place, et le marquerait lu — l'invariant §6.2 de
    /// la passation, corrigé ici pour deux boîtes.
    #[test]
    fn chaque_ligne_dit_dans_quelle_boite_elle_habite() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let envoyes = store.create_mailbox(account, "Sent", 1).unwrap();
        // Le décor doit DÉCLARER la portée qu'il exerce : depuis l'ADR 0010
        // une boîte ne regroupe que si on l'a dite dedans, et le nom du
        // dossier des envois varie d'un serveur à l'autre.
        store.set_thread_scope(account, Some("Sent")).unwrap();

        let mut recu = envelope(1, "Devis", 100, true);
        recu.message_id = Some("<alice-9@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[recu]).unwrap();
        let mut reponse = envelope(1, "Re: Devis", 200, true);
        reponse.message_id = Some("<moi-9@exemple.fr>".to_string());
        reponse.in_reply_to = Some("<alice-9@exemple.fr>".to_string());
        store.upsert_envelopes(envoyes, &[reponse]).unwrap();

        let fil = unified(&store)[0].thread_id.unwrap();
        let messages = store.thread_messages(fil).unwrap();

        assert_eq!(messages.len(), 2);
        assert!(
            messages.iter().all(|ligne| ligne.envelope.uid == 1),
            "le décor a bien deux messages de même UID : c'est tout l'objet"
        );
        let boites: Vec<&str> = messages.iter().map(|l| l.mailbox.as_str()).collect();
        assert!(boites.contains(&"INBOX"), "boîtes vues : {boites:?}");
        assert!(boites.contains(&"Sent"), "boîtes vues : {boites:?}");
    }

    /// L'autre face de la même règle : écrire à quelqu'un qui ne répond
    /// jamais ne crée PAS de conversation dans la boîte de réception.
    /// C'est ce que le compteur `inbox_size` protège, et c'est aussi ce
    /// qui rend l'index partiel possible (ADR 0009 §2 et §4).
    #[test]
    fn un_fil_purement_sortant_n_a_pas_de_ligne() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(account, "INBOX", 1).unwrap();
        let envoyes = store.create_mailbox(account, "Sent", 1).unwrap();
        // Le décor doit DÉCLARER la portée qu'il exerce : depuis l'ADR 0010
        // une boîte ne regroupe que si on l'a dite dedans, et le nom du
        // dossier des envois varie d'un serveur à l'autre.
        store.set_thread_scope(account, Some("Sent")).unwrap();

        let mut sortant = envelope(1, "Ma proposition", 100, true);
        sortant.message_id = Some("<moi-2@exemple.fr>".to_string());
        store.upsert_envelopes(envoyes, &[sortant]).unwrap();

        assert!(
            unified(&store).is_empty(),
            "rien n'a été reçu : la boîte de réception reste vide"
        );
        assert_eq!(store.unified_count().unwrap(), 0);
    }

    /// [ADR 0010] §3 — on STOCKE tout, on ne REGROUPE que dans la portée.
    ///
    /// Depuis l'[ADR 0009] un fil appartient au COMPTE. Dès que la
    /// synchronisation intégrale verse Archive, Corbeille et Spam dans ce
    /// même compte, leurs messages rejoindraient les fils **tout seuls** —
    /// et trois agrégats se corrompraient sans qu'aucun test ne le voie :
    ///
    /// - `size` : « 12 messages » sur un fil qui en montre 3 ;
    /// - `unseen` : un fil éternellement non lu à cause d'un spam ;
    /// - `last_epoch` : **la conversation remonte en tête de liste parce
    ///   qu'un spam s'y est accroché**.
    ///
    /// Le troisième est un défaut de CORRECTION : la liste mentirait sur
    /// l'ordre des échanges, sans recours pour l'utilisateur. Même motif de
    /// refus que le regroupement par sujet (ADR 0008 §2).
    ///
    /// Le compilateur ne protège rien ici — une boîte est une chaîne comme
    /// une autre (passation §6.2). C'est ce test qui tient l'invariant.
    #[test]
    fn un_message_hors_portee_ne_rejoint_pas_le_fil() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let spam = store.create_mailbox(account, "Spam", 1).unwrap();

        let mut recu = envelope(1, "Devis", 100, true);
        recu.message_id = Some("<alice-10@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[recu]).unwrap();

        // Le spam cite le message reçu — c'est exactement ce qui le ferait
        // rejoindre le fil. Il est PLUS RÉCENT et NON LU : s'il entrait,
        // les trois agrégats bougeraient d'un coup.
        let mut indesirable = envelope(1, "GAGNEZ 1000 EUROS", 300, false);
        indesirable.message_id = Some("<spam-1@ailleurs.example>".to_string());
        indesirable.in_reply_to = Some("<alice-10@exemple.fr>".to_string());
        store.upsert_envelopes(spam, &[indesirable]).unwrap();

        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "un seul fil");
        assert_eq!(
            lignes[0].thread_size, 1,
            "le spam ne compte pas dans l'échange"
        );
        assert_eq!(
            lignes[0].envelope.subject.as_deref(),
            Some("Devis"),
            "le fil reste représenté par le message reçu, pas par le spam \
             qui s'y est accroché"
        );
        assert_eq!(
            lignes[0].thread_unseen, 0,
            "un spam jamais ouvert ne rend pas la conversation non lue"
        );

        // L'autre moitié de l'ADR 0010 : hors portée ne veut pas dire
        // absent. Le message est stocké — donc cherchable.
        assert!(
            store.envelope(account, "Spam", 1).unwrap().is_some(),
            "le spam est bien en base : on stocke tout, on ne regroupe pas tout"
        );
    }

    /// La portée déclarée AVANT que la boîte n'existe doit valoir quand
    /// même — c'est le cas normal, pas le cas limite.
    ///
    /// La boucle de synchronisation de l'[ADR 0010] **crée** le dossier des
    /// envois : au moment où l'on déclare la portée, il n'y a aucune ligne
    /// à mettre à jour. Si la portée ne vivait que sur `mailboxes`, cette
    /// déclaration serait perdue, la boîte naîtrait hors portée, et ses
    /// messages resteraient sans fil jusqu'au prochain démarrage — la liste
    /// afficherait un échange amputé de nos réponses, sans rien signaler.
    ///
    /// D'où la mémoire portée par le COMPTE, que ce test garde.
    #[test]
    fn une_portee_declaree_avant_la_creation_de_la_boite_vaut_quand_meme() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();

        // On déclare « Envoyés » AVANT de l'avoir créé — l'ordre réel.
        store.set_thread_scope(account, Some("Sent")).unwrap();
        let envoyes = store.create_mailbox(account, "Sent", 1).unwrap();

        let mut recu = envelope(1, "Devis", 100, true);
        recu.message_id = Some("<alice-11@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[recu]).unwrap();
        let mut reponse = envelope(1, "Re: Devis", 200, true);
        reponse.message_id = Some("<moi-11@exemple.fr>".to_string());
        reponse.in_reply_to = Some("<alice-11@exemple.fr>".to_string());
        store.upsert_envelopes(envoyes, &[reponse]).unwrap();

        let lignes = unified(&store);
        assert_eq!(lignes.len(), 1, "un seul fil");
        assert_eq!(
            lignes[0].thread_size, 2,
            "la réponse a rejoint le fil dès son écriture, sans attendre \
             un redémarrage"
        );
    }

    /// La promesse de l'[ADR 0008] §4 — « le coût d'une page ne dépend
    /// plus de la taille de la boîte » — repose ENTIÈREMENT sur un index
    /// qui porte le tri. Si SQLite matérialise l'ordre dans un B-arbre
    /// temporaire, elle est rompue : silencieusement, et seulement à
    /// l'échelle, là où plus aucun test fonctionnel ne regarde.
    ///
    /// C'est arrivé. Le gate 3 a mesuré **987 ms** pour une page à
    /// 160 000 conversations, contre 0,66 ms une fois l'index posé.
    /// L'index d'origine était préfixé par `mailbox_id` : il servait une
    /// boîte, mais pas la **boîte unifiée**, qui les couvre toutes et qui
    /// est la vue par défaut du produit. Deux comptes suffisent à le
    /// reproduire — d'où ce décor.
    ///
    /// On interroge le plan plutôt qu'un chronomètre : une durée dépend
    /// de la machine, un plan d'exécution non.
    #[test]
    fn la_boite_unifiee_ne_materialise_pas_son_tri() {
        let mut store = Store::open_in_memory().unwrap();
        for (email, uids) in [("un@exemple.fr", 1..60u32), ("deux@exemple.fr", 60..120)] {
            let account = store.adopt_or_create_account(email, "gmail").unwrap();
            let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            let envelopes: Vec<Envelope> = uids
                .map(|uid| envelope(uid, "Sujet", 1_600_000_000 + i64::from(uid), true))
                .collect();
            store.upsert_envelopes(mailbox, &envelopes).unwrap();
        }

        let mut stmt = store
            .0
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                unified_page_sql(false, false)
            ))
            .unwrap();
        let plan: Vec<String> = stmt
            .query_map(params![200i64, 0i64], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        // « FOR LAST TERM OF ORDER BY » est acceptable : ce tri-là ne
        // départage que les ex æquo de date ET d'UID. C'est le tri
        // COMPLET qui coûte, et lui seul est interdit ici.
        assert!(
            !plan
                .iter()
                .any(|etape| etape.contains("TEMP B-TREE FOR ORDER BY")),
            "la page de la boîte unifiée matérialise son tri — le coût \
             redevient proportionnel à la taille de la boîte.\nPlan :\n{}",
            plan.join("\n")
        );
    }
}
