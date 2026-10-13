use serde::{Deserialize, Serialize};

pub const COLS: i32 = 48;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum CardId {
    Cpu,
    Memory,
    Gpu,
    Network,
    Storage,
    Battery,
    Audio,
    Thermal,
    Uptime,
    Processes,
    Devices,
    System,
}

impl CardId {
    pub fn all() -> &'static [CardId] {
        &[
            CardId::Cpu,
            CardId::Memory,
            CardId::Gpu,
            CardId::Network,
            CardId::Storage,
            CardId::Battery,
            CardId::Audio,
            CardId::Thermal,
            CardId::Uptime,
            CardId::Processes,
            CardId::Devices,
            CardId::System,
        ]
    }

    pub fn title(self) -> &'static str {
        match self {
            CardId::Cpu => "CPU",
            CardId::Memory => "Memory",
            CardId::Gpu => "GPU",
            CardId::Network => "Network",
            CardId::Storage => "Storage",
            CardId::Battery => "Battery",
            CardId::Audio => "Audio",
            CardId::Thermal => "Thermal",
            CardId::Uptime => "Uptime",
            CardId::Processes => "Processes",
            CardId::Devices => "Devices",
            CardId::System => "System",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GridItem {
    pub id: CardId,
    pub col: i32,
    pub row: i32,
    pub w: i32,
    pub h: i32,
}

impl GridItem {
    pub fn overlaps(&self, other: &GridItem) -> bool {
        if self.id == other.id {
            return false;
        }
        self.col < other.col + other.w
            && self.col + self.w > other.col
            && self.row < other.row + other.h
            && self.row + self.h > other.row
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GridLayout {
    pub items: Vec<GridItem>,
}

impl GridLayout {
    /// Clamp values that came from JSON, which is not validated on the way in.
    /// A zero or negative `w`/`h` would otherwise build a negative-size `Rect`
    /// and poison both the layout and the raccoon's collision rects.
    pub fn sanitize(&mut self) {
        for item in &mut self.items {
            item.w = item.w.clamp(4, COLS);
            item.h = item.h.clamp(3, 24);
            item.col = item.col.clamp(0, COLS - item.w);
            item.row = item.row.max(0);
        }
        self.items.sort_by_key(|i| i.row);
    }
}

impl GridLayout {
    pub fn default_dashboard() -> Self {
        Self {
            items: vec![
                GridItem { id: CardId::Cpu, col: 0, row: 0, w: 16, h: 9 },
                GridItem { id: CardId::Memory, col: 16, row: 0, w: 16, h: 9 },
                GridItem { id: CardId::Gpu, col: 32, row: 0, w: 16, h: 9 },
                GridItem { id: CardId::Network, col: 0, row: 9, w: 20, h: 8 },
                GridItem { id: CardId::Storage, col: 20, row: 9, w: 14, h: 8 },
                GridItem { id: CardId::Battery, col: 34, row: 9, w: 14, h: 8 },
                GridItem { id: CardId::Processes, col: 0, row: 17, w: 20, h: 10 },
                GridItem { id: CardId::Thermal, col: 20, row: 17, w: 14, h: 5 },
                GridItem { id: CardId::Audio, col: 34, row: 17, w: 14, h: 5 },
                GridItem { id: CardId::Uptime, col: 20, row: 22, w: 14, h: 5 },
                GridItem { id: CardId::System, col: 34, row: 22, w: 14, h: 5 },
                GridItem { id: CardId::Devices, col: 0, row: 27, w: 48, h: 5 },
            ],
        }
    }

    pub fn item_mut(&mut self, id: CardId) -> Option<&mut GridItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// Returns true only when the layout actually changed, so callers can save on
    /// real edits instead of once per frame of a drag.
    pub fn try_place(&mut self, id: CardId, col: i32, row: i32, w: i32, h: i32) -> bool {
        let w = w.clamp(4, COLS);
        let h = h.clamp(3, 24);
        let col = col.clamp(0, COLS - w);
        let row = row.max(0);
        let candidate = GridItem { id, col, row, w, h };
        if self
            .items
            .iter()
            .any(|other| other.id != id && candidate.overlaps(other))
        {
            return false;
        }
        if let Some(item) = self.item_mut(id) {
            if *item == candidate {
                return false;
            }
            *item = candidate;
            true
        } else {
            self.items.push(candidate);
            true
        }
    }

    pub fn max_row(&self) -> i32 {
        self.items
            .iter()
            .map(|i| i.row + i.h)
            .max()
            .unwrap_or(12)
    }
}
