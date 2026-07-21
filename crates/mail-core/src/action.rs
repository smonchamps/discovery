//! Les actions utilisateur en attente — le deuxième cœur du produit.
//!
//! Chaque intention est appliquée localement tout de suite (optimisme UI),
//! journalisée dans SQLite, puis rejouée vers le serveur **en tête de la
//! synchronisation suivante** : une coupure réseau ou un crash n'en perd
//! aucune, c'est le gate de la Phase 2 (PLAN.md §4).

use crate::envelope::Uid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    MarkSeen,
    MarkUnseen,
    /// Poser l'étoile (`\Flagged`).
    MarkFlagged,
    /// Retirer l'étoile.
    MarkUnflagged,
    /// Sortir de la boîte sans supprimer (chez Gmail : reste dans
    /// « Tous les messages »).
    Archive,
    /// Mettre à la corbeille du serveur.
    Delete,
    /// Déplacer vers un dossier choisi par l'utilisateur.
    ///
    /// Porte le nom **RÉSEAU** du dossier (UTF-7 modifié), jamais sa
    /// forme lisible : c'est ce nom-là qu'on renverra au serveur, et une
    /// action journalisée peut être rejouée des jours plus tard.
    MoveTo(String),
}

/// Préfixe des actions portant une destination. Le nom suit le premier
/// `:` — tout ce qui reste en fait partie, y compris d'autres `:`, que
/// les noms de dossiers autorisent.
const MOVE_PREFIX: &str = "move_to:";

impl Action {
    pub(crate) fn to_kind(&self) -> String {
        match self {
            Action::MarkSeen => "mark_seen".to_string(),
            Action::MarkUnseen => "mark_unseen".to_string(),
            Action::MarkFlagged => "mark_flagged".to_string(),
            Action::MarkUnflagged => "mark_unflagged".to_string(),
            Action::Archive => "archive".to_string(),
            Action::Delete => "delete".to_string(),
            Action::MoveTo(folder) => format!("{MOVE_PREFIX}{folder}"),
        }
    }

    pub(crate) fn parse(kind: &str) -> Option<Self> {
        match kind {
            "mark_seen" => Some(Action::MarkSeen),
            "mark_unseen" => Some(Action::MarkUnseen),
            "mark_flagged" => Some(Action::MarkFlagged),
            "mark_unflagged" => Some(Action::MarkUnflagged),
            "archive" => Some(Action::Archive),
            "delete" => Some(Action::Delete),
            other => other
                .strip_prefix(MOVE_PREFIX)
                // Une destination vide n'est pas rejouable : mieux vaut
                // ignorer l'action que déplacer vers nulle part.
                .filter(|folder| !folder.is_empty())
                .map(|folder| Action::MoveTo(folder.to_string())),
        }
    }

    /// L'action fait-elle DISPARAÎTRE le message de la boîte courante ?
    ///
    /// Ce qui disparaît localement doit disparaître côté serveur, et
    /// réciproquement — les trois cas partagent le même traitement en
    /// liste comme en rejeu.
    pub fn removes_from_mailbox(&self) -> bool {
        matches!(self, Action::Archive | Action::Delete | Action::MoveTo(_))
    }
}

/// Une action journalisée, dans l'ordre d'émission (id croissant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAction {
    pub id: i64,
    pub uid: Uid,
    pub action: Action,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_action() -> Vec<Action> {
        vec![
            Action::MarkSeen,
            Action::MarkUnseen,
            Action::MarkFlagged,
            Action::MarkUnflagged,
            Action::Archive,
            Action::Delete,
            Action::MoveTo("Archive".to_string()),
        ]
    }

    /// Une action journalisée peut être relue des jours plus tard, par une
    /// version différente du binaire. L'aller-retour est donc l'invariant
    /// central de ce module : le perdre, c'est perdre des intentions
    /// utilisateur déjà confirmées à l'écran.
    #[test]
    fn every_action_survives_a_round_trip_through_storage() {
        for action in every_action() {
            let kind = action.to_kind();
            assert_eq!(
                Action::parse(&kind).as_ref(),
                Some(&action),
                "aller-retour rompu pour {action:?} (encodé « {kind} »)"
            );
        }
    }

    /// Les noms de dossiers acceptent le `:` — et le séparateur
    /// hiérarchique d'IMAP est souvent `/` ou `.`, mais rien ne l'impose.
    /// Découper sur le DERNIER `:` casserait ces noms.
    #[test]
    fn a_destination_containing_a_colon_is_preserved() {
        let action = Action::MoveTo("Projets:2026/Clients".to_string());
        let kind = action.to_kind();
        assert_eq!(Action::parse(&kind), Some(action));
    }

    /// Le nom RÉSEAU voyage tel quel : ré-encoder ou décoder ici ferait
    /// échouer le rejeu sur un dossier accentué.
    #[test]
    fn an_encoded_folder_name_is_journaled_verbatim() {
        let wire = "Archiv&AOk-s";
        let kind = Action::MoveTo(wire.to_string()).to_kind();
        assert_eq!(kind, "move_to:Archiv&AOk-s");
        assert_eq!(Action::parse(&kind), Some(Action::MoveTo(wire.to_string())));
    }

    #[test]
    fn an_unknown_or_incomplete_kind_is_ignored() {
        assert_eq!(Action::parse("teleporter"), None);
        assert_eq!(Action::parse(""), None);
        assert_eq!(
            Action::parse("move_to:"),
            None,
            "déplacer vers nulle part n'est pas rejouable"
        );
    }

    /// Ces trois-là seuls retirent le message de la boîte : c'est ce qui
    /// décide de la disparition optimiste en liste.
    #[test]
    fn only_removing_actions_take_the_message_out_of_the_mailbox() {
        assert!(Action::Archive.removes_from_mailbox());
        assert!(Action::Delete.removes_from_mailbox());
        assert!(Action::MoveTo("Factures".to_string()).removes_from_mailbox());

        assert!(!Action::MarkSeen.removes_from_mailbox());
        assert!(!Action::MarkFlagged.removes_from_mailbox());
    }
}
