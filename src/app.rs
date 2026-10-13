use std::collections::HashMap;
use std::path::PathBuf;
use eframe::egui::{self, Align, Color32, FontId, RichText, ScrollArea, Stroke, TextEdit};
use crate::{
    file::{self, Session},
    tree::{CardId, ColumnEntry, SnowflakeTree},
};

// ── Visual constants ───────────────────────────────────────────────────────────
const CARD_W: f32 = 260.0;
const CARD_H_MIN: f32 = 160.0;
const CARD_PAD: f32 = 12.0;
const COL_GAP: f32 = 56.0; // wide enough for clean connector forks
const ROW_GAP: f32 = 12.0;

// Colours (dark theme)
const BG: Color32 = Color32::from_rgb(18, 18, 24);
const CARD_BG: Color32 = Color32::from_rgb(30, 30, 42);
const CARD_BG_CARRY: Color32 = Color32::from_rgb(26, 26, 36); // slightly dimmer for carry-overs
const CARD_BORDER: Color32 = Color32::from_rgb(60, 60, 90);
const CARD_BORDER_CARRY: Color32 = Color32::from_rgb(45, 45, 65);
const ACCENT: Color32 = Color32::from_rgb(120, 90, 220);
const LEAF_BADGE: Color32 = Color32::from_rgb(60, 200, 140);
const EXPANDED_BADGE: Color32 = Color32::from_rgb(80, 120, 200);
const TEXT_DIM: Color32 = Color32::from_rgb(140, 140, 160);
const TEXT_BRIGHT: Color32 = Color32::from_rgb(220, 220, 240);
const TEXT_CARRY: Color32 = Color32::from_rgb(170, 170, 190); // slightly muted for carry-overs
const EXPAND_CONNECTOR: Color32 = Color32::from_rgb(140, 100, 240);
const CARRY_CONNECTOR: Color32 = Color32::from_rgb(75, 85, 115);

// ── App state ──────────────────────────────────────────────────────────────────
pub struct SnowflakeApp {
    tree: SnowflakeTree,
    current_file: Option<PathBuf>,
    pending_action: Option<PendingAction>,
    status: Option<String>,
    show_open_dialog: bool,
    show_save_dialog: bool,
    dirty: bool,
    scroll_to_end: bool,
}

#[derive(Debug, Clone)]
enum PendingAction {
    Expand(CardId),
    Collapse(CardId),
}

#[derive(Debug, Clone)]
enum ConnectionKind {
    Expansion { children: [CardId; 3] },
    CarryOver { child_id: CardId },
}

/// Pre-collected connection info for drawing connectors between column pairs.
struct ConnectionInfo {
    parent_id: CardId,
    kind: ConnectionKind,
}

