//! Ce qui mérite d'interrompre l'utilisateur — et ce qui ne le mérite pas.
//!
//! Une notification est une interruption. Le produit se promet *simple*
//! ([PLAN.md](../../../docs/PLAN.md) §1) : mieux vaut n'en montrer aucune
//! qu'une de trop. Les règles sont donc écrites ici, en fonctions pures,
//! plutôt que dispersées dans la couche applicative où elles seraient
//! invérifiables.

use crate::envelope::Envelope;
use crate::sync::SyncMode;

/// Au-delà, on résume au lieu d'énumérer : une liste de dix expéditeurs
/// dans une bulle système n'est plus lisible.
const MAX_SENDERS_LISTED: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
}

/// Les arrivées d'une synchro qui peuvent donner lieu à notification.
///
/// Une synchro **initiale** n'en donne aucune : elle rapatrie la boîte
/// entière. Notifier là serait annoncer comme « nouveau » un courrier
/// vieux de trois ans, des milliers de fois.
pub fn arrivals_to_notify(mode: SyncMode, new_unread: Vec<Envelope>) -> Vec<Envelope> {
    match mode {
        SyncMode::Initial => Vec::new(),
        SyncMode::Incremental => new_unread,
    }
}

/// La notification à afficher pour un lot d'arrivées, tous comptes
/// confondus — **une seule**, jamais une par message.
///
/// Trois messages qui arrivent ensemble produisent une bulle, pas trois :
/// l'empilement de bulles est le défaut qui fait couper les notifications.
pub fn notification_for(arrivals: &[Envelope]) -> Option<Notification> {
    match arrivals {
        [] => None,
        [single] => Some(Notification {
            title: sender_of(single),
            body: subject_of(single),
        }),
        many => Some(Notification {
            title: format!("{} nouveaux messages", many.len()),
            body: summarize_senders(many),
        }),
    }
}

/// Expéditeurs distincts, dans l'ordre d'arrivée, coupés à
/// [`MAX_SENDERS_LISTED`].
fn summarize_senders(arrivals: &[Envelope]) -> String {
    let mut seen: Vec<String> = Vec::new();
    for arrival in arrivals {
        let sender = sender_of(arrival);
        if !seen.contains(&sender) {
            seen.push(sender);
        }
        if seen.len() > MAX_SENDERS_LISTED {
            break;
        }
    }
    if seen.len() > MAX_SENDERS_LISTED {
        let listed = seen[..MAX_SENDERS_LISTED].join(", ");
        format!("{listed}…")
    } else {
        seen.join(", ")
    }
}

/// Un expéditeur absent ne doit pas produire une bulle vide : le repli
/// est explicite, comme dans la liste.
fn sender_of(envelope: &Envelope) -> String {
    envelope
        .sender
        .clone()
        .or_else(|| envelope.sender_address.clone())
        .unwrap_or_else(|| "(expéditeur inconnu)".to_string())
}

fn subject_of(envelope: &Envelope) -> String {
    envelope
        .subject
        .clone()
        .unwrap_or_else(|| "(sans sujet)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrival(uid: u32, sender: Option<&str>, subject: Option<&str>) -> Envelope {
        Envelope {
            uid,
            subject: subject.map(str::to_string),
            sender: sender.map(str::to_string),
            sender_address: None,
            message_id: None,
            date: None,
            seen: false,
            flagged: false,
        }
    }

    /// La règle qui compte le plus. Une première synchro rapatrie la
    /// boîte entière : notifier y ferait annoncer comme « nouveaux » des
    /// milliers de messages vieux de plusieurs années. C'est le défaut
    /// qui fait désactiver les notifications pour de bon.
    #[test]
    fn an_initial_sync_never_notifies() {
        let arrivals = vec![arrival(1, Some("Alice"), Some("Bonjour"))];
        assert!(arrivals_to_notify(SyncMode::Initial, arrivals).is_empty());
    }

    #[test]
    fn an_incremental_sync_keeps_its_arrivals() {
        let arrivals = vec![arrival(1, Some("Alice"), Some("Bonjour"))];
        assert_eq!(arrivals_to_notify(SyncMode::Incremental, arrivals).len(), 1);
    }

    #[test]
    fn nothing_new_shows_nothing() {
        assert_eq!(notification_for(&[]), None);
    }

    #[test]
    fn a_single_message_shows_its_sender_and_subject() {
        let notification =
            notification_for(&[arrival(1, Some("Alice"), Some("Facture mars"))]).unwrap();
        assert_eq!(notification.title, "Alice");
        assert_eq!(notification.body, "Facture mars");
    }

    /// Trois messages arrivés ensemble font UNE bulle, pas trois.
    /// L'empilement est ce qui pousse à couper les notifications.
    #[test]
    fn several_messages_are_summarized_in_a_single_notification() {
        let notification = notification_for(&[
            arrival(1, Some("Alice"), Some("a")),
            arrival(2, Some("Bob"), Some("b")),
        ])
        .unwrap();
        assert_eq!(notification.title, "2 nouveaux messages");
        assert_eq!(notification.body, "Alice, Bob");
    }

    /// Le même expéditeur qui écrit trois fois n'apparaît qu'une fois :
    /// répéter son nom gaspille la seule ligne disponible.
    #[test]
    fn a_repeated_sender_is_listed_once() {
        let notification = notification_for(&[
            arrival(1, Some("Alice"), Some("a")),
            arrival(2, Some("Alice"), Some("b")),
            arrival(3, Some("Bob"), Some("c")),
        ])
        .unwrap();
        assert_eq!(notification.title, "3 nouveaux messages");
        assert_eq!(notification.body, "Alice, Bob");
    }

    #[test]
    fn beyond_three_senders_the_list_is_cut() {
        let notification = notification_for(&[
            arrival(1, Some("Alice"), None),
            arrival(2, Some("Bob"), None),
            arrival(3, Some("Carole"), None),
            arrival(4, Some("David"), None),
        ])
        .unwrap();
        assert_eq!(notification.title, "4 nouveaux messages");
        assert_eq!(notification.body, "Alice, Bob, Carole…");
    }

    /// Une bulle vide serait pire qu'une absence de bulle.
    #[test]
    fn a_message_without_sender_or_subject_still_reads() {
        let notification = notification_for(&[arrival(1, None, None)]).unwrap();
        assert_eq!(notification.title, "(expéditeur inconnu)");
        assert_eq!(notification.body, "(sans sujet)");
    }
}
