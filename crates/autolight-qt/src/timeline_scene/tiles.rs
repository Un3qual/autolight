use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineTileKey {
    pub track_row: usize,
    pub layer: TimelineTileLayer,
    pub zoom_bucket: i32,
    pub start_bucket: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimelineTileLayer {
    Waveform,
    Energy,
    HarmonicColor,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedTimelineTile {
    pub key: TimelineTileKey,
    pub origin_seconds: f64,
    pub width_seconds: f64,
    pub bands: Vec<PreparedTimelineBand>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedTimelineBand {
    pub color_rgba: [f32; 4],
    pub rects: Vec<PreparedTimelineRect>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedTimelineRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PreparedTimelineTile {
    pub fn is_json_payload(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Default)]
pub struct TimelineTileBuffer {
    active_tiles: BTreeMap<TimelineTileKey, PreparedTimelineTile>,
    pending_tiles: BTreeMap<TimelineTileKey, PreparedTimelineTile>,
    last_good_tiles: BTreeMap<TimelineTileKey, PreparedTimelineTile>,
}

impl TimelineTileBuffer {
    pub fn set_active_tiles(&mut self, tiles: impl IntoIterator<Item = PreparedTimelineTile>) {
        self.active_tiles = tiles.into_iter().map(|tile| (tile.key, tile)).collect();
        self.last_good_tiles = self.active_tiles.clone();
    }

    pub fn queue_pending_tile(&mut self, tile: PreparedTimelineTile) {
        self.pending_tiles.insert(tile.key, tile);
    }

    pub fn promote_pending_tiles(&mut self) {
        if self.pending_tiles.is_empty() {
            return;
        }
        self.active_tiles
            .extend(std::mem::take(&mut self.pending_tiles));
        self.last_good_tiles = self.active_tiles.clone();
    }

    pub fn active_tile(&self, key: TimelineTileKey) -> Option<&PreparedTimelineTile> {
        self.active_tiles
            .get(&key)
            .or_else(|| self.last_good_tiles.get(&key))
    }

    pub fn pending_tile(&self, key: TimelineTileKey) -> Option<&PreparedTimelineTile> {
        self.pending_tiles.get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PreparedTimelineBand, PreparedTimelineRect, PreparedTimelineTile, TimelineTileBuffer,
        TimelineTileKey, TimelineTileLayer,
    };

    #[test]
    fn timeline_tile_payload_is_not_json() {
        let tile = PreparedTimelineTile {
            key: TimelineTileKey {
                track_row: 0,
                layer: TimelineTileLayer::Waveform,
                zoom_bucket: 4,
                start_bucket: 12,
            },
            origin_seconds: 1.0,
            width_seconds: 2.0,
            bands: vec![PreparedTimelineBand {
                color_rgba: [0.2, 0.4, 0.8, 1.0],
                rects: vec![PreparedTimelineRect {
                    x: 0.0,
                    y: 1.0,
                    width: 2.0,
                    height: 3.0,
                }],
            }],
        };

        assert!(!tile.is_json_payload());
    }

    #[test]
    fn timeline_tile_keys_are_ordered_for_active_pending_swaps() {
        let active = TimelineTileKey {
            track_row: 1,
            layer: TimelineTileLayer::Waveform,
            zoom_bucket: 3,
            start_bucket: 10,
        };
        let pending = TimelineTileKey {
            start_bucket: 11,
            ..active
        };

        assert!(active < pending);
    }

    #[test]
    fn timeline_tiles_reuse_active_tile_during_scroll_within_tile() {
        let key = tile_key(0, 3, 12);
        let tile = tile(key, 0.0);
        let mut buffer = TimelineTileBuffer::default();

        buffer.set_active_tiles([tile.clone()]);

        assert_eq!(buffer.active_tile(key), Some(&tile));
    }

    #[test]
    fn timeline_tiles_prepare_next_zoom_bucket_without_replacing_active_tile() {
        let active_key = tile_key(0, 3, 12);
        let pending_key = tile_key(0, 4, 12);
        let active = tile(active_key, 0.0);
        let pending = tile(pending_key, 0.0);
        let mut buffer = TimelineTileBuffer::default();

        buffer.set_active_tiles([active.clone()]);
        buffer.queue_pending_tile(pending.clone());

        assert_eq!(buffer.active_tile(active_key), Some(&active));
        assert_eq!(buffer.pending_tile(pending_key), Some(&pending));

        buffer.promote_pending_tiles();

        assert_eq!(buffer.active_tiles.len(), 2);
        assert!(buffer.pending_tiles.is_empty());
        assert_eq!(buffer.active_tile(pending_key), Some(&pending));
        assert_eq!(buffer.active_tile(active_key), Some(&active));
    }

    fn tile_key(track_row: usize, zoom_bucket: i32, start_bucket: i64) -> TimelineTileKey {
        TimelineTileKey {
            track_row,
            layer: TimelineTileLayer::Waveform,
            zoom_bucket,
            start_bucket,
        }
    }

    fn tile(key: TimelineTileKey, origin_seconds: f64) -> PreparedTimelineTile {
        PreparedTimelineTile {
            key,
            origin_seconds,
            width_seconds: 2.0,
            bands: vec![PreparedTimelineBand {
                color_rgba: [0.2, 0.4, 0.8, 1.0],
                rects: vec![PreparedTimelineRect {
                    x: 0.0,
                    y: 1.0,
                    width: 2.0,
                    height: 3.0,
                }],
            }],
        }
    }
}
