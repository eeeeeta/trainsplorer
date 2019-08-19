-- Useful for movement queries
CREATE INDEX train_movements_tiploc_time_day_offset ON train_movements (tiploc, time, day_offset);
CREATE INDEX trains_id_date ON trains (id, date);
