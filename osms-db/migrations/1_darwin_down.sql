ALTER TABLE train_movements DROP CONSTRAINT train_movements_source_fkey;
DROP TABLE movement_sources;
ALTER TABLE train_movements DROP COLUMN estimated;
ALTER TABLE trains DROP COLUMN nre_id;
