ALTER TABLE schedule_movements DROP COLUMN origterm;
ALTER TABLE schedule_movements ADD COLUMN platform VARCHAR;
ALTER TABLE schedule_movements ADD COLUMN public_time TIME;
ALTER TABLE train_movements ADD COLUMN platform VARCHAR;
ALTER TABLE train_movements ADD COLUMN pfm_suppr BOOL NOT NULL DEFAULT false;
