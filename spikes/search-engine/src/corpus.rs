//! Corpus synthétique déterministe : 100 000 messages reproductibles.
//!
//! La distribution des mots est zipf-ienne (le carré d'un uniforme biaise
//! vers les mots fréquents), et des TERMES MARQUEURS sont injectés à des
//! fréquences exactes — chaque requête du banc a ainsi une vérité terrain :
//!
//! | marqueur                | règle          | ~docs (sur 100 000) |
//! |-------------------------|----------------|---------------------|
//! | kilimandjaro            | id % 833 == 0  | 121 (rare)          |
//! | budgetaire              | id % 6 == 0    | 16 667 (commun)     |
//! | montagne ∧ horizon      | id % 210 == 0  | 477 (ET)            |
//! | « comite directeur »    | id % 100 == 0  | 1 000 (phrase)      |

pub struct Doc {
    pub id: u64,
    pub subject: String,
    pub sender: String,
    pub body: String,
}

/// Générateur congruentiel (Knuth MMIX) : déterministe, zéro dépendance —
/// largement suffisant pour un corpus de banc d'essai.
struct Lcg {
    state: u64,
}

impl Lcg {
    fn for_doc(id: u64) -> Self {
        Self {
            state: id
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(0xD1B5_4A32_D192_ED03),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Mot au biais zipf-ien : le carré de l'uniforme favorise le début
    /// du vocabulaire (les mots « fréquents »).
    fn word(&mut self) -> &'static str {
        let u = self.uniform();
        VOCABULARY[(u * u * VOCABULARY.len() as f64) as usize]
    }

    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next_u64() % (hi - lo) as u64) as usize
    }
}

pub fn document(id: u64) -> Doc {
    let mut rng = Lcg::for_doc(id);

    let subject: Vec<&str> = (0..rng.range(5, 10)).map(|_| rng.word()).collect();
    let mut body: Vec<&str> = (0..rng.range(120, 350)).map(|_| rng.word()).collect();

    // Les marqueurs du banc (fréquences exactes, positions fixes < 120).
    if id.is_multiple_of(833) {
        body[7] = "kilimandjaro";
    }
    if id.is_multiple_of(6) {
        body[3] = "budgetaire";
    }
    if id.is_multiple_of(30) {
        body[11] = "montagne";
    }
    if id.is_multiple_of(42) {
        body[13] = "horizon";
    }
    if id.is_multiple_of(100) {
        body[20] = "comite";
        body[21] = "directeur";
    }

    Doc {
        id,
        subject: subject.join(" "),
        sender: format!("contact{}@exemple.fr", id % 211),
        body: body.join(" "),
    }
}

pub fn generate(range: std::ops::Range<u64>) -> Vec<Doc> {
    range.map(document).collect()
}

/// Vocabulaire d'emails français — accents compris, pour exercer
/// `remove_diacritics` (« reunion » doit trouver « réunion »).
const VOCABULARY: &[&str] = &[
    "le",
    "la",
    "de",
    "des",
    "un",
    "une",
    "pour",
    "avec",
    "dans",
    "sur",
    "vous",
    "nous",
    "merci",
    "bonjour",
    "cordialement",
    "projet",
    "réunion",
    "équipe",
    "dossier",
    "client",
    "commande",
    "livraison",
    "semaine",
    "prochaine",
    "compte",
    "rendu",
    "point",
    "suite",
    "retour",
    "envoi",
    "document",
    "contrat",
    "signature",
    "délai",
    "planning",
    "mise",
    "jour",
    "version",
    "rapport",
    "activité",
    "mensuel",
    "trimestre",
    "objectif",
    "résultat",
    "chiffre",
    "vente",
    "achat",
    "service",
    "produit",
    "offre",
    "promotion",
    "été",
    "hiver",
    "confirmation",
    "annulation",
    "rendez",
    "disponible",
    "proposition",
    "devis",
    "montant",
    "paiement",
    "échéance",
    "relance",
    "urgent",
    "important",
    "information",
    "détail",
    "question",
    "réponse",
    "validation",
    "accord",
    "modification",
    "changement",
    "nouvelle",
    "ancien",
    "collègue",
    "direction",
    "bureau",
    "site",
    "adresse",
    "téléphone",
    "message",
    "appel",
    "conférence",
    "présentation",
    "formation",
    "atelier",
    "séminaire",
    "invitation",
    "participation",
    "inscription",
    "programme",
    "ordre",
    "prévision",
    "budget",
    "dépense",
    "recette",
    "analyse",
    "étude",
    "marché",
    "concurrence",
    "stratégie",
    "développement",
    "croissance",
    "amélioration",
    "qualité",
    "processus",
    "méthode",
    "outil",
    "système",
    "application",
    "logiciel",
    "serveur",
    "données",
    "sécurité",
    "accès",
    "utilisateur",
    "problème",
    "solution",
    "incident",
    "maintenance",
    "support",
    "assistance",
    "demande",
    "ticket",
    "priorité",
    "traitement",
    "suivi",
    "avancement",
    "étape",
    "phase",
    "livrable",
    "jalon",
    "risque",
    "action",
    "décision",
    "arbitrage",
    "comptable",
    "facturation",
    "salaire",
    "congé",
    "absence",
    "présence",
    "horaire",
    "contrat",
    "avenant",
    "annexe",
];
