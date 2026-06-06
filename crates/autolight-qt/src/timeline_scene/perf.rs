#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimelineScenePerfSnapshot {
    pub scene_snapshot_parses: u64,
    pub tile_prepares: u64,
    pub tile_swaps: u64,
    pub paint_updates: u64,
    pub worst_tile_prepare_millis: u64,
    pub worst_scene_graph_update_millis: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TimelineScenePerfCounters {
    snapshot: TimelineScenePerfSnapshot,
}

impl TimelineScenePerfCounters {
    pub fn record_scene_snapshot_parse(&mut self) {
        self.snapshot.scene_snapshot_parses += 1;
    }

    pub fn record_tile_prepare(&mut self, millis: u64) {
        self.snapshot.tile_prepares += 1;
        self.snapshot.worst_tile_prepare_millis =
            self.snapshot.worst_tile_prepare_millis.max(millis);
    }

    pub fn record_tile_swap(&mut self) {
        self.snapshot.tile_swaps += 1;
    }

    pub fn record_paint_update(&mut self, millis: u64) {
        self.snapshot.paint_updates += 1;
        self.snapshot.worst_scene_graph_update_millis =
            self.snapshot.worst_scene_graph_update_millis.max(millis);
    }

    pub fn snapshot(&self) -> TimelineScenePerfSnapshot {
        self.snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::TimelineScenePerfCounters;

    #[test]
    fn timeline_scene_perf_counters_report_worst_timings_and_counts() {
        let mut counters = TimelineScenePerfCounters::default();

        counters.record_scene_snapshot_parse();
        counters.record_tile_prepare(12);
        counters.record_tile_prepare(8);
        counters.record_tile_swap();
        counters.record_paint_update(3);
        counters.record_paint_update(5);
        let snapshot = counters.snapshot();

        assert_eq!(snapshot.scene_snapshot_parses, 1);
        assert_eq!(snapshot.tile_prepares, 2);
        assert_eq!(snapshot.tile_swaps, 1);
        assert_eq!(snapshot.paint_updates, 2);
        assert_eq!(snapshot.worst_tile_prepare_millis, 12);
        assert_eq!(snapshot.worst_scene_graph_update_millis, 5);
    }
}
