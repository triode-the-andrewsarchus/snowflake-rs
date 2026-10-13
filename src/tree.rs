use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type CardId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: CardId,
    pub text: String,
    /// The three child cards that replaced this card (if expanded)
    pub children: Option<[CardId; 3]>,
    /// Label shown on the card header ("Beginning", "Middle", "End", or custom)
    pub label: String,
}

impl Card {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            text: String::new(),
            children: None,
            label: label.into(),
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_none()
    }
}

/// A card's entry in a flattened column.
#[derive(Debug, Clone)]
pub struct ColumnEntry {
    pub id: CardId,
    /// True if this card was carried over unchanged from the previous column
    /// (i.e. it wasn't expanded, so it persists as-is into the next revision).
    pub carried_over: bool,
}

/// The whole story tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnowflakeTree {
    /// All cards keyed by ID.
    pub cards: HashMap<CardId, Card>,
    /// The three root card IDs (Beginning, Middle, End).
    pub roots: [CardId; 3],
}

impl Default for SnowflakeTree {
    fn default() -> Self {
        let beginning = Card::new("Beginning");
        let middle = Card::new("Middle");
        let end = Card::new("End");
        let roots = [beginning.id.clone(), middle.id.clone(), end.id.clone()];
        let mut cards = HashMap::new();
        cards.insert(beginning.id.clone(), beginning);
        cards.insert(middle.id.clone(), middle);
        cards.insert(end.id.clone(), end);
        Self { cards, roots }
    }
}

impl SnowflakeTree {
    /// Expand a leaf card into three children.
    /// Returns the three new child IDs, or None if card is already expanded or doesn't exist.
    pub fn expand(&mut self, card_id: &str) -> Option<[CardId; 3]> {
        let card = self.cards.get(card_id)?;
        if card.children.is_some() {
            return None; // already expanded
        }
        let label = card.label.clone();
        let (l1, l2, l3) = child_labels(&label);
        let c1 = Card::new(l1);
        let c2 = Card::new(l2);
        let c3 = Card::new(l3);
        let ids = [c1.id.clone(), c2.id.clone(), c3.id.clone()];
        self.cards.insert(c1.id.clone(), c1);
        self.cards.insert(c2.id.clone(), c2);
        self.cards.insert(c3.id.clone(), c3);
        if let Some(card) = self.cards.get_mut(card_id) {
            card.children = Some(ids.clone());
        }
        Some(ids)
    }

    /// Collapse a card (remove its children recursively) and clear its expansion.
    pub fn collapse(&mut self, card_id: &str) {
        let children = self
            .cards
            .get(card_id)
            .and_then(|c| c.children.clone());
        if let Some(child_ids) = children {
            for cid in &child_ids {
                self.collapse(cid);
                self.cards.remove(cid);
            }
        }
        if let Some(card) = self.cards.get_mut(card_id) {
            card.children = None;
        }
    }

    /// Build flattened columns for display.
    ///
    /// Each column represents the story at a progressive level of detail:
    /// - Column 0: the three root beats (Beginning, Middle, End).
    /// - Column N+1: take column N, replace every expanded card with its three
    ///   children, and carry over every unexpanded leaf card unchanged.
    ///
    /// The **rightmost column** always contains the complete active story outline.
    /// Earlier columns show coarser revisions.
    pub fn flattened_columns(&self) -> Vec<Vec<ColumnEntry>> {
        let mut columns: Vec<Vec<ColumnEntry>> = Vec::new();

        // Column 0: roots (never carried-over, they originate here).
        columns.push(
            self.roots
                .iter()
                .map(|id| ColumnEntry {
                    id: id.clone(),
                    carried_over: false,
                })
                .collect(),
        );

        loop {
            let prev = columns.last().unwrap();
            let mut next = Vec::new();
            let mut any_expanded = false;

            for entry in prev {
                if let Some(card) = self.cards.get(&entry.id) {
                    if let Some(children) = &card.children {
                        // Expanded → replace with three children.
                        for cid in children {
                            next.push(ColumnEntry {
                                id: cid.clone(),
                                carried_over: false,
                            });
                        }
                        any_expanded = true;
                    } else {
                        // Leaf → carry over unchanged.
                        next.push(ColumnEntry {
                            id: entry.id.clone(),
                            carried_over: true,
                        });
                    }
                }
            }

            if !any_expanded {
                break; // all entries are leaves — we've reached the frontier
            }

            columns.push(next);
        }

        columns
    }

    /// Number of flattened columns (expansion depth + 1).
    pub fn depth(&self) -> usize {
        self.flattened_columns().len()
    }
}

fn child_labels(parent_label: &str) -> (String, String, String) {
    match parent_label {
        "Beginning" => (
            "Setup".into(),
            "Inciting Incident".into(),
            "First Turning Point".into(),
        ),
        "Middle" => (
            "Rising Action".into(),
            "Midpoint".into(),
            "Complications".into(),
        ),
        "End" => (
            "Climax Setup".into(),
            "Climax".into(),
            "Resolution".into(),
        ),
        _ => (
            format!("{} · Start", parent_label),
            format!("{} · Core", parent_label),
            format!("{} · Close", parent_label),
        ),
    }
}