impl SnowflakeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = CARD_BG;
        visuals.override_text_color = Some(TEXT_BRIGHT);
        cc.egui_ctx.set_visuals(visuals);

        let session = file::load_session();
        let (tree, current_file) = if let Some(ref path) = session.last_file {
            match file::load(path) {
                Ok(t) => (t, Some(path.clone())),
                Err(_) => (SnowflakeTree::default(), None),
            }
        } else {
            (SnowflakeTree::default(), None)
        };

        Self {
            tree,
            current_file,
            pending_action: None,
            status: None,
            show_open_dialog: false,
            show_save_dialog: false,
            dirty: false,
            scroll_to_end: true, // start scrolled to the leaf column
        }
    }

    // ── Toolbar ─────────────────────────────────────────────────────────────────
    fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("❄ Snowflake")
                    .color(ACCENT)
                    .font(FontId::proportional(18.0))
                    .strong(),
            );
            ui.separator();

            if ui
                .button("📂 Open")
                .on_hover_text("Open a .snowflake file")
                .clicked()
            {
                self.show_open_dialog = true;
            }
            if ui
                .button("💾 Save")
                .on_hover_text("Save current file (Ctrl+S)")
                .clicked()
            {
                if self.current_file.is_some() {
                    self.do_save(None);
                } else {
                    self.show_save_dialog = true;
                }
            }
            if ui.button("💾 Save As…").clicked() {
                self.show_save_dialog = true;
            }
            if ui
                .button("🆕 New")
                .on_hover_text("Start a new story")
                .clicked()
            {
                self.tree = SnowflakeTree::default();
                self.current_file = None;
                self.dirty = false;
                self.scroll_to_end = true;
                self.status = Some("New story started.".into());
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let depth = self.tree.depth();
                ui.label(
                    RichText::new(format!("{} revision(s)", depth))
                        .color(TEXT_DIM)
                        .font(FontId::proportional(12.0)),
                );
                if let Some(ref f) = self.current_file {
                    let name = f
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "untitled".into());
                    let title = if self.dirty {
                        format!("● {}", name)
                    } else {
                        name
                    };
                    ui.label(
                        RichText::new(title)
                            .color(TEXT_DIM)
                            .font(FontId::proportional(12.0)),
                    );
                }
            });
        });
    }

    // ── Status bar ───────────────────────────────────────────────────────────────
    fn draw_status(&self, ui: &mut egui::Ui) {
        if let Some(ref msg) = self.status {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(msg)
                        .color(TEXT_DIM)
                        .font(FontId::proportional(11.0)),
                );
            });
        }
    }

    // ── Main canvas ─────────────────────────────────────────────────────────────
    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        // ── Pre-compute owned data (immutable borrows, all released before mut) ──
        let flat_cols: Vec<Vec<ColumnEntry>> = self.tree.flattened_columns();
        let num_cols = flat_cols.len();
        let is_last_col = |idx: usize| idx == num_cols - 1;

        // For each column gap, collect connections (expansions & carry-overs).
        let connections_per_col: Vec<Vec<ConnectionInfo>> = flat_cols
            .iter()
            .enumerate()
            .map(|(col_idx, col)| {
                if col_idx + 1 >= flat_cols.len() {
                    return Vec::new();
                }
                col.iter()
                    .filter_map(|entry| {
                        if let Some(card) = self.tree.cards.get(&entry.id) {
                            if let Some(children) = &card.children {
                                Some(ConnectionInfo {
                                    parent_id: entry.id.clone(),
                                    kind: ConnectionKind::Expansion {
                                        children: children.clone(),
                                    },
                                })
                            } else {
                                Some(ConnectionInfo {
                                    parent_id: entry.id.clone(),
                                    kind: ConnectionKind::CarryOver {
                                        child_id: entry.id.clone(),
                                    },
                                })
                            }
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect();

        let scroll_to_end = std::mem::replace(&mut self.scroll_to_end, false);

        // ── Split self into disjoint mutable borrows for closures ────────────────
        let tree = &mut self.tree;
        let pending = &mut self.pending_action;
        let dirty = &mut self.dirty;

        // Keyed by (col_idx, card_id) since carry-overs appear in multiple cols.
        let mut card_rects: HashMap<(usize, String), egui::Rect> = HashMap::new();

        ScrollArea::horizontal()
            .auto_shrink(false)
            .show(ui, |ui| {
                // Lay out columns left-to-right; center-align vertically.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = COL_GAP;

                    for (col_idx, col) in flat_cols.iter().enumerate() {
                        let last_col = is_last_col(col_idx);

                        // push_id so carry-over cards in different columns
                        // get unique widget IDs for their TextEdits.
                        ui.push_id(col_idx, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(CARD_W);
                                ui.spacing_mut().item_spacing.y = ROW_GAP;

                                // Column header
                                if last_col {
                                    ui.label(
                                        RichText::new("▸ Active Story")
                                            .color(LEAF_BADGE)
                                            .font(FontId::proportional(10.0))
                                            .strong(),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new(format!("Revision {}", col_idx))
                                            .color(TEXT_DIM)
                                            .font(FontId::proportional(10.0)),
                                    );
                                }

                                for entry in col {
                                    let rect = Self::draw_card(
                                        ui, tree, entry, last_col, pending, dirty,
                                    );
                                    card_rects
                                        .insert((col_idx, entry.id.clone()), rect);
                                }
                            });
                        });
                    }

                    if scroll_to_end {
                        ui.scroll_to_cursor(Some(Align::Max));
                    }
                });

                // ── Draw connector lines ──────────────────────────────────────
                let painter = ui.painter();

                for (col_idx, conns) in connections_per_col.iter().enumerate() {
                    let num_conns = conns.len();
                    if num_conns == 0 {
                        continue;
                    }

                    for (conn_idx, conn) in conns.iter().enumerate() {
                        let parent_key = (col_idx, conn.parent_id.clone());
                        let Some(&parent_rect) = card_rects.get(&parent_key) else {
                            continue;
                        };

                        // Stagger mid_x for each connection in the column gap
                        // so vertical lines never overlap.
                        let x_start = parent_rect.max.x + 8.0;
                        let x_end = parent_rect.max.x + COL_GAP - 8.0;
                        let usable_w = x_end - x_start;
                        let mid_x = if num_conns > 1 {
                            x_start + (conn_idx as f32 + 1.0) * usable_w / (num_conns as f32 + 1.0)
                        } else {
                            parent_rect.max.x + COL_GAP / 2.0
                        };

                        let parent_cy = parent_rect.center().y;

                        match &conn.kind {
                            ConnectionKind::Expansion { children } => {
                                let stroke = Stroke::new(2.0_f32, EXPAND_CONNECTOR);

                                // Collect child rects in col_idx + 1
                                let child_rects: Vec<egui::Rect> = children
                                    .iter()
                                    .filter_map(|cid| {
                                        card_rects.get(&(col_idx + 1, cid.clone())).copied()
                                    })
                                    .collect();

                                if child_rects.is_empty() {
                                    continue;
                                }

                                let child_ys: Vec<f32> =
                                    child_rects.iter().map(|r| r.center().y).collect();
                                let min_y = child_ys
                                    .iter()
                                    .copied()
                                    .fold(f32::INFINITY, f32::min)
                                    .min(parent_cy);
                                let max_y = child_ys
                                    .iter()
                                    .copied()
                                    .fold(f32::NEG_INFINITY, f32::max)
                                    .max(parent_cy);

                                // 1) Exit dot at parent right edge
                                painter.circle_filled(
                                    egui::pos2(parent_rect.max.x, parent_cy),
                                    3.5_f32,
                                    EXPAND_CONNECTOR,
                                );

                                // 2) Horizontal stub from parent to mid_x
                                painter.line_segment(
                                    [
                                        egui::pos2(parent_rect.max.x, parent_cy),
                                        egui::pos2(mid_x, parent_cy),
                                    ],
                                    stroke,
                                );

                                // 3) Vertical spine at mid_x
                                if (max_y - min_y).abs() > 1.0 {
                                    painter.line_segment(
                                        [egui::pos2(mid_x, min_y), egui::pos2(mid_x, max_y)],
                                        stroke,
                                    );
                                }

                                // 4) Horizontal stubs to children + entry dots
                                for cr in &child_rects {
                                    let cy = cr.center().y;
                                    painter.line_segment(
                                        [egui::pos2(mid_x, cy), egui::pos2(cr.min.x, cy)],
                                        stroke,
                                    );
                                    painter.circle_filled(
                                        egui::pos2(cr.min.x, cy),
                                        3.0_f32,
                                        EXPAND_CONNECTOR,
                                    );
                                }

                                // Junction dot at spine/parent-stub intersection
                                painter.circle_filled(
                                    egui::pos2(mid_x, parent_cy),
                                    2.5_f32,
                                    EXPAND_CONNECTOR,
                                );
                            }
                            ConnectionKind::CarryOver { child_id } => {
                                let stroke = Stroke::new(1.2_f32, CARRY_CONNECTOR);

                                let Some(&child_rect) =
                                    card_rects.get(&(col_idx + 1, child_id.clone()))
                                else {
                                    continue;
                                };

                                let child_cy = child_rect.center().y;

                                if (parent_cy - child_cy).abs() < 2.0 {
                                    // Straight horizontal line
                                    painter.line_segment(
                                        [
                                            egui::pos2(parent_rect.max.x, parent_cy),
                                            egui::pos2(child_rect.min.x, parent_cy),
                                        ],
                                        stroke,
                                    );
                                } else {
                                    // Step line via mid_x
                                    painter.line_segment(
                                        [
                                            egui::pos2(parent_rect.max.x, parent_cy),
                                            egui::pos2(mid_x, parent_cy),
                                        ],
                                        stroke,
                                    );
                                    painter.line_segment(
                                        [
                                            egui::pos2(mid_x, parent_cy),
                                            egui::pos2(mid_x, child_cy),
                                        ],
                                        stroke,
                                    );
                                    painter.line_segment(
                                        [
                                            egui::pos2(mid_x, child_cy),
                                            egui::pos2(child_rect.min.x, child_cy),
                                        ],
                                        stroke,
                                    );
                                }

                                // Subtle dot at child entry point
                                painter.circle_filled(
                                    egui::pos2(child_rect.min.x, child_cy),
                                    2.0_f32,
                                    CARRY_CONNECTOR,
                                );
                            }
                        }
                    }
                }
            });
    }

    /// Draw a single card.  Returns the outer rect for connector drawing.
    fn draw_card(
        ui: &mut egui::Ui,
        tree: &mut SnowflakeTree,
        entry: &ColumnEntry,
        is_active_col: bool,
        pending: &mut Option<PendingAction>,
        dirty: &mut bool,
    ) -> egui::Rect {
        let card_id = &entry.id;
        let is_leaf = tree
            .cards
            .get(card_id)
            .map(|c| c.is_leaf())
            .unwrap_or(true);
        let label = tree
            .cards
            .get(card_id)
            .map(|c| c.label.clone())
            .unwrap_or_default();
        let badge = if is_leaf { LEAF_BADGE } else { EXPANDED_BADGE };

        // Carried-over cards are visually muted.
        let bg = if entry.carried_over { CARD_BG_CARRY } else { CARD_BG };
        let border = if entry.carried_over { CARD_BORDER_CARRY } else { CARD_BORDER };
        let text_col = if entry.carried_over { TEXT_CARRY } else { TEXT_BRIGHT };

        let frame_resp = egui::Frame::new()
            .fill(bg)
            .stroke(Stroke::new(1.0_f32, border))
            .corner_radius(8.0)
            .inner_margin(CARD_PAD)
            .show(ui, |ui| {
                ui.set_width(CARD_W - CARD_PAD * 2.0);
                ui.set_min_height(CARD_H_MIN - CARD_PAD * 2.0);

                // ── Header ──────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("●")
                            .color(badge)
                            .font(FontId::proportional(10.0)),
                    );
                    ui.label(
                        RichText::new(&label)
                            .color(badge)
                            .font(FontId::proportional(12.0))
                            .strong(),
                    );
                    if entry.carried_over {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new("↳")
                                        .color(TEXT_DIM)
                                        .font(FontId::proportional(11.0)),
                                );
                            },
                        );
                    }
                });
                ui.separator();

                // ── Text editor ─────────────────────────────────────────
                if let Some(card) = tree.cards.get_mut(card_id) {
                    let te = TextEdit::multiline(&mut card.text)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY)
                        .hint_text("Write your story beat here…")
                        .font(FontId::proportional(13.0))
                        .text_color(text_col);
                    let r = ui.add(te);
                    if r.changed() {
                        *dirty = true;
                    }
                }

                ui.add_space(4.0);

                // ── Expand / Collapse button ────────────────────────────
                // Only show action buttons in the active (rightmost) column
                // to avoid confusion with carry-over duplicates.
                if is_active_col {
                    if is_leaf {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("⊞ Expand →")
                                        .color(ACCENT)
                                        .font(FontId::proportional(12.0)),
                                )
                                .fill(ACCENT.gamma_multiply(0.12))
                                .stroke(Stroke::new(1.0_f32, ACCENT)),
                            )
                            .clicked()
                        {
                            *pending =
                                Some(PendingAction::Expand(card_id.clone()));
                        }
                    }
                    // No collapse in active col — use collapse in the history col
                    // where the card was first expanded.
                } else if !is_leaf {
                    // In history columns, expanded cards can be collapsed.
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("⊟ Collapse")
                                    .color(TEXT_DIM)
                                    .font(FontId::proportional(12.0)),
                            )
                            .fill(CARD_BORDER.gamma_multiply(0.25))
                            .stroke(Stroke::new(1.0_f32, CARD_BORDER)),
                        )
                        .clicked()
                    {
                        *pending =
                            Some(PendingAction::Collapse(card_id.clone()));
                    }
                }
            });

        frame_resp.response.rect
    }

    // ── File dialog helpers ──────────────────────────────────────────────────────
    fn handle_file_dialogs(&mut self) {
        if self.show_open_dialog {
            self.show_open_dialog = false;
            let result = rfd::FileDialog::new()
                .add_filter("Snowflake", &["snowflake"])
                .set_title("Open Snowflake Story")
                .pick_file();
            if let Some(path) = result {
                match file::load(&path) {
                    Ok(tree) => {
                        self.tree = tree;
                        self.current_file = Some(path.clone());
                        self.dirty = false;
                        self.scroll_to_end = true;
                        self.status = Some(format!("Opened: {}", path.display()));
                        file::save_session(&Session {
                            last_file: Some(path),
                        });
                    }
                    Err(e) => {
                        self.status = Some(format!("Error opening file: {}", e));
                    }
                }
            }
        }

        if self.show_save_dialog {
            self.show_save_dialog = false;
            let result = rfd::FileDialog::new()
                .add_filter("Snowflake", &["snowflake"])
                .set_title("Save Snowflake Story")
                .set_file_name("story.snowflake")
                .save_file();
            if let Some(path) = result {
                self.do_save(Some(path));
            }
        }
    }

    fn do_save(&mut self, path: Option<PathBuf>) {
        let path = path.or_else(|| self.current_file.clone());
        let Some(path) = path else {
            self.status = Some("No file path. Use Save As…".into());
            return;
        };
        match file::save(&path, &self.tree) {
            Ok(()) => {
                self.current_file = Some(path.clone());
                self.dirty = false;
                self.status = Some(format!("Saved: {}", path.display()));
                file::save_session(&Session {
                    last_file: Some(path),
                });
            }
            Err(e) => {
                self.status = Some(format!("Save error: {}", e));
            }
        }
    }
}

impl eframe::App for SnowflakeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_file_dialogs();

        if let Some(action) = self.pending_action.take() {
            match action {
                PendingAction::Expand(id) => {
                    self.tree.expand(&id);
                    self.dirty = true;
                    self.scroll_to_end = true;
                }
                PendingAction::Collapse(id) => {
                    self.tree.collapse(&id);
                    self.dirty = true;
                }
            }
        }

        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
            if self.current_file.is_some() {
                self.do_save(None);
            } else {
                self.show_save_dialog = true;
            }
        }

        egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(22, 22, 32))
                    .inner_margin(6.0),
            )
            .show(ctx, |ui| {
                self.draw_toolbar(ui);
            });

        egui::TopBottomPanel::bottom("status_bar")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(15, 15, 22))
                    .inner_margin(4.0),
            )
            .exact_height(22.0)
            .show(ctx, |ui| {
                self.draw_status(ui);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ctx, |ui| {
                self.draw_canvas(ui);
            });

        ctx.request_repaint();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(ref path) = self.current_file.clone() {
            let _ = file::save(path, &self.tree);
        }
        file::save_session(&Session {
            last_file: self.current_file.clone(),
        });
    }
}
