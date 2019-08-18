CREATE TABLE train_movements_2 (
	id INTEGER PRIMARY KEY,
	parent_train INT NOT NULL REFERENCES trains ON DELETE CASCADE,
	updates INT REFERENCES train_movements ON DELETE CASCADE,
	tiploc TEXT NOT NULL,
	action INT NOT NULL,
	actual BOOL NOT NULL,
	time TEXT NOT NULL,
	public_time TEXT,
	day_offset INT NOT NULL,
	source INT NOT NULL,
	platform TEXT,
	pfm_suppr BOOL NOT NULL,
	unknown_delay BOOL NOT NULL
);
INSERT INTO train_movements_2 (id, parent_train, updates, tiploc, action, actual, time, public_time, day_offset, source, platform, pfm_suppr, unknown_delay)
	SELECT * FROM train_movements;
DROP TABLE train_movements;
ALTER TABLE train_movements_2 RENAME TO train_movements;
CREATE UNIQUE INDEX train_movements_unique ON train_movements (parent_train, updates, tiploc, action, time, day_offset, source);
