//! Chargement à la demande du corps d'un message : cache SQLite d'abord,
//! serveur ensuite, puis mise en cache — le principe « enveloppes d'abord »
//! appliqué jusqu'au bout (le corps n'arrive qu'au clic, puis reste offline).

use crate::envelope::Uid;
use crate::error::Error;
use crate::remote::MailServer;
use crate::store::Store;

/// Corps HTML brut (pré-assainissement) d'un message. `None` si la boîte n'a
/// jamais été synchronisée ou si le message a disparu du serveur.
pub fn load_body(
    server: &mut dyn MailServer,
    store: &mut Store,
    mailbox: &str,
    uid: Uid,
) -> Result<Option<String>, Error> {
    if let Some(cached) = store.body(mailbox, uid)? {
        return Ok(Some(cached));
    }
    let Some(state) = store.sync_state(mailbox)? else {
        return Ok(None);
    };
    match server.fetch_body_html(mailbox, uid)? {
        Some(html) => {
            store.save_body(state.mailbox_id, uid, &html)?;
            Ok(Some(html))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    fn synced_setup() -> (FakeServer, Store) {
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "sujet", "<p>corps du message</p>");
        let mut store = Store::open_in_memory().unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, "INBOX")
            .unwrap();
        (server, store)
    }

    #[test]
    fn fetches_then_serves_from_cache() {
        let (mut server, mut store) = synced_setup();

        let first = load_body(&mut server, &mut store, "INBOX", 1).unwrap();
        assert_eq!(first.as_deref(), Some("<p>corps du message</p>"));
        assert_eq!(server.body_fetches, 1);

        let second = load_body(&mut server, &mut store, "INBOX", 1).unwrap();
        assert_eq!(second.as_deref(), Some("<p>corps du message</p>"));
        assert_eq!(server.body_fetches, 1, "le cache doit éviter le serveur");
    }

    #[test]
    fn returns_none_for_vanished_message() {
        let (mut server, mut store) = synced_setup();
        assert_eq!(
            load_body(&mut server, &mut store, "INBOX", 99).unwrap(),
            None
        );
    }

    #[test]
    fn returns_none_before_first_sync_without_touching_server() {
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "sujet", "<p>x</p>");
        let mut store = Store::open_in_memory().unwrap();

        assert_eq!(
            load_body(&mut server, &mut store, "INBOX", 1).unwrap(),
            None
        );
        assert_eq!(server.body_fetches, 0);
    }
}
