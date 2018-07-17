ALTER TABLE train_movements ADD CONSTRAINT train_movements_parent_mvt_source_unique UNIQUE(parent_mvt, source);
