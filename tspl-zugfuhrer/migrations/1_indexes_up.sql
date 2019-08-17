CREATE INDEX IF NOT EXISTS trains_parents_and_date ON trains (parent_uid, parent_start_date, parent_stp_indicator, parent_source, date);
